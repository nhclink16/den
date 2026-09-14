use super::*;
use axum::{
    body::to_bytes,
    extract::Request,
    http::{HeaderMap, Version},
    response::IntoResponse,
};
use tokio::sync::oneshot;

// Disposable test-only key pair, generated for this HTTP/2 stub. Never used
// with Apple or any real app, account, or device.
const KEY: &[u8] = include_bytes!("../../tests/fixtures/apns-test-key.pem");
const PUBLIC: &[u8] = include_bytes!("../../tests/fixtures/apns-test-public.pem");

struct Captured {
    version: Version,
    headers: HeaderMap,
    path: String,
    body: Value,
    reply: oneshot::Sender<(StatusCode, Value)>,
}
struct Fixture {
    state: AppState,
    dir: PathBuf,
    requests: mpsc::Receiver<Captured>,
    server: tokio::task::JoinHandle<()>,
    den: Option<(String, tokio::task::JoinHandle<()>)>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        if let Some((_, server)) = &self.den {
            server.abort();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
impl Fixture {
    async fn new(environment: Environment) -> Self {
        let (tx, requests) = mpsc::channel(16);
        let app = Router::new().fallback(move |req: Request| {
            let tx = tx.clone();
            async move {
                let (parts, body) = req.into_parts();
                let body = to_bytes(body, 4096).await.unwrap();
                let (reply, result) = oneshot::channel();
                tx.send(Captured {
                    version: parts.version,
                    headers: parts.headers,
                    path: parts.uri.path().into(),
                    body: serde_json::from_slice(&body).unwrap(),
                    reply,
                })
                .await
                .unwrap();
                let (status, body) = result
                    .await
                    .unwrap_or((StatusCode::INTERNAL_SERVER_ERROR, json!({})));
                (status, Json(body)).into_response()
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let dir = std::env::temp_dir().join(format!("den-apns-test-{}", ulid::Ulid::new()));
        let mut apns =
            Apns::new(environment, "test-key-id".into(), "test-team".into(), KEY).unwrap();
        apns.endpoints = [endpoint.clone(), format!("{endpoint}/production")];
        let state = AppState::open(
            dir.join("den.db"),
            dir.join("uploads"),
            dir.join("bootstrap"),
            "http://127.0.0.1:7000".into(),
            1024,
        )
        .await
        .unwrap()
        .with_apns(apns);
        for id in ["alice", "bob"] {
            sqlx::query("INSERT INTO users(id,username,display_name,role) VALUES(?,?,?,'member')")
                .bind(id)
                .bind(id)
                .bind(id)
                .execute(&state.db)
                .await
                .unwrap();
            sqlx::query("INSERT INTO sessions(id,user_id,secret_hash,csrf_hash,expires_at) VALUES(?,?,?,?,?)")
                .bind(id).bind(id).bind(crate::auth::hash(id)).bind("test-csrf").bind(now()+3600).execute(&state.db).await.unwrap();
        }
        sqlx::query("INSERT INTO channels(id,name,kind) VALUES('general','General','text'),('dm','DM','dm')").execute(&state.db).await.unwrap();
        for id in ["alice", "bob"] {
            sqlx::query("INSERT INTO channel_members(channel_id,user_id) VALUES('dm',?)")
                .bind(id)
                .execute(&state.db)
                .await
                .unwrap();
        }
        Self {
            state,
            dir,
            requests,
            server,
            den: None,
        }
    }
    async fn auth(&self, id: &str) -> crate::auth::Auth {
        crate::auth::Auth {
            user: crate::auth::user(&self.state, id)
                .await
                .unwrap_or_else(|_| panic!("fixture user missing")),
            credential: id.into(),
            session: true,
        }
    }
    async fn register(&self, id: &str, token: &str) -> Device {
        crate::devices::register(
            State(self.state.clone()),
            self.auth(id).await,
            ApiJson(RegisterDevice {
                platform: DevicePlatform::Ios,
                token: token.into(),
                app_version: "1.0".into(),
                purpose: DevicePurpose::Alert,
                environment: None,
                client_id: None,
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("registration failed: {}", e.1))
        .0
    }
    async fn message(&self, channel: &str, content: &str) -> Message {
        crate::messages::send(
            State(self.state.clone()),
            self.auth("alice").await,
            axum::extract::Path(channel.into()),
            ApiJson(CreateMessage {
                content: content.into(),
                reply_to: None,
                upload_ids: vec![],
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("message failed: {}", e.1))
        .0
    }
    async fn receive(&mut self) -> Captured {
        tokio::time::timeout(Duration::from_secs(5), self.requests.recv())
            .await
            .expect("APNs request timed out")
            .expect("stub stopped")
    }
    async fn redeem(&mut self, ticket: &str) -> reqwest::Response {
        if self.den.is_none() {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let app = crate::router(self.state.clone());
            let task = tokio::spawn(async move {
                axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await
                .unwrap();
            });
            self.den = Some((url, task));
        }
        reqwest::Client::new()
            .post(format!(
                "{}/calls/invitations/redeem",
                self.den.as_ref().unwrap().0
            ))
            .json(&json!({"ticket":ticket}))
            .send()
            .await
            .unwrap()
    }
    async fn count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM devices")
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }
    async fn wait_count(&self, count: i64) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while self.count().await != count {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("device cleanup did not finish");
    }
}

#[tokio::test]
async fn real_notification_path_encodes_http2_signed_headers_deep_links_badge_and_preferences() {
    let mut f = Fixture::new(Environment::Sandbox).await;
    f.register("bob", "aabb01").await;
    f.message("general", "quiet ordinary message").await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), f.requests.recv())
            .await
            .is_err()
    );
    let message = f.message("general", "hello @bob").await;
    let request = f.receive().await;
    assert_eq!(request.version, Version::HTTP_2);
    assert_eq!(request.path, "/3/device/aabb01");
    assert_eq!(request.headers["apns-topic"], TOPIC);
    assert_eq!(request.headers["apns-push-type"], "alert");
    assert_eq!(request.headers["apns-collapse-id"], "general");
    assert_eq!(request.headers["apns-priority"], "10");
    assert_eq!(request.body["message_id"], message.id);
    assert_eq!(request.body["channel_id"], "general");
    assert_eq!(request.body["reason"], "mention");
    assert_eq!(request.body["aps"]["badge"], 1);
    let jwt = request.headers["authorization"]
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap()
        .to_owned();
    #[derive(Deserialize)]
    struct Claims {
        iss: String,
        iat: i64,
    }
    let mut validation = jsonwebtoken::Validation::new(Algorithm::ES256);
    validation.set_required_spec_claims(&["iss", "iat"]);
    validation.validate_exp = false;
    validation.set_issuer(&["test-team"]);
    let decoded = jsonwebtoken::decode::<Claims>(
        &jwt,
        &jsonwebtoken::DecodingKey::from_ec_pem(PUBLIC).unwrap(),
        &validation,
    )
    .unwrap();
    assert_eq!(decoded.header.kid.as_deref(), Some("test-key-id"));
    assert_eq!(decoded.claims.iss, "test-team");
    assert!((now() - decoded.claims.iat).abs() < 10);
    request.reply.send((StatusCode::OK, json!({}))).unwrap();
    f.message("dm", "DM with attachment coming later").await;
    let dm = f.receive().await;
    assert_eq!(dm.body["reason"], "dm");
    assert_eq!(dm.body["aps"]["badge"], 2);
    assert_eq!(dm.headers["authorization"], format!("Bearer {jwt}"));
    dm.reply.send((StatusCode::OK, json!({}))).unwrap();
    sqlx::query("INSERT INTO notification_preferences(user_id,mentions,dms) VALUES('bob',0,0)")
        .execute(&f.state.db)
        .await
        .unwrap();
    f.message("dm", "@bob preferences suppress DM and mention")
        .await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), f.requests.recv())
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO channel_subscriptions(user_id,channel_id) VALUES('bob','general')")
        .execute(&f.state.db)
        .await
        .unwrap();
    f.message("general", "followed channel").await;
    let followed = f.receive().await;
    assert_eq!(followed.body["reason"], "subscribed_channel");
    followed.reply.send((StatusCode::OK, json!({}))).unwrap();
}

#[tokio::test]
async fn invalid_tokens_are_removed_but_transient_errors_and_fresh_registrations_survive() {
    let mut f = Fixture::new(Environment::Production).await;
    for (status, reason) in [
        (410, "Unregistered"),
        (400, "BadDeviceToken"),
        (400, "DeviceTokenNotForTopic"),
    ] {
        f.register("bob", "aa01").await;
        f.message("dm", "invalid destination").await;
        let r = f.receive().await;
        r.reply
            .send((
                StatusCode::from_u16(status).unwrap(),
                json!({"reason":reason,"timestamp":now_millis()}),
            ))
            .unwrap();
        f.wait_count(0).await;
    }
    f.register("bob", "aa01").await;
    f.message("dm", "temporary outage").await;
    f.receive()
        .await
        .reply
        .send((
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"reason":"ServiceUnavailable"}),
        ))
        .unwrap();
    // The next request proves the worker finished the rejected request and kept the token.
    f.message("dm", "retry as a new notification").await;
    let r = f.receive().await;
    assert_eq!(f.count().await, 1);
    // Re-register while the rejection is in flight; the old generation must not delete it.
    f.register("bob", "aa01").await;
    r.reply
        .send((
            StatusCode::GONE,
            json!({"reason":"Unregistered","timestamp":now_millis()}),
        ))
        .unwrap();
    f.message("dm", "new registration survives").await;
    let r = f.receive().await;
    assert_eq!(f.count().await, 1);
    r.reply
        .send((
            StatusCode::GONE,
            json!({"reason":"Unregistered","timestamp":1}),
        ))
        .unwrap();
    f.message("dm", "older invalidation timestamp also survives")
        .await;
    f.receive()
        .await
        .reply
        .send((StatusCode::OK, json!({})))
        .unwrap();
    assert_eq!(f.count().await, 1);
}

#[tokio::test]
async fn queue_rechecks_account_ownership_logout_environment_and_invitation_decline() {
    let mut f = Fixture::new(Environment::Sandbox).await;
    f.register("bob", "ab01").await;
    f.message("dm", "occupy sender").await;
    let held = f.receive().await;
    f.message("dm", "old account queued notification").await;
    f.register("alice", "ab01").await;
    held.reply.send((StatusCode::OK, json!({}))).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), f.requests.recv())
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM devices")
        .execute(&f.state.db)
        .await
        .unwrap();
    f.register("bob", "ab02").await;
    sqlx::query("UPDATE devices SET environment='production'")
        .execute(&f.state.db)
        .await
        .unwrap();
    f.message("dm", "registered production destination").await;
    let delivered = f.receive().await;
    assert_eq!(delivered.path, "/production/3/device/ab02");
    delivered.reply.send((StatusCode::OK, json!({}))).unwrap();
    sqlx::query("UPDATE devices SET environment='sandbox'")
        .execute(&f.state.db)
        .await
        .unwrap();
    f.message("dm", "hold sender for decline").await;
    let held = f.receive().await;
    f.state
        .calls
        .lock()
        .await
        .entry("dm".into())
        .or_default()
        .insert("alice:phone".into(), "PA_alice".into());
    let invite = crate::invitations::invite(
        State(f.state.clone()),
        f.auth("alice").await,
        axum::extract::Path("dm".into()),
    )
    .await
    .unwrap_or_else(|_| panic!("invite failed"))
    .0;
    crate::invitations::decline(
        State(f.state.clone()),
        f.auth("bob").await,
        axum::extract::Path("dm".into()),
        ApiJson(DeclineCallInvitation {
            invitation_id: Some(invite.id.clone()),
            from_user_id: invite.from_user_id,
            expires_at: invite.expires_at,
        }),
    )
    .await
    .unwrap_or_else(|_| panic!("decline failed"));
    held.reply.send((StatusCode::OK, json!({}))).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), f.requests.recv())
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM sessions WHERE id='bob'")
        .execute(&f.state.db)
        .await
        .unwrap();
    f.message("dm", "logged out destination").await;
    assert_eq!(f.count().await, 0);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), f.requests.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn call_invitation_alert_has_future_voip_fields_and_expires_in_45_seconds() {
    let mut f = Fixture::new(Environment::Sandbox).await;
    f.register("bob", "ab03").await;
    f.state
        .calls
        .lock()
        .await
        .entry("dm".into())
        .or_default()
        .insert("alice:phone".into(), "PA_alice".into());
    let invite = crate::invitations::invite(
        State(f.state.clone()),
        f.auth("alice").await,
        axum::extract::Path("dm".into()),
    )
    .await
    .unwrap_or_else(|_| panic!("invite failed"))
    .0;
    let r = f.receive().await;
    assert_eq!(r.body["type"], "call_invite");
    assert_eq!(r.body["channel_id"], "dm");
    assert_eq!(r.body["from_user_id"], "alice");
    assert_eq!(r.body["expires_at"], invite.expires_at);
    assert_eq!(r.headers["apns-expiration"], invite.expires_at.to_string());
    assert_eq!(r.headers["apns-push-type"], "alert");
    r.reply.send((StatusCode::OK, json!({}))).unwrap();
}

#[tokio::test]
async fn pushkit_sends_one_state_ticket_without_duplicate_alert_or_media_credentials() {
    let mut f = Fixture::new(Environment::Sandbox).await;
    let client_id = "7c57e315-034c-45b2-8faa-b5696cafc4db";
    for (token, purpose, environment, client) in [
        ("ab04", DevicePurpose::Voip, Environment::Sandbox, client_id),
        (
            "ab05",
            DevicePurpose::Alert,
            Environment::Sandbox,
            client_id,
        ),
        (
            "ab06",
            DevicePurpose::Alert,
            Environment::Production,
            "25450a80-b211-4697-a789-00840b4d38f8",
        ),
    ] {
        let _ = crate::devices::register(
            State(f.state.clone()),
            f.auth("bob").await,
            ApiJson(RegisterDevice {
                platform: DevicePlatform::Ios,
                token: token.into(),
                app_version: "1".into(),
                purpose,
                environment: Some(environment),
                client_id: Some(client.into()),
            }),
        )
        .await
        .unwrap_or_else(|_| panic!("registration failed"));
    }
    f.state
        .calls
        .lock()
        .await
        .entry("dm".into())
        .or_default()
        .insert("alice:phone".into(), "PA_alice".into());
    let invite = crate::invitations::invite(
        State(f.state.clone()),
        f.auth("alice").await,
        axum::extract::Path("dm".into()),
    )
    .await
    .unwrap_or_else(|_| panic!("invite failed"))
    .0;
    let voip = f.receive().await;
    assert_eq!(voip.path, "/3/device/ab04");
    assert_eq!(voip.headers["apns-push-type"], "voip");
    assert_eq!(voip.headers["apns-topic"], "app.denchat.ios.voip");
    assert_eq!(voip.headers["apns-priority"], "10");
    assert_eq!(voip.headers["apns-expiration"], "0");
    assert!(voip.headers.get("apns-collapse-id").is_none());
    assert_eq!(voip.body["aps"], json!({}));
    assert_eq!(voip.body["invitation_id"], invite.id);
    assert_eq!(voip.body["from_display_name"], "alice");
    assert!(voip.body.get("token").is_none());
    assert!(voip.body.get("livekit_token").is_none());
    assert!(voip.body["fetch_ticket"]
        .as_str()
        .is_some_and(|s| s.len() >= 32));
    voip.reply.send((StatusCode::OK, json!({}))).unwrap();
    let fallback = f.receive().await;
    assert_eq!(
        fallback.path, "/production/3/device/ab06",
        "Same-installation alert must be suppressed; other environment still delivers"
    );
    assert_eq!(fallback.headers["apns-push-type"], "alert");
    fallback.reply.send((StatusCode::OK, json!({}))).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), f.requests.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn voip_ticket_is_one_use_state_only_and_invalidated_by_rotation_and_logout() {
    let mut f = Fixture::new(Environment::Sandbox).await;
    let registration = RegisterDevice {
        platform: DevicePlatform::Ios,
        token: "f001".into(),
        app_version: "1".into(),
        purpose: DevicePurpose::Voip,
        environment: None,
        client_id: Some("c7853376-bc49-427d-952c-c3018318116a".into()),
    };
    let _ = crate::devices::register(
        State(f.state.clone()),
        f.auth("bob").await,
        ApiJson(registration.clone()),
    )
    .await
    .unwrap_or_else(|_| panic!("registration failed"));
    f.state
        .calls
        .lock()
        .await
        .entry("dm".into())
        .or_default()
        .insert("alice:phone".into(), "PA_alice".into());
    async fn incoming(f: &mut Fixture) -> (CallInvitation, String) {
        let invite = crate::invitations::invite(
            State(f.state.clone()),
            f.auth("alice").await,
            axum::extract::Path("dm".into()),
        )
        .await
        .unwrap_or_else(|_| panic!("invite failed"))
        .0;
        let push = f.receive().await;
        let ticket = push.body["fetch_ticket"].as_str().unwrap().to_owned();
        push.reply.send((StatusCode::OK, json!({}))).unwrap();
        (invite, ticket)
    }
    async fn cancel(f: &Fixture, invite: &CallInvitation) {
        crate::invitations::cancel(
            State(f.state.clone()),
            f.auth("alice").await,
            axum::extract::Path("dm".into()),
            ApiJson(IdentifyCallInvitation {
                invitation_id: invite.id.clone(),
            }),
        )
        .await
        .unwrap_or_else(|_| panic!("cancel failed"));
    }
    let (invite, ticket) = incoming(&mut f).await;
    assert_eq!(f.redeem("not-a-ticket").await.status(), 410);
    let base = f.den.as_ref().unwrap().0.clone();
    let client = reqwest::Client::new();
    let (first, second) = tokio::join!(
        client
            .post(format!("{base}/calls/invitations/redeem"))
            .json(&json!({"ticket":ticket}))
            .send(),
        client
            .post(format!("{base}/calls/invitations/redeem"))
            .json(&json!({"ticket":ticket}))
            .send()
    );
    let (first, second) = (first.unwrap(), second.unwrap());
    assert!(matches!(
        (first.status().as_u16(), second.status().as_u16()),
        (200, 410) | (410, 200)
    ));
    let response = if first.status() == 200 { first } else { second };
    assert_eq!(response.headers()["cache-control"], "no-store");
    let state: Value = response.json().await.unwrap();
    assert_eq!(state["invitation"]["id"], invite.id);
    assert_eq!(state["state"], "ringing");
    assert!(state.get("token").is_none());
    assert!(state.get("url").is_none());
    assert_eq!(f.redeem(&ticket).await.status(), 410);
    cancel(&f, &invite).await;
    let (invite, ticket) = incoming(&mut f).await;
    let _ = crate::devices::register(
        State(f.state.clone()),
        f.auth("bob").await,
        ApiJson(registration),
    )
    .await
    .unwrap_or_else(|_| panic!("rotation failed"));
    assert_eq!(f.redeem(&ticket).await.status(), 410);
    cancel(&f, &invite).await;
    let (invite, ticket) = incoming(&mut f).await;
    // Advance only this persisted capability's deadline, keeping its registration live.
    sqlx::query("UPDATE call_fetch_tickets SET expires_at=? WHERE invitation_id=?")
        .bind(now() - 1)
        .bind(&invite.id)
        .execute(&f.state.db)
        .await
        .unwrap();
    assert_eq!(f.redeem(&ticket).await.status(), 410);
    cancel(&f, &invite).await;
    let (_, ticket) = incoming(&mut f).await;
    let base = f.den.as_ref().unwrap().0.clone();
    assert_eq!(
        reqwest::Client::new()
            .post(format!("{base}/auth/logout"))
            .bearer_auth("bob")
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(f.redeem(&ticket).await.status(), 410);
}
