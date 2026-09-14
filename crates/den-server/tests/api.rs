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
        let state = if configured {
            state.with_livekit(
                "ws://127.0.0.1:1".into(),
                "test-key".into(),
                "test-secret-at-least-thirty-two-bytes".into(),
            )
        } else {
            state
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
    fn req(&self, method: Method, path: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.url))
            .bearer_auth(token)
    }
    async fn post(&self, path: &str, token: &str, body: Value) -> Value {
        let r = self
            .req(Method::POST, path, token)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200, "{path}");
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
#[path = "api/auth.rs"]
mod auth;
#[path = "api/chat.rs"]
mod chat;
#[path = "api/realtime.rs"]
mod realtime;
#[path = "api/uploads.rs"]
mod uploads;

#[path = "api/m2_live.rs"]
mod m2_live;
#[path = "api/m2_messages.rs"]
mod m2_messages;

#[path = "api/m2_files.rs"]
mod m2_files;

#[path = "api/calls.rs"]
mod calls;

#[path = "api/objects.rs"]
mod objects;
#[path = "api/portable.rs"]
mod portable;

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
