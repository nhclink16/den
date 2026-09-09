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
        v[0].id.clone()
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
