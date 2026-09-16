use super::*;
use den_host::pty::Sessions;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub(super) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A real den-host: real portable-pty sessions driven by the server's real frames
/// over a real WebSocket. Shared with the thread card tests so they exercise an
/// actual terminal rather than a stand-in.
pub(super) struct Host {
    pub(super) credential: Value,
    sessions: Sessions,
}
impl Host {
    pub(super) async fn new(t: &Test) -> Self {
        let enrollment = t.post("/hosts/enroll", &t.admin.token, json!({})).await;
        let code = enrollment["code"]
            .as_str()
            .unwrap()
            .rsplit_once('#')
            .unwrap()
            .1;
        let credential = t
            .post("/hosts/login", "", json!({"code":code,"name":"reconcile"}))
            .await;
        Self {
            credential,
            sessions: Sessions::default(),
        }
    }
    pub(super) async fn connect(&self, t: &Test) -> Socket {
        let mut request = format!("{}/hosts/ws", t.url.replace("http", "ws"))
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {}", self.credential["token"].as_str().unwrap())
                .parse()
                .unwrap(),
        );
        connect_async(request).await.unwrap().0
    }
    pub(super) async fn hello(&mut self, socket: &mut Socket, ids: Option<Vec<String>>) {
        let mut hello = json!({"type":"hello","direct_url":null});
        if let Some(ids) = ids {
            hello["session_ids"] = json!(ids);
        }
        socket
            .send(Frame::Binary(serde_json::to_vec(&hello).unwrap().into()))
            .await
            .unwrap();
        self.flush(socket).await;
    }
    /// Which sessions this host actually has open. The server's view is not
    /// evidence; this is the host's own inventory.
    pub(super) fn ids(&self) -> Vec<String> {
        let mut ids = self.sessions.ids();
        ids.sort();
        ids
    }
    /// Drain until the host has applied the Close for this session. A Ping/Pong
    /// barrier only orders frames the server sent inline with the hello
    /// reconciliation; a Close queued by a separate HTTP request is not covered by
    /// it, so waiting for the frame itself is the only reliable ordering.
    pub(super) async fn closed(&mut self, socket: &mut Socket, session: &str) {
        self.receive_until(socket, |frame| matches!(frame, Frame::Binary(bytes) if matches!(serde_json::from_slice::<HostFrame>(bytes).unwrap(), HostFrame::Close{session_id} if session_id == session))).await;
    }
    // A WebSocket ping gives an ordered barrier after the preceding inventory.
    // Apply the real server's frames to real portable-pty sessions.
    pub(super) async fn flush(&mut self, socket: &mut Socket) {
        socket
            .send(Frame::Ping(b"barrier".to_vec().into()))
            .await
            .unwrap();
        self.receive_until(
            socket,
            |frame| matches!(frame, Frame::Pong(bytes) if bytes.as_ref() == b"barrier"),
        )
        .await;
    }
    pub(super) async fn receive_until(
        &mut self,
        socket: &mut Socket,
        done: impl Fn(&Frame) -> bool,
    ) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let frame = socket.next().await.unwrap().unwrap();
                let complete = done(&frame);
                match frame {
                    Frame::Binary(bytes) => {
                        let mut frame: HostFrame = serde_json::from_slice(&bytes).unwrap();
                        if let HostFrame::Open { shell, .. } = &mut frame {
                            *shell = Some("/bin/sh".into());
                        }
                        self.sessions.handle(frame).unwrap();
                    }
                    Frame::Pong(_) => (),
                    Frame::Ping(_) => socket.flush().await.unwrap(),
                    other => panic!("unexpected host response: {other:?}"),
                }
                if complete {
                    break;
                }
            }
        })
        .await
        .expect("host barrier");
    }
    pub(super) async fn open(&mut self, t: &Test, socket: &mut Socket) -> String {
        self.open_with(t, socket, json!({})).await.0
    }
    /// Open a session with an explicit request body, so a card can carry task or
    /// thread context. Returns the session ID and the card object.
    pub(super) async fn open_with(
        &mut self,
        t: &Test,
        socket: &mut Socket,
        body: Value,
    ) -> (String, Value) {
        let object = t
            .post(
                &format!(
                    "/hosts/{}/sessions",
                    self.credential["host_id"].as_str().unwrap()
                ),
                &t.admin.token,
                body,
            )
            .await;
        let id = object["id"].as_str().unwrap().to_owned();
        let id = id.as_str();
        self.receive_until(socket, |frame| matches!(frame, Frame::Binary(bytes) if matches!(serde_json::from_slice::<HostFrame>(bytes).unwrap(), HostFrame::Open{session_id,..} if session_id == id))).await;
        (id.to_owned(), object.clone())
    }
    pub(super) async fn command(&mut self, t: &Test, socket: &mut Socket, id: &str, command: &str) {
        let response = t
            .req(
                Method::POST,
                &format!("/sessions/{id}/write"),
                &t.admin.token,
            )
            .json(&json!({"text":command}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 204);
        self.receive_until(socket, |frame| matches!(frame, Frame::Binary(bytes) if matches!(serde_json::from_slice::<HostFrame>(bytes).unwrap(), HostFrame::Input{session_id,bytes} if session_id == id && bytes == command.as_bytes()))).await;
    }
    pub(super) async fn pid(&mut self, t: &Test, socket: &mut Socket, id: &str) -> u32 {
        let path = t.dir.join(format!("{id}.pid"));
        self.command(
            t,
            socket,
            id,
            &format!(
                "stty -echo; KEEP=original; echo $$ > '{}'\n",
                path.display()
            ),
        )
        .await;
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(value) = tokio::fs::read_to_string(&path).await {
                    if let Ok(pid) = value.trim().parse() {
                        break pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
}

pub(super) fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}
pub(super) async fn stopped(pid: u32) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while alive(pid) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("original shell must terminate");
}
async fn online(t: &Test, expected: bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hosts: Value = t
                .req(Method::GET, "/hosts", &t.admin.token)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if hosts[0]["online"] == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn ended(t: &Test, id: &str) -> bool {
    let state: String = sqlx::query_scalar("SELECT state FROM objects WHERE id=?")
        .bind(id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    !serde_json::from_str::<Value>(&state).unwrap()["terminal"]["ended_at"].is_null()
}

#[tokio::test]
async fn inventory_closes_retired_ptys_and_preserves_the_surviving_shell() {
    let t = Test::new().await;
    let mut host = Host::new(&t).await;
    let mut socket = host.connect(&t).await;
    host.hello(&mut socket, Some(vec![])).await;
    let keep = host.open(&t, &mut socket).await;
    let close = host.open(&t, &mut socket).await;
    let missing = host.open(&t, &mut socket).await;
    let keep_pid = host.pid(&t, &mut socket, &keep).await;
    let close_pid = host.pid(&t, &mut socket, &close).await;
    let missing_pid = host.pid(&t, &mut socket, &missing).await;
    socket.close(None).await.unwrap();
    drop(socket);
    online(&t, false).await;
    let response = t
        .req(
            Method::DELETE,
            &format!("/sessions/{close}"),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    assert!(ended(&t, &close).await);
    sqlx::query("DELETE FROM objects WHERE id=?")
        .bind(&missing)
        .execute(&t.state.db)
        .await
        .unwrap();
    assert!(alive(close_pid) && alive(missing_pid));

    let mut socket = host.connect(&t).await;
    host.hello(
        &mut socket,
        Some(vec![keep.clone(), close.clone(), missing.clone()]),
    )
    .await;
    stopped(close_pid).await;
    stopped(missing_pid).await;
    assert!(alive(keep_pid));
    assert!(host.sessions.dimensions(&close).is_none());
    assert!(host.sessions.dimensions(&missing).is_none());
    let proof = t.dir.join("survivor");
    host.command(
        &t,
        &mut socket,
        &keep,
        &format!("echo \"$$:$KEEP\" > '{}'\n", proof.display()),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while tokio::fs::read_to_string(&proof)
            .await
            .unwrap_or_default()
            .trim()
            != format!("{keep_pid}:original")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("same shell and variable after reconnect");

    // A connected host must also converge without waiting for another reconnect.
    sqlx::query("DELETE FROM objects WHERE id=?")
        .bind(&keep)
        .execute(&t.state.db)
        .await
        .unwrap();
    host.hello(&mut socket, Some(vec![keep.clone()])).await;
    stopped(keep_pid).await;
}

#[tokio::test]
async fn inventory_crossing_open_keeps_it_and_lost_exit_does_not_respawn() {
    let t = Test::new().await;
    let mut host = Host::new(&t).await;
    let mut socket = host.connect(&t).await;
    host.hello(&mut socket, Some(vec![])).await;
    // The Open is issued, but the host's inventory was captured before receiving it.
    let object = t
        .post(
            &format!(
                "/hosts/{}/sessions",
                host.credential["host_id"].as_str().unwrap()
            ),
            &t.admin.token,
            json!({}),
        )
        .await;
    let id = object["id"].as_str().unwrap();
    host.hello(&mut socket, Some(vec![])).await;
    assert!(!ended(&t, id).await, "periodic absence is not an exit");
    let pid = host.pid(&t, &mut socket, id).await;
    host.command(&t, &mut socket, id, "exit 7\n").await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if host
                .sessions
                .drain()
                .iter()
                .any(|f| matches!(f, HostFrame::Exited{session_id,code:7} if session_id==id))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // Deliberately lose Exited between drain and transport delivery.
    stopped(pid).await;
    socket.close(None).await.unwrap();
    drop(socket);
    online(&t, false).await;
    assert!(!ended(&t, id).await);
    let mut socket = host.connect(&t).await;
    host.hello(&mut socket, Some(vec![])).await;
    assert!(
        ended(&t, id).await,
        "initial inventory reconciles the lost exit"
    );
    assert!(
        host.sessions.dimensions(id).is_none(),
        "reconnect must not create a replacement shell"
    );
}

#[tokio::test]
async fn legacy_hello_is_not_empty_and_failed_reads_never_authorize_closes() {
    let t = Test::new().await;
    let mut host = Host::new(&t).await;
    let mut socket = host.connect(&t).await;
    host.hello(&mut socket, None).await;
    let id = host.open(&t, &mut socket).await;
    let pid = host.pid(&t, &mut socket, &id).await;
    socket.close(None).await.unwrap();
    drop(socket);
    online(&t, false).await;
    let mut socket = host.connect(&t).await;
    let response = t
        .req(
            Method::POST,
            &format!(
                "/hosts/{}/sessions",
                host.credential["host_id"].as_str().unwrap()
            ),
            &t.admin.token,
        )
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        409,
        "host is unavailable until the handshake completes"
    );
    host.hello(&mut socket, None).await;
    assert!(
        !ended(&t, &id).await,
        "legacy absence must not end a session"
    );
    assert!(alive(pid));

    sqlx::query("ALTER TABLE terminal_sessions RENAME TO temporarily_unavailable_sessions")
        .execute(&t.state.db)
        .await
        .unwrap();
    refuses_closes(&host, &t, &mut socket, &id).await;
    assert!(alive(pid));
    sqlx::query("ALTER TABLE temporarily_unavailable_sessions RENAME TO terminal_sessions")
        .execute(&t.state.db)
        .await
        .unwrap();
    let mut socket = reconnects_cleanly(&mut host, &t, &id, pid).await;

    // A listed session whose own state will not parse is an unread inventory
    // too, not an inactive one. Skipping it would retire a live PTY.
    let state: String = sqlx::query_scalar("SELECT state FROM objects WHERE id=?")
        .bind(&id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE objects SET state=? WHERE id=?")
        .bind(r#"{"terminal":"unreadable"}"#)
        .bind(&id)
        .execute(&t.state.db)
        .await
        .unwrap();
    refuses_closes(&host, &t, &mut socket, &id).await;
    assert!(alive(pid));
    sqlx::query("UPDATE objects SET state=? WHERE id=?")
        .bind(&state)
        .bind(&id)
        .execute(&t.state.db)
        .await
        .unwrap();
    reconnects_cleanly(&mut host, &t, &id, pid).await;
}

// Reconciliation that cannot read its own state must drop the socket for retry,
// both on a live connection and on the initial handshake, and issue no Close.
async fn refuses_closes(host: &Host, t: &Test, socket: &mut Socket, id: &str) {
    for initial in [false, true] {
        if initial {
            online(t, false).await;
            *socket = host.connect(t).await;
        }
        socket
            .send(Frame::Binary(
                serde_json::to_vec(&json!({"type":"hello","direct_url":null,"session_ids":[id]}))
                    .unwrap()
                    .into(),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(Ok(frame)) = socket.next().await {
                match frame {
                    Frame::Binary(bytes) => assert!(
                        !matches!(
                            serde_json::from_slice::<HostFrame>(&bytes).unwrap(),
                            HostFrame::Close { .. }
                        ),
                        "failed database read must not retire a PTY"
                    ),
                    Frame::Ping(_) => socket.flush().await.unwrap(),
                    Frame::Close(_) => break,
                    _ => (),
                }
            }
        })
        .await
        .expect("failed reconciliation disconnects without issuing closes");
    }
}

async fn reconnects_cleanly(host: &mut Host, t: &Test, id: &str, pid: u32) -> Socket {
    online(t, false).await;
    let mut socket = host.connect(t).await;
    host.hello(&mut socket, Some(vec![id.to_owned()])).await;
    assert!(
        !ended(t, id).await,
        "a readable inventory keeps the session"
    );
    assert!(alive(pid));
    socket
}
