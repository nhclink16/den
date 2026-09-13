use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    routing::post,
    Json, Router,
};
use den_core::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Clone)]
struct Mock {
    url: String,
    connections: Arc<AtomicUsize>,
    done: tokio::sync::mpsc::Sender<()>,
}
async fn login(State(s): State<Mock>, Json(v): Json<HostLogin>) -> Json<HostCredential> {
    assert_eq!(v.code, "one-time");
    Json(HostCredential {
        server_url: s.url,
        host_id: "host".into(),
        name: v.name,
        token: "test-only-host-token".into(),
    })
}
async fn ws(
    State(s): State<Mock>,
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    assert_eq!(headers["authorization"], "Bearer test-only-host-token");
    ws.on_upgrade(move |mut socket| async move {
        let n = s.connections.fetch_add(1, Ordering::SeqCst);
        let open = HostFrame::Open {
            session_id: "session".into(),
            cols: 80,
            rows: 24,
            shell: Some("/bin/sh".into()),
        };
        socket
            .send(Message::Binary(serde_json::to_vec(&open).unwrap().into()))
            .await
            .unwrap();
        let command = if n == 0 {
            "stty -echo; X=SOCKET_PERSIST; printf 'SOCKET_%s\\n' OK\n"
        } else {
            "printf '%s\\n' \"$X\"\n"
        };
        socket
            .send(Message::Binary(
                serde_json::to_vec(&HostFrame::Input {
                    session_id: "session".into(),
                    bytes: command.as_bytes().to_vec(),
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        let mut out = String::new();
        while let Some(Ok(Message::Binary(b))) = socket.recv().await {
            if let HostFrame::Output { session_id, bytes } = serde_json::from_slice(&b).unwrap() {
                out.push_str(&String::from_utf8_lossy(&bytes));
                socket
                    .send(Message::Binary(
                        serde_json::to_vec(&HostFrame::Ack {
                            session_id,
                            bytes: bytes.len(),
                        })
                        .unwrap()
                        .into(),
                    ))
                    .await
                    .unwrap();
                if out.contains(if n == 0 {
                    "SOCKET_OK"
                } else {
                    "SOCKET_PERSIST"
                }) {
                    if n > 0 {
                        s.done.send(()).await.unwrap();
                    }
                    break;
                }
            }
        }
    })
}
#[tokio::test]
async fn binary_login_mode_and_reconnect_keep_the_shell() {
    let dir = std::env::temp_dir().join(format!("den-host-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (done, mut received) = tokio::sync::mpsc::channel(1);
    let app = Router::new()
        .route("/hosts/login", post(login))
        .route("/hosts/ws", axum::routing::get(ws))
        .with_state(Mock {
            url: url.clone(),
            connections: Arc::new(AtomicUsize::new(0)),
            done,
        });
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let status = tokio::process::Command::new(env!("CARGO_BIN_EXE_den-host"))
        .env("DEN_HOST_CONFIG_DIR", &dir)
        .args(["login", &format!("{url}#one-time")])
        .status()
        .await
        .unwrap();
    assert!(status.success());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.join("host.toml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_den-host"))
        .env("DEN_HOST_CONFIG_DIR", &dir)
        .arg("run")
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(12), received.recv()).await;
    child.kill().await.unwrap();
    server.abort();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(matches!(result, Ok(Some(()))));
}
