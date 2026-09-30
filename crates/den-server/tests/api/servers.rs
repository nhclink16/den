use super::realtime::{socket, Socket};
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGP4DwQACfsD/fteaysAAAAASUVORK5CYII=";

fn info() -> ServerInfo {
    ServerInfo {
        slug: "minecraft".into(),
        game: "minecraft".into(),
        name: "The Den".into(),
        details: vec!["1.21.1".into(), "Fabric".into()],
        address: Some("minecraft.example:25565".into()),
        actions: vec![
            ServerAction {
                id: "save".into(),
                label: "Save".into(),
                admin_only: false,
                states: vec![ServerState::Up],
            },
            ServerAction {
                id: "restart".into(),
                label: "Restart".into(),
                admin_only: true,
                states: vec![ServerState::Up],
            },
            ServerAction {
                id: "start".into(),
                label: "Start".into(),
                admin_only: false,
                states: vec![ServerState::Down, ServerState::Asleep],
            },
        ],
        icon_png: Some(PNG.into()),
    }
}

fn status(names: &[&str]) -> ServerStatus {
    ServerStatus {
        state: ServerState::Up,
        players: names
            .iter()
            .map(|name| ServerPlayer {
                name: (*name).into(),
                user_id: None,
            })
            .collect(),
        max_players: Some(20),
        stats: vec![ServerStat {
            key: "tps".into(),
            label: "Tick rate".into(),
            unit: StatUnit::Tps,
            value: 19.8,
            graph: true,
        }],
        playtime: None,
    }
}

async fn api_token(t: &Test, session: &Session) -> String {
    t.post("/tokens", &session.token, json!({"name":"game relay"}))
        .await["token"]
        .as_str()
        .unwrap()
        .into()
}

fn ws_request(
    t: &Test,
    path: &str,
    token: Option<&str>,
) -> tokio_tungstenite::tungstenite::http::Request<()> {
    let mut request = format!("{}{path}", t.url.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    if let Some(token) = token {
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    }
    request
}

async fn relay(t: &Test, token: &str) -> Socket {
    let mut peer = connect_async(ws_request(t, "/servers/relay", Some(token)))
        .await
        .unwrap()
        .0;
    send(&mut peer, RelayFrame::Hello { server: info() }).await;
    peer
}

async fn send(peer: &mut Socket, frame: RelayFrame) {
    peer.send(Frame::Text(serde_json::to_string(&frame).unwrap().into()))
        .await
        .unwrap();
    // The pong follows processing of the preceding JSON frame, including writes.
    peer.send(Frame::Ping(b"barrier".to_vec().into()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match peer.next().await.unwrap().unwrap() {
                Frame::Pong(bytes) if bytes.as_ref() == b"barrier" => break,
                Frame::Ping(bytes) => peer.send(Frame::Pong(bytes)).await.unwrap(),
                other => panic!("unexpected relay frame before barrier: {other:?}"),
            }
        }
    })
    .await
    .expect("relay processed frame");
}

async fn next_frame(peer: &mut Socket) -> String {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match peer.next().await.unwrap().unwrap() {
                Frame::Text(text) => return text.to_string(),
                Frame::Ping(bytes) => peer.send(Frame::Pong(bytes)).await.unwrap(),
                Frame::Pong(_) => (),
                other => panic!("unexpected websocket frame: {other:?}"),
            }
        }
    })
    .await
    .expect("websocket text frame")
}

async fn updated(peer: &mut Socket) -> GameServer {
    loop {
        if let Event::ServerUpdated { server } =
            serde_json::from_str(&next_frame(peer).await).unwrap()
        {
            return server;
        }
    }
}

async fn detail(t: &Test, token: &str) -> ServerDetail {
    let response = t
        .req(Method::GET, "/servers/minecraft", token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

async fn action(t: &Test, token: &str, name: &str) -> reqwest::Response {
    t.req(
        Method::POST,
        &format!("/servers/minecraft/actions/{name}"),
        token,
    )
    .send()
    .await
    .unwrap()
}

async fn respond(peer: &mut Socket, action: &str, by: &str, ok: bool, message: &str) {
    let RelayFrame::Command {
        command_id,
        action: received,
        by: caller,
    } = serde_json::from_str(&next_frame(peer).await).unwrap()
    else {
        panic!("expected relay command")
    };
    assert_eq!(received, action);
    assert_eq!(caller, by);
    assert!(!command_id.is_empty());
    send(
        peer,
        RelayFrame::Result {
            command_id,
            ok,
            message: Some(message.into()),
        },
    )
    .await;
}

#[tokio::test]
async fn relay_requires_admin_api_token() {
    let t = Test::new().await;
    let member = t.member("relay_member").await;
    let member_token = api_token(&t, &member).await;
    let mut cookie = ws_request(&t, "/servers/relay", None);
    cookie.headers_mut().insert(
        "Cookie",
        format!("den_session={}", t.admin.token).parse().unwrap(),
    );
    cookie
        .headers_mut()
        .insert("Origin", t.url.parse().unwrap());
    for (request, expected) in [
        (ws_request(&t, "/servers/relay", Some(&member_token)), 403),
        (cookie, 403),
        (ws_request(&t, "/servers/relay", None), 401),
    ] {
        match connect_async(request).await {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status().as_u16(), expected)
            }
            _ => panic!("expected HTTP {expected} before websocket upgrade"),
        }
    }
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    assert!(detail(&t, &member.token).await.server.connected);
    peer.close(None).await.unwrap();
}

#[tokio::test]
async fn hello_and_status_reach_list_detail_and_ws() {
    let t = Test::new().await;
    let member = t.member("servers_watcher").await;
    let mut watcher = connect_async(ws_request(&t, "/ws?servers=true", Some(&member.token)))
        .await
        .unwrap()
        .0;
    let mut ordinary = socket(&t, &member.token).await;
    assert!(matches!(
        serde_json::from_str::<Event>(&next_frame(&mut watcher).await).unwrap(),
        Event::Resync { .. }
    ));
    assert!(matches!(
        serde_json::from_str::<Event>(&next_frame(&mut ordinary).await).unwrap(),
        Event::Resync { .. }
    ));
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    let hello = updated(&mut watcher).await;
    assert!(hello.connected);
    assert_eq!(hello.name, info().name);
    sqlx::query(
        "INSERT INTO game_server_samples(slug,key,at,value) VALUES('minecraft','expired',?,1)",
    )
    .bind(hello.updated_at - 26 * 3600)
    .execute(&t.state.db)
    .await
    .unwrap();
    let live = status(&["Alex", "Steve"]);
    send(
        &mut peer,
        RelayFrame::Status {
            status: live.clone(),
        },
    )
    .await;
    let seen = updated(&mut watcher).await;
    assert_eq!(seen.state, ServerState::Up);
    assert_eq!(seen.players, live.players);
    assert_eq!(seen.stats, live.stats);
    assert_eq!(seen.actions, info().actions);
    assert_eq!(seen.details, info().details);
    assert_eq!(seen.address, info().address);
    assert_eq!(seen.max_players, Some(20));
    let listed: Vec<GameServer> = t
        .req(Method::GET, "/servers", &member.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed, vec![seen.clone()]);
    let page = detail(&t, &member.token).await;
    assert_eq!(page.server, seen);
    let players = page
        .history
        .iter()
        .find(|series| series.key == "players")
        .unwrap();
    assert_eq!(players.label, "Players");
    assert_eq!(players.unit, StatUnit::Number);
    assert_eq!(players.points.last().unwrap().value, 2.0);
    let tps = page
        .history
        .iter()
        .find(|series| series.key == "tps")
        .unwrap();
    assert_eq!(tps.label, "Tick rate");
    assert_eq!(tps.unit, StatUnit::Tps);
    assert_eq!(tps.points.last().unwrap().value, 19.8);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM game_server_samples WHERE key='expired'"
        )
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0
    );
    let icon_url = seen.icon_url.as_ref().unwrap();
    assert!(icon_url.starts_with("/servers/minecraft/icon?v="));
    let icon = t
        .req(Method::GET, icon_url, &member.token)
        .send()
        .await
        .unwrap();
    assert_eq!(icon.status(), StatusCode::OK);
    assert_eq!(icon.headers()["content-type"], "image/png");
    assert_eq!(icon.headers()["cache-control"], "private, max-age=86400");
    assert_eq!(
        icon.bytes().await.unwrap().as_ref(),
        STANDARD.decode(PNG).unwrap()
    );
    let mut changed = info();
    changed.name = "The Den updated".into();
    send(
        &mut peer,
        RelayFrame::Hello {
            server: changed.clone(),
        },
    )
    .await;
    let changed_event = updated(&mut watcher).await;
    assert_eq!(changed_event.name, changed.name);
    assert_eq!(changed_event.state, ServerState::Up);
    assert_eq!(detail(&t, &member.token).await.server, changed_event);
    let mut second = status(&["Alex"]);
    second.stats[0].value = 20.0;
    send(&mut peer, RelayFrame::Status { status: second }).await;
    assert_eq!(updated(&mut watcher).await.players.len(), 1);
    let after = detail(&t, &member.token).await;
    for series in after.history {
        let first = page
            .history
            .iter()
            .find(|old| old.key == series.key)
            .unwrap();
        let previous_at = first.points.last().unwrap().at;
        let latest = series.points.last().unwrap();
        assert_eq!(latest.at % 60, 0);
        assert_eq!(
            series.points.len(),
            if latest.at == previous_at { 1 } else { 2 },
            "one sample per minute"
        );
        assert_eq!(
            latest.value,
            if series.key == "players" { 1.0 } else { 20.0 }
        );
    }
    let following = Event::Presence {
        user_id: member.user.id.clone(),
        online: false,
    };
    t.state.events.send(following.clone()).unwrap();
    loop {
        let event: Event = serde_json::from_str(&next_frame(&mut ordinary).await).unwrap();
        assert!(
            !matches!(event, Event::ServerUpdated { .. }),
            "server events require opt-in"
        );
        if event == following {
            break;
        }
    }
}

#[tokio::test]
async fn graph_keys_are_capped_across_messages() {
    let t = Test::new().await;
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    for index in 0..50 {
        let mut value = status(&[]);
        value.stats[0].key = format!("graph_{index}");
        send(&mut peer, RelayFrame::Status { status: value }).await;
    }
    let page = detail(&t, &t.admin.token).await;
    assert_eq!(
        page.history.len(),
        5,
        "players plus at most four graph keys"
    );
    assert_eq!(
        page.history
            .iter()
            .map(|series| series.key.as_str())
            .collect::<Vec<_>>(),
        ["players", "graph_0", "graph_1", "graph_2", "graph_3"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT key) FROM game_server_samples WHERE slug='minecraft'"
        )
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        5,
        "stored samples must also cap distinct keys"
    );
    let mut value = status(&[]);
    value.stats[0].key = "graph_0".into();
    value.stats[0].value = 42.0;
    send(&mut peer, RelayFrame::Status { status: value }).await;
    let page = detail(&t, &t.admin.token).await;
    assert_eq!(page.history.len(), 5);
    assert_eq!(
        page.history
            .iter()
            .find(|series| series.key == "graph_0")
            .unwrap()
            .points
            .last()
            .unwrap()
            .value,
        42.0,
        "an existing key must keep accepting samples at the cap"
    );
}

#[tokio::test]
async fn actions_follow_capabilities_and_roles() {
    let t = Test::new().await;
    let member = t.member("server_actions").await;
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    assert_eq!(
        action(&t, &member.token, "undeclared").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        action(&t, &member.token, "restart").await.status(),
        StatusCode::FORBIDDEN
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(200), next_frame(&mut peer))
            .await
            .is_err(),
        "denied action must not reach relay"
    );
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &member.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut peer,
        "save",
        &member.user.display_name,
        true,
        "World saved",
    )
    .await;
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .json::<ServerActionResult>()
            .await
            .unwrap()
            .message
            .as_deref(),
        Some("World saved")
    );
    let response = action(&t, &member.token, "start").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error, "conflict");
    assert_eq!(error.message, "Not available while the server is up");
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &member.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut peer,
        "save",
        &member.user.display_name,
        false,
        "Disk is full",
    )
    .await;
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error, "server_action_failed");
    assert_eq!(error.message, "Disk is full");
}

#[tokio::test]
async fn pending_command_survives_reconnect() {
    let t = Test::new().await;
    let token = api_token(&t, &t.admin).await;
    let mut first = relay(&t, &token).await;
    send(
        &mut first,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &t.admin.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    let command: RelayFrame = serde_json::from_str(&next_frame(&mut first).await).unwrap();
    let RelayFrame::Command { ref command_id, .. } = command else {
        panic!("expected relay command")
    };
    first.close(None).await.unwrap();
    drop(first);
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<ApiError>().await.unwrap().message,
        "The server's relay disconnected"
    );

    let mut second = connect_async(ws_request(&t, "/servers/relay", Some(&token)))
        .await
        .unwrap()
        .0;
    second
        .send(Frame::Text(
            serde_json::to_string(&RelayFrame::Hello { server: info() })
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();
    let replay = tokio::time::timeout(Duration::from_secs(2), next_frame(&mut second))
        .await
        .expect("pending command must be replayed after reconnect");
    assert_eq!(
        serde_json::from_str::<RelayFrame>(&replay).unwrap(),
        command
    );
    send(
        &mut second,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let response = action(&t, &t.admin.token, "save").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<ApiError>().await.unwrap().message,
        "The server is still working on the last request"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(200), next_frame(&mut second))
            .await
            .is_err(),
        "busy server must not deliver a new command"
    );
    send(
        &mut second,
        RelayFrame::Result {
            command_id: "unrelated-command".into(),
            ok: true,
            message: None,
        },
    )
    .await;
    assert_eq!(
        action(&t, &t.admin.token, "save").await.status(),
        StatusCode::CONFLICT,
        "an unrelated result must not clear the busy command"
    );
    send(
        &mut second,
        RelayFrame::Result {
            command_id: command_id.clone(),
            ok: true,
            message: Some("World saved".into()),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &t.admin.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    let RelayFrame::Command {
        command_id: next_id,
        action,
        by,
    } = serde_json::from_str(&next_frame(&mut second).await).unwrap()
    else {
        panic!("expected next relay command")
    };
    assert_ne!(next_id, *command_id);
    assert_eq!(action, "save");
    assert_eq!(by, t.admin.user.display_name);
    send(
        &mut second,
        RelayFrame::Result {
            command_id: next_id,
            ok: true,
            message: Some("Saved again".into()),
        },
    )
    .await;
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn action_without_relay_and_after_reconnect() {
    let t = Test::new().await;
    let member = t.member("server_reconnect").await;
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&["Alex"]),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &member.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    let command: RelayFrame = serde_json::from_str(&next_frame(&mut peer).await).unwrap();
    let RelayFrame::Command { ref command_id, .. } = command else {
        panic!("expected relay command")
    };
    peer.close(None).await.unwrap();
    drop(peer);
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<ApiError>().await.unwrap().message,
        "The server's relay disconnected"
    );
    let offline = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let server = detail(&t, &member.token).await.server;
            if !server.connected {
                break server;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(offline.state, ServerState::Down);
    assert!(offline.players.is_empty() && offline.stats.is_empty());
    assert_eq!(offline.actions, info().actions);
    let response = action(&t, &member.token, "save").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error, "conflict");
    assert_eq!(error.message, "The server's relay isn't connected");
    assert_eq!(
        action(&t, &member.token, "restart").await.status(),
        StatusCode::FORBIDDEN,
        "admin action stays forbidden while disconnected"
    );
    assert_eq!(
        action(&t, &member.token, "undeclared").await.status(),
        StatusCode::NOT_FOUND
    );
    let mut peer = connect_async(ws_request(&t, "/servers/relay", Some(&token)))
        .await
        .unwrap()
        .0;
    peer.send(Frame::Text(
        serde_json::to_string(&RelayFrame::Hello { server: info() })
            .unwrap()
            .into(),
    ))
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<RelayFrame>(&next_frame(&mut peer).await).unwrap(),
        command
    );
    send(
        &mut peer,
        RelayFrame::Result {
            command_id: command_id.clone(),
            ok: true,
            message: Some("Saved before reconnect".into()),
        },
    )
    .await;
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &member.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut peer,
        "save",
        &member.user.display_name,
        true,
        "Saved after reconnect",
    )
    .await;
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .json::<ServerActionResult>()
            .await
            .unwrap()
            .message
            .as_deref(),
        Some("Saved after reconnect")
    );
    let mut replacement = relay(&t, &token).await;
    send(
        &mut replacement,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(Ok(frame)) = peer.next().await {
            match frame {
                Frame::Close(_) => break,
                Frame::Ping(bytes) => peer.send(Frame::Pong(bytes)).await.unwrap(),
                other => panic!("replaced relay received frame: {other:?}"),
            }
        }
    })
    .await
    .expect("new relay closes old connection");
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &member.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut replacement,
        "save",
        &member.user.display_name,
        true,
        "Saved by replacement",
    )
    .await;
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .json::<ServerActionResult>()
            .await
            .unwrap()
            .message
            .as_deref(),
        Some("Saved by replacement")
    );
}

#[tokio::test]
async fn week_playtime_counts_sessions() {
    let t = Test::new().await;
    let member = t.member("server_playtime").await;
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    let mut first = status(&["Alex", "Steve"]);
    first.playtime = Some(vec![
        ServerPlaytime {
            name: "Alex".into(),
            user_id: Some(member.user.id.clone()),
            total_seconds: Some(1000),
            week_seconds: 0,
        },
        ServerPlaytime {
            name: "Veteran".into(),
            user_id: None,
            total_seconds: Some(2000),
            week_seconds: 0,
        },
    ]);
    send(&mut peer, RelayFrame::Status { status: first }).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM game_server_sessions WHERE ended_at IS NULL"
        )
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        2
    );
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&["Steve"]),
        },
    )
    .await;
    let sessions: Vec<(String, Option<i64>)> =
        sqlx::query_as("SELECT player, ended_at FROM game_server_sessions ORDER BY player")
            .fetch_all(&t.state.db)
            .await
            .unwrap();
    assert_eq!(sessions[0].0, "Alex");
    assert!(sessions[0].1.is_some());
    assert_eq!(sessions[1], ("Steve".into(), None));
    let mut asleep = status(&["Steve"]);
    asleep.state = ServerState::Asleep;
    send(&mut peer, RelayFrame::Status { status: asleep }).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM game_server_sessions WHERE ended_at IS NULL"
        )
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    sqlx::query(
        "UPDATE game_server_sessions SET started_at=?,seen_at=?,ended_at=? WHERE player='Alex'",
    )
    .bind(now - 120)
    .bind(now - 20)
    .bind(now - 20)
    .execute(&t.state.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE game_server_sessions SET started_at=?,seen_at=?,ended_at=? WHERE player='Steve'",
    )
    .bind(now - 90)
    .bind(now - 30)
    .bind(now - 30)
    .execute(&t.state.db)
    .await
    .unwrap();
    let week = 7 * 24 * 60 * 60;
    for (name, started, ended) in [
        ("Alex", now - week - 100, Some(now - week + 3600)),
        ("Steve", now - week - 100, Some(now - week - 1)),
        ("Newcomer", now - 40, None),
    ] {
        sqlx::query("INSERT INTO game_server_sessions(id,slug,player,started_at,seen_at,ended_at) VALUES(?,'minecraft',?,?,?,?)")
            .bind(ulid::Ulid::new().to_string()).bind(name).bind(started).bind(ended.unwrap_or(now)).bind(ended).execute(&t.state.db).await.unwrap();
    }
    let page = detail(&t, &member.token).await;
    assert_eq!(
        page.playtime
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["Veteran", "Alex", "Steve", "Newcomer"]
    );
    assert_eq!(page.playtime[0].total_seconds, Some(2000));
    assert_eq!(page.playtime[0].week_seconds, 0);
    assert_eq!(page.playtime[1].total_seconds, Some(1000));
    assert_eq!(page.playtime[1].user_id.as_ref(), Some(&member.user.id));
    assert!(
        (3697..=3700).contains(&page.playtime[1].week_seconds),
        "session crossing week boundary is clipped"
    );
    assert_eq!(page.playtime[2].total_seconds, None);
    assert_eq!(
        page.playtime[2].week_seconds, 60,
        "sessions outside the week do not count"
    );
    assert_eq!(page.playtime[3].total_seconds, None);
    assert!(
        (40..=43).contains(&page.playtime[3].week_seconds),
        "open session counts through now"
    );
    peer.close(None).await.unwrap();
    drop(peer);
    tokio::time::timeout(Duration::from_secs(3), async {
        while detail(&t, &member.token).await.server.connected
            || sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM game_server_sessions WHERE ended_at IS NULL",
            )
            .fetch_one(&t.state.db)
            .await
            .unwrap()
                != 0
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    sqlx::query("INSERT INTO game_server_sessions(id,slug,player,started_at,seen_at) VALUES(?,'minecraft','Restart',?,?)")
        .bind(ulid::Ulid::new().to_string()).bind(now - 60).bind(now - 30).execute(&t.state.db).await.unwrap();
    let reopened = AppState::open(
        t.dir.join("den.db"),
        t.dir.join("uploads"),
        t.dir.join("bootstrap.key"),
        t.url.clone(),
        1024 * 1024,
    )
    .await
    .unwrap();
    let ended: i64 =
        sqlx::query_scalar("SELECT ended_at FROM game_server_sessions WHERE player='Restart'")
            .fetch_one(&reopened.db)
            .await
            .unwrap();
    assert_eq!(
        ended,
        now - 30,
        "restart closes abandoned sessions at last sighting"
    );
    reopened.db.close().await;
}

#[tokio::test]
async fn revoked_relay_key_is_cut_off() {
    let t = Test::new().await;
    let key = t
        .post("/tokens", &t.admin.token, json!({"name":"revoked relay"}))
        .await;
    let mut peer = relay(&t, key["token"].as_str().unwrap()).await;
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&["Alex"]),
        },
    )
    .await;
    let before = detail(&t, &t.admin.token).await;
    let stored_playtime: String =
        sqlx::query_scalar("SELECT playtime FROM game_servers WHERE slug='minecraft'")
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    let response = t
        .req(
            Method::DELETE,
            &format!("/tokens/{}", key["credential"]["id"].as_str().unwrap()),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let mut changed = status(&["Unauthorized"]);
    changed.playtime = Some(vec![ServerPlaytime {
        name: "Unauthorized".into(),
        user_id: None,
        total_seconds: Some(999),
        week_seconds: 0,
    }]);
    let _ = peer
        .send(Frame::Text(
            serde_json::to_string(&RelayFrame::Status { status: changed })
                .unwrap()
                .into(),
        ))
        .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(frame)) = peer.next().await {
            match frame {
                Frame::Close(_) => break,
                Frame::Ping(bytes) => {
                    let _ = peer.send(Frame::Pong(bytes)).await;
                }
                _ => (),
            }
        }
    })
    .await
    .expect("revoked relay socket must close within 5 seconds");
    let listed: Vec<GameServer> = t
        .req(Method::GET, "/servers", &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!listed[0].connected);
    let after = detail(&t, &t.admin.token).await;
    let after_playtime: String =
        sqlx::query_scalar("SELECT playtime FROM game_servers WHERE slug='minecraft'")
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(
        after_playtime, stored_playtime,
        "revoked status must not change stored playtime"
    );
    assert_eq!(
        after.history, before.history,
        "revoked status must not change stored history"
    );
}

#[tokio::test]
async fn second_key_cannot_take_over_server() {
    let t = Test::new().await;
    let key_a = t
        .post("/tokens", &t.admin.token, json!({"name":"relay A"}))
        .await;
    let key_b = api_token(&t, &t.admin).await;
    let mut a = relay(&t, key_a["token"].as_str().unwrap()).await;
    send(
        &mut a,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let mut b = connect_async(ws_request(&t, "/servers/relay", Some(&key_b)))
        .await
        .unwrap()
        .0;
    b.send(Frame::Text(
        serde_json::to_string(&RelayFrame::Hello { server: info() })
            .unwrap()
            .into(),
    ))
    .await
    .unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(3), b.next())
        .await
        .expect("second key must receive ownership close")
        .unwrap()
        .unwrap();
    let Frame::Close(Some(close)) = closed else {
        panic!("expected ownership close, got {closed:?}")
    };
    assert_eq!(u16::from(close.code), 4003);
    assert_eq!(
        close.reason,
        "This server belongs to another relay key; revoke that key to move it"
    );
    assert!(detail(&t, &t.admin.token).await.server.connected);
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &t.admin.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut a,
        "save",
        &t.admin.user.display_name,
        true,
        "Saved by A",
    )
    .await;
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/tokens/{}", key_a["credential"]["id"].as_str().unwrap()),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::NO_CONTENT
    );
    let mut b = relay(&t, &key_b).await;
    send(
        &mut b,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &t.admin.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut b,
        "save",
        &t.admin.user.display_name,
        true,
        "Saved by B",
    )
    .await;
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn pending_command_blocks_retry() {
    let t = Test::new().await;
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &t.admin.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    let RelayFrame::Command { command_id, .. } =
        serde_json::from_str(&next_frame(&mut peer).await).unwrap()
    else {
        panic!("expected first command")
    };
    let mut starting = status(&[]);
    starting.state = ServerState::Starting;
    send(&mut peer, RelayFrame::Status { status: starting }).await;
    let response = tokio::time::timeout(Duration::from_secs(2), action(&t, &t.admin.token, "save"))
        .await
        .expect("pending retry must return 409 without waiting for the relay");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error, "conflict");
    assert_eq!(
        error.message,
        "The server is still working on the last request"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(200), next_frame(&mut peer))
            .await
            .is_err(),
        "retry must not send a second command"
    );
    send(
        &mut peer,
        RelayFrame::Status {
            status: status(&[]),
        },
    )
    .await;
    let response = tokio::time::timeout(Duration::from_secs(25), pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error, "timeout");
    assert_eq!(
        error.message,
        "The server is still working on it; check back in a moment"
    );
    let response = tokio::time::timeout(Duration::from_secs(2), action(&t, &t.admin.token, "save"))
        .await
        .expect("timed-out command must still block retry");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error, "conflict");
    assert_eq!(
        error.message,
        "The server is still working on the last request"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(200), next_frame(&mut peer))
            .await
            .is_err(),
        "timeout must not permit a second command"
    );
    send(
        &mut peer,
        RelayFrame::Result {
            command_id,
            ok: true,
            message: Some("Saved late".into()),
        },
    )
    .await;
    let request = t.req(
        Method::POST,
        "/servers/minecraft/actions/save",
        &t.admin.token,
    );
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    respond(
        &mut peer,
        "save",
        &t.admin.user.display_name,
        true,
        "Saved next request",
    )
    .await;
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn history_and_sessions_are_bounded() {
    let t = Test::new().await;
    let token = api_token(&t, &t.admin).await;
    let mut peer = relay(&t, &token).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    for (id, days) in [("old", 40), ("recent", 20)] {
        let ended = now - days * 86400;
        sqlx::query("INSERT INTO game_server_sessions(id,slug,player,started_at,seen_at,ended_at) VALUES(?,'minecraft',?,?,?,?)")
            .bind(id).bind(id).bind(ended - 60).bind(ended).bind(ended).execute(&t.state.db).await.unwrap();
    }
    let mut value = status(&["Alex"]);
    value.stats = (0..6)
        .map(|n| ServerStat {
            key: format!("stat{n}"),
            label: format!("Graph {n}"),
            unit: StatUnit::Number,
            value: n as f64,
            graph: true,
        })
        .collect();
    value.stats.insert(
        0,
        ServerStat {
            key: "players".into(),
            label: "Ignored players stat".into(),
            unit: StatUnit::Number,
            value: 99.0,
            graph: true,
        },
    );
    value.stats.insert(
        0,
        ServerStat {
            key: "ungraphed".into(),
            label: "Not graphed".into(),
            unit: StatUnit::Number,
            value: 9.0,
            graph: false,
        },
    );
    send(&mut peer, RelayFrame::Status { status: value }).await;
    let sessions: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM game_server_sessions WHERE ended_at IS NOT NULL ORDER BY id",
    )
    .fetch_all(&t.state.db)
    .await
    .unwrap();
    assert_eq!(
        sessions,
        vec!["recent"],
        "expired ended sessions must be pruned"
    );
    let page = detail(&t, &t.admin.token).await;
    assert_eq!(
        page.history
            .iter()
            .map(|s| s.key.as_str())
            .collect::<Vec<_>>(),
        vec!["players", "stat0", "stat1", "stat2", "stat3"],
        "history must contain players plus the first four graph stats"
    );
    assert_eq!(page.history[0].points.last().unwrap().value, 1.0);
    assert_eq!(page.history[1].label, "Graph 0");
    let mut value = status(&[]);
    value.stats = (0..16)
        .map(|n| ServerStat {
            key: format!("new{n}"),
            label: format!("New {n}"),
            unit: StatUnit::Number,
            value: n as f64,
            graph: false,
        })
        .collect();
    send(&mut peer, RelayFrame::Status { status: value }).await;
    let page = detail(&t, &t.admin.token).await;
    assert_eq!(
        page.history
            .iter()
            .find(|s| s.key == "stat0")
            .unwrap()
            .label,
        "stat0",
        "obsolete stat labels must be dropped once the cache exceeds 16 keys"
    );
}

#[tokio::test]
async fn idle_relay_loses_admin_access() {
    for change in ["revoke", "demote", "remove"] {
        let t = Test::new().await;
        let observer = t.member("relay_observer").await;
        let key = t
            .post("/tokens", &t.admin.token, json!({"name":"idle relay"}))
            .await;
        let mut peer = relay(&t, key["token"].as_str().unwrap()).await;
        if change == "revoke" {
            assert_eq!(
                t.req(
                    Method::DELETE,
                    &format!("/tokens/{}", key["credential"]["id"].as_str().unwrap()),
                    &t.admin.token
                )
                .send()
                .await
                .unwrap()
                .status(),
                StatusCode::NO_CONTENT
            );
        } else {
            let query = if change == "demote" {
                "UPDATE users SET role='member' WHERE id=?"
            } else {
                "UPDATE users SET removed_at=1 WHERE id=?"
            };
            sqlx::query(query)
                .bind(&t.admin.user.id)
                .execute(&t.state.db)
                .await
                .unwrap();
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(Ok(frame)) = peer.next().await {
                match frame {
                    Frame::Close(_) => break,
                    Frame::Ping(bytes) => {
                        let _ = peer.send(Frame::Pong(bytes)).await;
                    }
                    _ => (),
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("idle relay must close after {change}"));
        assert!(
            !detail(&t, &observer.token).await.server.connected,
            "relay remains connected after {change}"
        );
    }
}
