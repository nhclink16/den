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
    assert!(matches!(
        serde_json::from_str::<RelayFrame>(&next_frame(&mut peer).await).unwrap(),
        RelayFrame::Command { .. }
    ));
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
