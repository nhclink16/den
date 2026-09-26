use den_core::*;
use den_server::AppState;
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message as Frame},
};

/// `From<sqlx::Error>` answers 500 with a generic body and logs the real error at
/// ERROR. Install one subscriber for the binary so a failing test's captured
/// output carries that line.
fn trace() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::ERROR)
            .with_test_writer()
            .try_init();
    });
}
struct Test {
    url: String,
    dir: PathBuf,
    state: AppState,
    task: tokio::task::JoinHandle<()>,
    http: Client,
    admin: Session,
}
impl Drop for Test {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
impl Test {
    async fn new() -> Self {
        Self::with_voice(false).await
    }
    async fn with_voice(configured: bool) -> Self {
        Self::with_voice_at(configured.then_some("ws://127.0.0.1:1")).await
    }
    async fn with_voice_at(voice: Option<&str>) -> Self {
        Self::configured(voice, None).await
    }
    async fn configured(voice: Option<&str>, music: Option<String>) -> Self {
        Self::with_music(voice, music.map(|r| (r, "ffmpeg".into(), "den-dj".into()))).await
    }
    async fn with_music(voice: Option<&str>, music: Option<(String, String, String)>) -> Self {
        Self::built(voice, music, None).await
    }
    /// A server that holds a Spotify client secret. Tests never read process
    /// environment, and the real secret is not on any developer machine.
    async fn with_spotify() -> Self {
        Self::built(None, None, Some((None, Duration::from_secs(10)))).await
    }
    async fn with_spotify_provider(base_url: String, timeout: Duration) -> Self {
        Self::built(None, None, Some((Some(base_url), timeout))).await
    }
    async fn built(
        voice: Option<&str>,
        music: Option<(String, String, String)>,
        spotify: Option<(Option<String>, Duration)>,
    ) -> Self {
        trace();
        let dir = std::env::temp_dir().join(format!("den-test-{}", ulid::Ulid::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = AppState::open(
            dir.join("den.db"),
            dir.join("uploads"),
            dir.join("bootstrap.key"),
            url.clone(),
            1024 * 1024,
        )
        .await
        .unwrap();
        let state = if let Some(voice) = voice {
            state.with_livekit(
                voice.into(),
                "test-key".into(),
                "test-secret-at-least-thirty-two-bytes".into(),
            )
        } else {
            state
        };
        let state = if let Some((resolver, ffmpeg, publisher)) = music {
            state.with_music_tools(resolver, ffmpeg, publisher)
        } else {
            state
        };
        let state = match spotify {
            Some((Some(base_url), timeout)) => state
                .with_spotify_test_endpoint(
                    "test-client-secret".into(),
                    [3u8; 32],
                    base_url,
                    timeout,
                )
                .unwrap(),
            Some((None, _)) => state
                .with_spotify("test-client-secret".into(), [3u8; 32])
                .unwrap(),
            None => state,
        };
        let app = den_server::router_with_web(state.clone(), dir.join("spa"));
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let http = Client::new();
        let key = std::fs::read_to_string(dir.join("bootstrap.key")).unwrap();
        let response = http
            .post(format!("{url}/auth/init"))
            .json(&Bootstrap {
                username: "admin".into(),
                password: "test-password-123".into(),
                bootstrap_token: key,
                display_name: None,
            })
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let admin = response.json().await.unwrap();
        Self {
            url,
            dir,
            state,
            task,
            http,
            admin,
        }
    }
    async fn stop_for_export(&mut self) {
        use sqlx::Connection;
        self.task.abort();
        let _ = (&mut self.task).await;
        self.state.db.close().await;
        // WAL files can remain after shutdown. Prove readers have released the
        // database by obtaining the same exclusive lock used by offline export.
        let mut db = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(self.dir.join("den.db"))
                .busy_timeout(Duration::from_secs(5)),
        )
        .await
        .unwrap();
        sqlx::query("PRAGMA locking_mode=EXCLUSIVE")
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("COMMIT").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
    }
    fn req(&self, method: Method, path: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.url))
            .bearer_auth(token)
    }
    async fn post(&self, path: &str, token: &str, body: Value) -> Value {
        self.post_phase("", path, token, body).await
    }
    /// `phase` distinguishes several posts to one path. A failure reports the
    /// status, the path and the decoded API error only: request bodies and
    /// successful auth payloads carry credentials and are never printed. The 500
    /// body is deliberately generic, so its cause arrives through the captured
    /// tracing line rather than from here.
    async fn post_phase(&self, phase: &str, path: &str, token: &str, body: Value) -> Value {
        let r = self
            .req(Method::POST, path, token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = r.status();
        if status != StatusCode::OK {
            let detail = match r.json::<ApiError>().await {
                Ok(e) => format!("{}: {}", e.error, e.message),
                Err(_) => "<response body was not an ApiError>".into(),
            };
            let at = if phase.is_empty() {
                String::new()
            } else {
                format!(" [{phase}]")
            };
            panic!("POST {path}{at} returned {} — {detail}", status.as_u16());
        }
        r.json().await.unwrap()
    }
    async fn member(&self, name: &str) -> Session {
        let invite = self
            .post(
                "/invites",
                &self.admin.token,
                json!({"uses":1,"expires_in_hours":1}),
            )
            .await;
        serde_json::from_value(
            self.post(
                "/auth/register",
                "",
                json!({"username":name,"password":"test-password-123","invite":invite["code"]}),
            )
            .await,
        )
        .unwrap()
    }
    async fn general(&self) -> String {
        let v: Vec<Channel> = self
            .req(Method::GET, "/channels", &self.admin.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        v.iter()
            .find(|c| c.kind == ChannelKind::Text)
            .unwrap()
            .id
            .clone()
    }
}
#[path = "api/activities.rs"]
mod activities;
#[path = "api/auth.rs"]
mod auth;
#[path = "api/chat.rs"]
mod chat;
#[path = "api/realtime.rs"]
mod realtime;
#[path = "api/uploads.rs"]
mod uploads;
#[path = "api/wallpapers.rs"]
mod wallpapers;

#[path = "api/m2_live.rs"]
mod m2_live;
#[path = "api/m2_messages.rs"]
mod m2_messages;

#[path = "api/m2_files.rs"]
mod m2_files;

#[path = "api/calls.rs"]
mod calls;
#[path = "api/ios.rs"]
mod ios;
#[path = "api/ios_calls.rs"]
mod ios_calls;

#[path = "api/objects.rs"]
mod objects;
#[path = "api/portable.rs"]
mod portable;
#[path = "api/thread_conversations.rs"]
mod thread_conversations;
#[path = "api/thread_objects.rs"]
mod thread_objects;
#[path = "api/thread_unread.rs"]
mod thread_unread;
#[path = "api/threads.rs"]
mod threads;

#[tokio::test]
async fn host_enrollment_is_single_use_and_expires() {
    let t = Test::new().await;
    let enrollment = t.post("/hosts/enroll", &t.admin.token, json!({})).await;
    let code = enrollment["code"]
        .as_str()
        .unwrap()
        .rsplit_once('#')
        .unwrap()
        .1;
    let login = t
        .post("/hosts/login", "", json!({"code":code,"name":"codexbox"}))
        .await;
    assert!(login["token"].as_str().unwrap().len() > 20);
    let response = t
        .req(Method::POST, "/hosts/login", "")
        .json(&json!({"code":code,"name":"again"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let enrollment = t.post("/hosts/enroll", &t.admin.token, json!({})).await;
    sqlx::query("UPDATE host_enrollments SET expires_at=0")
        .execute(&t.state.db)
        .await
        .unwrap();
    let response=t.req(Method::POST,"/hosts/login","").json(&json!({"code":enrollment["code"].as_str().unwrap().rsplit_once('#').unwrap().1,"name":"expired"})).send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn terminal_grants_enforce_view_control_revoke_expiry_and_privacy() {
    let t = Test::new().await;
    let bob = t.member("terminal_bob").await;
    let outsider = t.member("outsider").await;
    let enrollment = t.post("/hosts/enroll", &t.admin.token, json!({})).await;
    let h=t.post("/hosts/login","",json!({"code":enrollment["code"].as_str().unwrap().rsplit_once('#').unwrap().1,"name":"test-host"})).await;
    let host = h["host_id"].as_str().unwrap();
    let mut req = format!("{}/hosts/ws", t.url.replace("http", "ws"))
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "authorization",
        format!("Bearer {}", h["token"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    let (mut socket, _) = connect_async(req).await.unwrap();
    socket
        .send(Frame::Binary(
            serde_json::to_vec(&json!({"type":"hello","direct_url":null}))
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let o = t
        .post(
            &format!("/hosts/{host}/sessions"),
            &t.admin.token,
            json!({}),
        )
        .await;
    let id = o["id"].as_str().unwrap();
    let write = format!("/sessions/{id}/write");
    assert_eq!(
        t.req(Method::POST, &write, &t.admin.token)
            .json(&json!({"text":"owner"}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        t.req(Method::POST, &write, &outsider.token)
            .json(&json!({"text":"no"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let general = t.general().await;
    t.post(
        &format!("/sessions/{id}/share"),
        &t.admin.token,
        json!({"channel_id":general}),
    )
    .await;
    let request = t
        .post(
            &format!("/hosts/{host}/requests"),
            &bob.token,
            json!({"capability":"terminal_view","duration_minutes":60}),
        )
        .await;
    let view = t
        .post(
            &format!("/requests/{}/decide", request["id"].as_str().unwrap()),
            &t.admin.token,
            json!({"allow":true}),
        )
        .await;
    assert!(view["grant_id"].is_string());
    assert_eq!(
        t.req(Method::POST, &write, &bob.token)
            .json(&json!({"text":"view cannot type"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/sessions/{id}/controller"),
            &t.admin.token
        )
        .json(&json!({"user_id":bob.user.id}))
        .send()
        .await
        .unwrap()
        .status(),
        403
    );
    let request = t
        .post(
            &format!("/hosts/{host}/requests"),
            &bob.token,
            json!({"capability":"terminal_control","standing":true}),
        )
        .await;
    // A failure between writing the grant and recording its decision rolls back both.
    sqlx::query("CREATE TRIGGER fail_grant_audit BEFORE INSERT ON access_log WHEN NEW.action='grant' BEGIN SELECT RAISE(ABORT,'test audit failure'); END").execute(&t.state.db).await.unwrap();
    let failed = t
        .req(
            Method::POST,
            &format!("/requests/{}/decide", request["id"].as_str().unwrap()),
            &t.admin.token,
        )
        .json(&json!({"allow":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(failed.status(), 500);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM grants WHERE capability='terminal_control'"
        )
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0
    );
    sqlx::query("DROP TRIGGER fail_grant_audit")
        .execute(&t.state.db)
        .await
        .unwrap();
    let grant = t
        .post(
            &format!("/requests/{}/decide", request["id"].as_str().unwrap()),
            &t.admin.token,
            json!({"allow":true}),
        )
        .await;
    t.post(
        &format!("/sessions/{id}/controller"),
        &t.admin.token,
        json!({"user_id":bob.user.id}),
    )
    .await;
    assert_eq!(
        t.req(Method::POST, &write, &bob.token)
            .json(&json!({"text":"BOB_ALLOWED"}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    // The host must receive the allowed bytes, not merely an HTTP success.
    let found = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let frame = socket.next().await.unwrap().unwrap();
            if let Frame::Binary(b) = frame {
                if let HostFrame::Input { bytes, .. } = serde_json::from_slice(&b).unwrap() {
                    if bytes == b"BOB_ALLOWED" {
                        break;
                    }
                }
            }
        }
    })
    .await;
    assert!(found.is_ok());
    let direct = t
        .post(
            &format!("/sessions/{id}/direct-token"),
            &bob.token,
            json!({}),
        )
        .await;
    let check = json!({"token":direct["token"],"session_id":id,"input":true});
    assert_eq!(
        t.req(
            Method::POST,
            "/hosts/direct/check",
            h["token"].as_str().unwrap()
        )
        .json(&check)
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    let start = std::time::Instant::now();
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/grants/{}", grant["grant_id"].as_str().unwrap()),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    assert_eq!(
        t.req(Method::POST, &write, &bob.token)
            .json(&json!({"text":"BOB_REVOKED"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(
        t.req(
            Method::POST,
            "/hosts/direct/check",
            h["token"].as_str().unwrap()
        )
        .json(&check)
        .send()
        .await
        .unwrap()
        .status(),
        403
    );
    sqlx::query("UPDATE grants SET expires_at=0 WHERE grantee_id=?")
        .bind(&bob.user.id)
        .execute(&t.state.db)
        .await
        .unwrap();
    assert_eq!(
        t.req(Method::POST, &write, &bob.token)
            .json(&json!({"text":"expired"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    // Sharing the card does not permit generic object writes to escalate access.
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/objects/{id}/patch"),
            &t.admin.token
        )
        .json(&json!({"base_version":0,"put":[],"remove":[]}))
        .send()
        .await
        .unwrap()
        .status(),
        403
    );
    // Even after every grant expires, ownership still gives control.
    assert_eq!(
        t.req(Method::POST, &write, &t.admin.token)
            .json(&json!({"text":"owner again"}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
}

#[path = "api/appearance.rs"]
mod appearance;

#[path = "api/desktop.rs"]
mod desktop;

#[path = "api/backgrounds.rs"]
mod backgrounds;

#[path = "api/members.rs"]
mod members;
#[path = "api/profile_images.rs"]
mod profile_images;
#[path = "api/profiles.rs"]
mod profiles;
#[path = "api/terminal_recording.rs"]
mod terminal_recording;
#[path = "api/voice_preferences.rs"]
mod voice_preferences;

#[path = "api/jams.rs"]
mod jams;

#[cfg(unix)]
#[path = "api/music.rs"]
mod music;
#[path = "api/sounds.rs"]
mod sounds;
#[path = "api/sounds_ws.rs"]
mod sounds_ws;
#[path = "api/spotify.rs"]
mod spotify;

#[cfg(unix)]
#[path = "api/terminal_reconciliation.rs"]
mod terminal_reconciliation;
