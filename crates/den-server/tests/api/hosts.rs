use super::realtime::{socket, Socket};
use super::*;

struct Machine {
    id: String,
    token: String,
    socket: Socket,
}

/// Enroll a machine for `owner` and bring it online the way den-host does.
async fn machine(t: &Test, owner: &str, name: &str) -> Machine {
    let enrollment = t.post("/hosts/enroll", owner, json!({})).await;
    let code = enrollment["code"]
        .as_str()
        .unwrap()
        .rsplit_once('#')
        .unwrap()
        .1;
    let login = t
        .post("/hosts/login", "", json!({"code":code,"name":name}))
        .await;
    let token = login["token"].as_str().unwrap().to_string();
    let id = login["host_id"].as_str().unwrap().to_string();
    let socket = host_socket(t, &token).await.expect("host connects");
    for _ in 0..100 {
        let hosts: Vec<Host> = t
            .req(Method::GET, "/hosts", owner)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if hosts.iter().any(|h| h.id == id && h.online) {
            return Machine { id, token, socket };
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("machine never came online");
}
async fn host_socket(t: &Test, token: &str) -> Option<Socket> {
    let mut request = format!("{}/hosts/ws", t.url.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (mut socket, _) = connect_async(request).await.ok()?;
    socket
        .send(Frame::Binary(
            serde_json::to_vec(&json!({"type":"hello","direct_url":null}))
                .unwrap()
                .into(),
        ))
        .await
        .ok()?;
    Some(socket)
}
async fn status(t: &Test, method: Method, path: &str, token: &str, body: Value) -> u16 {
    t.req(method, path, token)
        .json(&body)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}
async fn get<T: serde::de::DeserializeOwned>(t: &Test, path: &str, token: &str) -> T {
    t.req(Method::GET, path, token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn grant(t: &Test, host: &str, grantee: &str, owner: &str, capability: &str) -> String {
    let request = t
        .post(
            &format!("/hosts/{host}/requests"),
            grantee,
            json!({"capability":capability,"standing":true}),
        )
        .await;
    let decided = t
        .post(
            &format!("/requests/{}/decide", request["id"].as_str().unwrap()),
            owner,
            json!({"allow":true}),
        )
        .await;
    decided["grant_id"].as_str().unwrap().to_string()
}

// A machine is its owner's. Only they decide requests or unenroll it, and only
// they and the grantee see or end a grant. Admins have no override here.
#[tokio::test]
async fn host_requests_and_grants_answer_only_the_owner_and_grantee() {
    let t = Test::new().await;
    let alice = t.member("host_alice").await;
    let bob = t.member("host_bob").await;
    let admin = t.admin.token.as_str();
    let m = machine(&t, &alice.token, "alice-box").await;
    let request = t
        .post(
            &format!("/hosts/{}/requests", m.id),
            &bob.token,
            json!({"capability":"terminal_control","duration_minutes":60}),
        )
        .await;
    let decide = format!("/requests/{}/decide", request["id"].as_str().unwrap());
    for token in [bob.token.as_str(), admin] {
        assert_eq!(
            status(&t, Method::POST, &decide, token, json!({"allow":true})).await,
            404
        );
    }
    let grants: i64 = sqlx::query_scalar("SELECT count(*) FROM grants")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(grants, 0, "a refused decision wrote a grant");
    // Bob's control request becomes a view grant through a fresh request.
    t.post(&decide, &alice.token, json!({"allow":false})).await;
    let gid = grant(&t, &m.id, &bob.token, &alice.token, "terminal_view").await;

    let theirs: Vec<Grant> = get(&t, "/grants", admin).await;
    assert!(theirs.is_empty());
    let log: Vec<AccessLog> = get(&t, "/access/log", admin).await;
    assert!(log.iter().all(|l| l.host_id != m.id));
    for token in [admin, bob.token.as_str()] {
        let hosts: Vec<Host> = get(&t, "/hosts", token).await;
        assert!(hosts.iter().all(|h| h.id != m.id));
    }
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/grants/{gid}"),
            admin,
            json!({})
        )
        .await,
        404
    );
    for token in [admin, bob.token.as_str()] {
        assert_eq!(
            status(
                &t,
                Method::DELETE,
                &format!("/hosts/{}", m.id),
                token,
                json!({})
            )
            .await,
            404
        );
    }
    let hosts: Vec<Host> = get(&t, "/hosts", &alice.token).await;
    assert!(hosts.iter().any(|h| h.id == m.id && h.online));
    // A view grant does not open shells, and neither does being an admin.
    for token in [admin, bob.token.as_str()] {
        assert_eq!(
            status(
                &t,
                Method::POST,
                &format!("/hosts/{}/sessions", m.id),
                token,
                json!({})
            )
            .await,
            404
        );
    }
    let mine: Vec<Grant> = get(&t, "/grants", &bob.token).await;
    assert_eq!(mine.len(), 1);
    // The grantee may hand their own grant back.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/grants/{gid}"),
            &bob.token,
            json!({})
        )
        .await,
        204
    );
    let mine: Vec<Grant> = get(&t, "/grants", &bob.token).await;
    assert!(mine.is_empty());
}

// Every session route refuses someone without a grant, and a view grant stays
// a view grant: no control, no ending it, no carrying it into a DM the grantee
// isn't in. Even a control grant doesn't let you make yourself the controller.
#[tokio::test]
async fn terminal_sessions_refuse_people_without_the_right_grant() {
    let t = Test::new().await;
    let alice = t.member("session_alice").await;
    let bob = t.member("session_bob").await;
    let carol = t.member("session_carol").await;
    let dave = t.member("session_dave").await;
    let m = machine(&t, &alice.token, "alice-box").await;
    let general = t.general().await;
    let session = t
        .post(
            &format!("/hosts/{}/sessions", m.id),
            &alice.token,
            json!({"channel_id":general}),
        )
        .await;
    let sid = session["id"].as_str().unwrap();
    grant(&t, &m.id, &bob.token, &alice.token, "terminal_view").await;

    let routes = [
        (
            Method::POST,
            format!("/sessions/{sid}/share"),
            json!({"channel_id":general}),
        ),
        (
            Method::POST,
            format!("/sessions/{sid}/direct-token"),
            json!({}),
        ),
        (
            Method::POST,
            format!("/sessions/{sid}/request-control"),
            json!({}),
        ),
        (
            Method::POST,
            format!("/sessions/{sid}/write"),
            json!({"text":"x"}),
        ),
        (
            Method::POST,
            format!("/sessions/{sid}/controller"),
            json!({"user_id":null}),
        ),
        (
            Method::POST,
            format!("/sessions/{sid}/recording"),
            json!({"enabled":true}),
        ),
        (Method::DELETE, format!("/sessions/{sid}"), json!({})),
    ];
    for token in [carol.token.as_str(), t.admin.token.as_str()] {
        for (method, path, body) in &routes {
            assert_eq!(
                status(&t, method.clone(), path, token, body.clone()).await,
                404,
                "{method} {path}"
            );
        }
    }

    let cards = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM terminal_cards WHERE session_id=?")
            .bind(sid)
            .fetch_one(&t.state.db)
            .await
            .unwrap()
    };
    let before = cards().await;
    let dm = t
        .post("/dms", &carol.token, json!({"member_ids":[dave.user.id]}))
        .await;
    assert_eq!(
        status(
            &t,
            Method::POST,
            &format!("/sessions/{sid}/share"),
            &bob.token,
            json!({"channel_id":dm["id"]})
        )
        .await,
        404
    );
    assert_eq!(cards().await, before, "a refused share left a card");
    for (method, path, body) in [
        (
            Method::POST,
            format!("/sessions/{sid}/request-control"),
            json!({}),
        ),
        (
            Method::POST,
            format!("/sessions/{sid}/controller"),
            json!({"user_id":bob.user.id}),
        ),
        (Method::DELETE, format!("/sessions/{sid}"), json!({})),
    ] {
        assert_eq!(
            status(&t, method.clone(), &path, &bob.token, body).await,
            404,
            "{method} {path}"
        );
    }
    grant(&t, &m.id, &bob.token, &alice.token, "terminal_control").await;
    assert_eq!(
        status(
            &t,
            Method::POST,
            &format!("/sessions/{sid}/controller"),
            &bob.token,
            json!({"user_id":bob.user.id})
        )
        .await,
        404
    );
    let card: Value = get(&t, &format!("/objects/{sid}"), &alice.token).await;
    assert!(card["state"]["terminal"]["ended_at"].is_null());
    assert_eq!(
        card["state"]["terminal"]["active_controller_id"],
        alice.user.id
    );
}

async fn output_within(socket: &mut Socket, wait: Duration) -> Vec<u8> {
    let found = tokio::time::timeout(wait, async {
        loop {
            match socket.next().await {
                Some(Ok(Frame::Binary(b))) => {
                    if let Ok(Event::TerminalOutput { bytes, .. }) = serde_json::from_slice(&b) {
                        return bytes;
                    }
                }
                Some(Ok(Frame::Ping(v))) => {
                    let _ = socket.send(Frame::Pong(v)).await;
                }
                Some(Ok(_)) => {}
                _ => return Vec::new(),
            }
        }
    })
    .await;
    found.unwrap_or_default()
}

// Output flows over the socket, not REST, so check the socket. Someone in the
// room without a grant can ask to open the session and gets nothing; a view
// grantee sees output, and their keystrokes never reach the machine.
#[tokio::test]
async fn terminal_output_reaches_only_viewers_and_view_input_never_reaches_the_host() {
    let t = Test::new().await;
    let alice = t.member("output_alice").await;
    let bob = t.member("output_bob").await;
    let carol = t.member("output_carol").await;
    let mut m = machine(&t, &alice.token, "alice-box").await;
    let general = t.general().await;
    let session = t
        .post(
            &format!("/hosts/{}/sessions", m.id),
            &alice.token,
            json!({"channel_id":general}),
        )
        .await;
    let sid = session["id"].as_str().unwrap().to_string();
    grant(&t, &m.id, &bob.token, &alice.token, "terminal_view").await;
    let mut viewer = socket(&t, &bob.token).await;
    let mut outsider = socket(&t, &carol.token).await;
    let open = serde_json::to_vec(&json!({"type":"terminal_open","session_id":sid})).unwrap();
    for s in [&mut viewer, &mut outsider] {
        s.send(Frame::Binary(open.clone().into())).await.unwrap();
    }
    viewer
        .send(Frame::Binary(
            serde_json::to_vec(
                &json!({"type":"terminal_input","session_id":sid,"bytes":b"VIEW_TYPED".to_vec()}),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

    // Only Bob's open reaches the machine, and his typing doesn't.
    let mut replays = 0;
    let _ = tokio::time::timeout(Duration::from_millis(800), async {
        while let Some(Ok(frame)) = m.socket.next().await {
            match frame {
                Frame::Binary(b) => match serde_json::from_slice::<HostFrame>(&b).unwrap() {
                    HostFrame::Replay { .. } => replays += 1,
                    HostFrame::Input { bytes, .. } => {
                        assert_ne!(bytes, b"VIEW_TYPED", "view input reached the host")
                    }
                    _ => {}
                },
                Frame::Ping(v) => {
                    let _ = m.socket.send(Frame::Pong(v)).await;
                }
                _ => {}
            }
        }
    })
    .await;
    assert_eq!(replays, 1);

    m.socket
        .send(Frame::Binary(
            serde_json::to_vec(
                &json!({"type":"output","session_id":sid,"bytes":b"SECRET_OUTPUT".to_vec()}),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        output_within(&mut viewer, Duration::from_secs(2)).await,
        b"SECRET_OUTPUT"
    );
    assert!(output_within(&mut outsider, Duration::from_millis(500))
        .await
        .is_empty());
}
