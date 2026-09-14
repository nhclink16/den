use crate::*;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;

const TOPIC: &str = "app.denchat.ios";

#[derive(Clone, Copy, Default)]
pub(crate) enum Environment {
    #[default]
    Sandbox,
    Production,
}
impl Environment {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Production => "production",
        }
    }
    fn endpoint(self) -> &'static str {
        match self {
            Self::Sandbox => "https://api.sandbox.push.apple.com",
            Self::Production => "https://api.push.apple.com",
        }
    }
}

pub(crate) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_millis() as i64
}

pub(crate) struct Apns {
    environment: Environment,
    endpoint: String,
    http: reqwest::Client,
    key: EncodingKey,
    key_id: String,
    team_id: String,
    jwt: Option<(i64, String)>,
}

impl Apns {
    fn new(
        environment: Environment,
        key_id: String,
        team_id: String,
        pem: &[u8],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            environment,
            endpoint: environment.endpoint().into(),
            http: reqwest::Client::builder()
                .http2_prior_knowledge()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .build()?,
            key: EncodingKey::from_ec_pem(pem)?,
            key_id,
            team_id,
            jwt: None,
        })
    }
    fn bearer(&mut self) -> anyhow::Result<&str> {
        let time = now();
        if self
            .jwt
            .as_ref()
            .is_none_or(|(issued, _)| time < *issued || time - issued >= 3000)
        {
            #[derive(Serialize)]
            struct Claims<'a> {
                iss: &'a str,
                iat: i64,
            }
            let mut header = Header::new(Algorithm::ES256);
            header.kid = Some(self.key_id.clone());
            let token = encode(
                &header,
                &Claims {
                    iss: &self.team_id,
                    iat: time,
                },
                &self.key,
            )?;
            self.jwt = Some((time, token));
        }
        Ok(&self.jwt.as_ref().expect("JWT created").1)
    }
}

impl AppState {
    /// Configure alert APNs once before sharing the server state. No credentials
    /// means one startup log and a no-op sender. Tests do not read process env.
    pub async fn with_apns_from_env(mut self) -> anyhow::Result<Self> {
        let environment = match std::env::var("DEN_APNS_ENV").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("sandbox") => Environment::Sandbox,
            Ok("production") => Environment::Production,
            _ => anyhow::bail!("DEN_APNS_ENV must be sandbox or production"),
        };
        Arc::get_mut(&mut self.0)
            .expect("configure before sharing")
            .apns_environment = environment;
        let key_path = std::env::var("DEN_APNS_KEY_PATH").unwrap_or_default();
        let key_id = std::env::var("DEN_APNS_KEY_ID").unwrap_or_default();
        let team_id = std::env::var("DEN_APNS_TEAM_ID").unwrap_or_default();
        if key_path.is_empty() || key_id.is_empty() || team_id.is_empty() {
            tracing::info!("APNs is not configured; push delivery disabled");
            return Ok(self);
        }
        let pem = tokio::fs::read(key_path)
            .await
            .map_err(|_| anyhow::anyhow!("Cannot read DEN_APNS_KEY_PATH"))?;
        let apns = Apns::new(environment, key_id, team_id, &pem).map_err(|_| {
            anyhow::anyhow!("Invalid APNs signing key or HTTP client configuration")
        })?;
        Ok(self.with_apns(apns))
    }
    fn with_apns(mut self, apns: Apns) -> Self {
        let inner = Arc::get_mut(&mut self.0).expect("configure before sharing");
        inner.apns_environment = apns.environment;
        let (tx, mut rx) = mpsc::channel::<Job>(256);
        let db = inner.db.clone();
        tokio::spawn(async move {
            let mut apns = apns;
            while let Some(job) = rx.recv().await {
                if deliver(&db, &mut apns, &job).await.is_err() {
                    // Transport errors can include the token in their URL.
                    tracing::warn!("APNs delivery failed; notification was not delivered");
                }
            }
        });
        inner.push = Some(tx);
        self
    }
}

pub(crate) struct Job {
    user_id: String,
    channel_id: String,
    expires_at: i64,
    payload: Value,
    invitation: Option<CallInvitation>,
}

fn queue(s: &AppState, job: Job) {
    if let Some(tx) = &s.push {
        if tx.try_send(job).is_err() {
            tracing::warn!("APNs queue full or stopped; notification skipped");
        }
    }
}

pub(crate) fn notification(
    s: &AppState,
    user: &str,
    message: &Message,
    reason: NotificationReason,
    badge: i64,
) {
    let title = match reason {
        NotificationReason::Mention => "You were mentioned in Den",
        NotificationReason::Dm => "New direct message in Den",
        NotificationReason::SubscribedChannel => "New message in Den",
    };
    let body: String = message.content.chars().take(180).collect();
    queue(
        s,
        Job {
            user_id: user.into(),
            channel_id: message.channel_id.clone(),
            expires_at: now() + 3600,
            payload: json!({"aps":{"alert":{"title":title,"body":body},"sound":"default","badge":badge},
            "type":"notification","channel_id":message.channel_id,"message_id":message.id,"reason":reason}),
            invitation: None,
        },
    );
}

/// The same custom fields are used by the socket event and the future VoIP
/// payload. This slice sends an ordinary alert token; it never mislabels an
/// alert registration as PushKit or sends apns-push-type: voip to that token.
pub(crate) fn invitation(s: &AppState, user: &str, invite: &CallInvitation) {
    queue(
        s,
        Job {
            user_id: user.into(),
            channel_id: invite.channel_id.clone(),
            expires_at: invite.expires_at,
            payload: json!({"aps":{"alert":{"title":"Incoming Den call","body":"Open Den to join"},"sound":"default"},
            "type":"call_invite","channel_id":invite.channel_id,"from_user_id":invite.from_user_id,"expires_at":invite.expires_at}),
            invitation: Some(invite.clone()),
        },
    );
}

#[derive(sqlx::FromRow)]
struct Destination {
    id: String,
    token: String,
    generation: i64,
    registered_at: i64,
}
#[derive(Deserialize)]
struct Rejection {
    reason: Option<String>,
    timestamp: Option<i64>,
}

async fn deliver(db: &SqlitePool, apns: &mut Apns, job: &Job) -> anyhow::Result<()> {
    if now() >= job.expires_at {
        return Ok(());
    }
    let body = serde_json::to_vec(&job.payload)?;
    anyhow::ensure!(body.len() <= 4096, "APNs payload exceeds alert limit");
    let devices = sqlx::query_as::<_, Destination>(
        "SELECT d.id,d.token,d.generation,d.registered_at FROM devices d
         WHERE d.user_id=? AND d.environment=? AND
         (EXISTS(SELECT 1 FROM sessions WHERE id=d.session_id AND expires_at>?)
          OR EXISTS(SELECT 1 FROM tokens WHERE id=d.token_id))",
    )
    .bind(&job.user_id)
    .bind(apns.environment.as_str())
    .bind(now())
    .fetch_all(db)
    .await?;
    for device in devices {
        if now() >= job.expires_at {
            break;
        }
        let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channels c WHERE c.id=? AND (c.kind IN ('text','voice') OR EXISTS(SELECT 1 FROM channel_members WHERE channel_id=c.id AND user_id=?)))")
            .bind(&job.channel_id).bind(&job.user_id).fetch_one(db).await?;
        if !visible {
            break;
        }
        if let Some(invite) = &job.invitation {
            if !crate::invitations::pending(db, &job.user_id, invite).await? {
                break;
            }
        }
        // Re-registration, logout or account switching while this job was queued
        // must not send the previous account's notification to the new owner.
        let current: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM devices d WHERE d.id=? AND d.generation=? AND d.user_id=?
             AND (EXISTS(SELECT 1 FROM sessions WHERE id=d.session_id AND expires_at>?)
             OR EXISTS(SELECT 1 FROM tokens WHERE id=d.token_id)))",
        )
        .bind(&device.id)
        .bind(device.generation)
        .bind(&job.user_id)
        .bind(now())
        .fetch_one(db)
        .await?;
        if !current {
            continue;
        }
        let bearer = apns.bearer()?.to_owned();
        let response = apns
            .http
            .post(format!("{}/3/device/{}", apns.endpoint, device.token))
            .bearer_auth(bearer)
            .header("apns-topic", TOPIC)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header("apns-collapse-id", &job.channel_id)
            .header("apns-expiration", job.expires_at.to_string())
            .header("content-type", "application/json")
            .body(body.clone())
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            continue;
        }
        let rejection = response.json::<Rejection>().await.ok();
        let invalid =
            rejection
                .as_ref()
                .is_some_and(|r| match (status.as_u16(), r.reason.as_deref()) {
                    (400, Some("BadDeviceToken" | "DeviceTokenNotForTopic")) => true,
                    (410, Some("Unregistered")) => {
                        r.timestamp.is_none_or(|at| device.registered_at <= at)
                    }
                    _ => false,
                });
        if invalid {
            sqlx::query("DELETE FROM devices WHERE id=? AND generation=?")
                .bind(&device.id)
                .bind(device.generation)
                .execute(db)
                .await?;
        } else {
            tracing::warn!(
                status = status.as_u16(),
                "APNs rejected notification; registration retained"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "push/tests.rs"]
mod tests;
