use crate::{auth::Auth, *};
use axum::extract::{
    ws::{CloseFrame, Message as Frame, WebSocket, WebSocketUpgrade},
    Path,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqliteRow, Row};
use tokio::sync::{mpsc, oneshot};

struct Connection {
    id: String,
    tx: mpsc::Sender<RelayFrame>,
    pending: HashMap<String, (Instant, oneshot::Sender<RelayFrame>)>,
}

#[derive(Default)]
struct Live {
    connection: Option<Connection>,
    busy: Option<(String, String, String, Instant)>,
    status: Option<ServerStatus>,
    updated_at: Option<i64>,
    stats: HashMap<String, ServerStat>,
}

#[derive(Default)]
pub(crate) struct Servers {
    live: Mutex<HashMap<String, Live>>,
}

async fn game(s: &AppState, row: &SqliteRow) -> Result<GameServer> {
    let info: ServerInfo =
        serde_json::from_str(row.try_get("info")?).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let icon: Option<Vec<u8>> = row.try_get("icon")?;
    let all = s.servers.live.lock().await;
    let live = all.get(&info.slug);
    let connected = live.is_some_and(|l| l.connection.is_some());
    let status = live.filter(|_| connected).and_then(|l| l.status.as_ref());
    Ok(GameServer {
        icon_url: icon.map(|bytes| {
            let hash = format!("{:x}", Sha256::digest(bytes));
            format!("/servers/{}/icon?v={}", info.slug, &hash[..12])
        }),
        connected,
        state: status.map_or(ServerState::Down, |v| v.state),
        players: status.map_or_else(Vec::new, |v| v.players.clone()),
        max_players: status.and_then(|v| v.max_players),
        stats: status.map_or_else(Vec::new, |v| v.stats.clone()),
        updated_at: live
            .and_then(|l| l.updated_at)
            .unwrap_or(row.try_get("updated_at")?),
        slug: info.slug,
        game: info.game,
        name: info.name,
        details: info.details,
        address: info.address,
        actions: info.actions,
    })
}

async fn announce(s: &AppState, slug: &str) -> Result<()> {
    let row = sqlx::query("SELECT * FROM game_servers WHERE slug=?")
        .bind(slug)
        .fetch_one(&s.db)
        .await?;
    let _ = s.events.send(Event::ServerUpdated {
        server: game(s, &row).await?,
    });
    Ok(())
}

#[utoipa::path(get,path="/servers",responses((status=200,body=[GameServer])))]
pub(crate) async fn list(State(s): State<AppState>, _a: Auth) -> Result<Json<Vec<GameServer>>> {
    let rows = sqlx::query("SELECT * FROM game_servers ORDER BY slug")
        .fetch_all(&s.db)
        .await?;
    let mut servers = Vec::new();
    for row in rows {
        servers.push(game(&s, &row).await?);
    }
    Ok(Json(servers))
}

#[utoipa::path(get,path="/servers/{slug}",params(("slug"=String,Path)),responses((status=200,body=ServerDetail),(status=404,body=ApiError)))]
pub(crate) async fn detail(
    State(s): State<AppState>,
    _a: Auth,
    Path(slug): Path<String>,
) -> Result<Json<ServerDetail>> {
    let row = sqlx::query("SELECT * FROM game_servers WHERE slug=?")
        .bind(&slug)
        .fetch_one(&s.db)
        .await?;
    let server = game(&s, &row).await?;
    let mut playtime: Vec<ServerPlaytime> = serde_json::from_str(row.try_get("playtime")?)
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let at = now();
    let weeks = sqlx::query_as::<_, (String, i64)>(
        "SELECT player, SUM(MAX(0, MIN(COALESCE(ended_at, ?), ?) - MAX(started_at, ?))) FROM game_server_sessions WHERE slug=? AND started_at<=? AND COALESCE(ended_at, ?)>=? GROUP BY player",
    )
    .bind(at).bind(at).bind(at - 7 * 86400).bind(&slug).bind(at).bind(at).bind(at - 7 * 86400)
    .fetch_all(&s.db).await?;
    for p in &mut playtime {
        p.week_seconds = 0;
    }
    for (name, seconds) in weeks {
        if let Some(p) = playtime.iter_mut().find(|p| p.name == name) {
            p.week_seconds = seconds;
        } else if seconds > 0 {
            playtime.push(ServerPlaytime {
                name,
                user_id: None,
                total_seconds: None,
                week_seconds: seconds,
            });
        }
    }
    playtime.sort_by(|a, b| {
        b.total_seconds
            .cmp(&a.total_seconds)
            .then(b.week_seconds.cmp(&a.week_seconds))
            .then(a.name.cmp(&b.name))
    });
    let samples = sqlx::query_as::<_, (String, i64, f64)>(
        "SELECT key, at, value FROM game_server_samples WHERE slug=? AND at>=? AND at<=? ORDER BY key, at",
    )
    .bind(&slug)
    .bind(at - 86400)
    .bind(at)
    .fetch_all(&s.db)
    .await?;
    let stats = s
        .servers
        .live
        .lock()
        .await
        .get(&slug)
        .map(|l| l.stats.clone())
        .unwrap_or_default();
    let mut history = vec![ServerSeries {
        key: "players".into(),
        label: "Players".into(),
        unit: StatUnit::Number,
        points: vec![],
    }];
    for (key, at, value) in samples {
        let index = match history.iter().position(|series| series.key == key) {
            Some(i) => i,
            None => {
                let stat = stats.get(&key);
                history.push(ServerSeries {
                    label: stat.map_or_else(|| key.clone(), |s| s.label.clone()),
                    unit: stat.map_or(StatUnit::Number, |s| s.unit),
                    key,
                    points: vec![],
                });
                history.len() - 1
            }
        };
        history[index].points.push(ServerPoint { at, value });
    }
    Ok(Json(ServerDetail {
        server,
        playtime,
        history,
    }))
}

#[utoipa::path(get,path="/servers/{slug}/icon",params(("slug"=String,Path)),responses((status=200,description="PNG icon",content_type="image/png",body=Vec<u8>),(status=404,body=ApiError)))]
pub(crate) async fn icon(
    State(s): State<AppState>,
    _a: Auth,
    Path(slug): Path<String>,
) -> Result<Response> {
    let icon =
        sqlx::query_scalar::<_, Option<Vec<u8>>>("SELECT icon FROM game_servers WHERE slug=?")
            .bind(slug)
            .fetch_one(&s.db)
            .await?
            .ok_or_else(Error::missing)?;
    Ok((
        [
            ("content-type", "image/png"),
            ("cache-control", "private, max-age=86400"),
        ],
        icon,
    )
        .into_response())
}

#[utoipa::path(post,path="/servers/{slug}/actions/{action}",params(("slug"=String,Path),("action"=String,Path)),responses((status=200,body=ServerActionResult),(status=403,body=ApiError),(status=404,body=ApiError),(status=409,body=ApiError),(status=502,body=ApiError),(status=504,body=ApiError)))]
pub(crate) async fn action(
    State(s): State<AppState>,
    a: Auth,
    Path((slug, action)): Path<(String, String)>,
) -> Result<Json<ServerActionResult>> {
    let id = s.id();
    let (tx, rx) = oneshot::channel();
    let sender;
    {
        let _g = s.writes.lock().await;
        let row = sqlx::query("SELECT * FROM game_servers WHERE slug=?")
            .bind(&slug)
            .fetch_one(&s.db)
            .await?;
        let server = game(&s, &row).await?;
        let capability = server
            .actions
            .iter()
            .find(|c| c.id == action)
            .ok_or_else(Error::missing)?;
        if capability.admin_only {
            a.admin()?;
        }
        let mut all = s.servers.live.lock().await;
        let live = all
            .get_mut(&slug)
            .filter(|l| l.connection.is_some())
            .ok_or_else(|| Error::conflict("The server's relay isn't connected"))?;
        if live
            .busy
            .as_ref()
            .is_some_and(|(_, _, _, at)| at.elapsed() < Duration::from_secs(300))
        {
            return Err(Error::conflict(
                "The server is still working on the last request",
            ));
        }
        if !capability.states.contains(&server.state) {
            let state = serde_json::to_value(server.state).unwrap();
            return Err(Error::conflict(format!(
                "Not available while the server is {}",
                state.as_str().unwrap()
            )));
        }
        live.busy = Some((
            id.clone(),
            action.clone(),
            a.user.display_name.clone(),
            Instant::now(),
        ));
        let c = live.connection.as_mut().unwrap();
        sender = c.tx.clone();
        c.pending.insert(id.clone(), (Instant::now(), tx));
    }
    let answer = tokio::time::timeout(Duration::from_secs(20), async {
        if sender
            .send(RelayFrame::Command {
                command_id: id.clone(),
                action,
                by: a.user.display_name,
            })
            .await
            .is_err()
        {
            return Err(());
        }
        rx.await.map_err(|_| ())
    })
    .await;
    match answer {
        Ok(Ok(RelayFrame::Result {
            ok: true, message, ..
        })) => Ok(Json(ServerActionResult { message })),
        Ok(Ok(RelayFrame::Result { message, .. })) => Err(Error(
            StatusCode::BAD_GATEWAY,
            "server_action_failed",
            message.unwrap_or_else(|| "The server couldn't do that".into()),
        )),
        Err(_) => Err(Error(
            StatusCode::GATEWAY_TIMEOUT,
            "timeout",
            "The server is still working on it; check back in a moment".into(),
        )),
        _ => Err(Error::conflict("The server's relay disconnected")),
    }
}

#[utoipa::path(get,path="/servers/relay",responses((status=101,description="Admin API token relay; JSON RelayFrame text messages, hello required within 5 seconds"),(status=401,body=ApiError),(status=403,body=ApiError)))]
pub(crate) async fn relay(
    State(s): State<AppState>,
    a: Auth,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    if a.session {
        return Err(Error::forbidden());
    }
    a.admin()?;
    Ok(ws
        .max_message_size(256 * 1024)
        .max_frame_size(256 * 1024)
        .on_upgrade(move |ws| run(s, a, ws)))
}

async fn hello(s: &AppState, token_id: &str, mut info: ServerInfo) -> Result<()> {
    let length = |v: &str, min, max| (min..=max).contains(&v.chars().count());
    if !valid_server_slug(&info.slug)
        || !length(&info.name, 1, 100)
        || !length(&info.game, 1, 32)
        || info.details.len() > 8
        || info.details.iter().any(|d| !length(d, 0, 32))
        || info.address.as_ref().is_some_and(|a| !length(a, 0, 100))
        || info.actions.len() > 8
        || info
            .actions
            .iter()
            .any(|a| !valid_server_slug(&a.id) || !length(&a.label, 0, 32))
    {
        return Err(Error::bad("Invalid server info"));
    }
    let icon = info
        .icon_png
        .take()
        .map(|v| {
            if v.len() > 87384 {
                return Err(Error::bad("Invalid server icon"));
            }
            let bytes = STANDARD
                .decode(v)
                .map_err(|_| Error::bad("Invalid server icon"))?;
            if bytes.len() > 64 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err(Error::bad("Invalid server icon"));
            }
            Ok(bytes)
        })
        .transpose()?;
    let mut stored = serde_json::to_value(&info).unwrap();
    stored.as_object_mut().unwrap().remove("icon_png");
    sqlx::query("INSERT INTO game_servers(slug,info,icon,updated_at,token_id) VALUES(?,?,?,?,?) ON CONFLICT(slug) DO UPDATE SET info=excluded.info,icon=excluded.icon,updated_at=excluded.updated_at,token_id=COALESCE(game_servers.token_id,excluded.token_id)")
        .bind(&info.slug).bind(stored.to_string()).bind(icon).bind(now()).bind(token_id).execute(&s.db).await?;
    Ok(())
}

async fn end_sessions(s: &AppState, slug: &str) -> Result<()> {
    sqlx::query("UPDATE game_server_sessions SET ended_at=? WHERE slug=? AND ended_at IS NULL")
        .bind(now())
        .bind(slug)
        .execute(&s.db)
        .await?;
    Ok(())
}

async fn status(s: &AppState, slug: &str, mut status: ServerStatus) -> Result<()> {
    status.players.truncate(100);
    status.stats.truncate(16);
    let at = now();
    let mut tx = s.db.begin().await?;
    if let Some(playtime) = &mut status.playtime {
        playtime.truncate(200);
        sqlx::query("UPDATE game_servers SET playtime=? WHERE slug=?")
            .bind(serde_json::to_string(playtime).unwrap())
            .bind(slug)
            .execute(&mut *tx)
            .await?;
    }
    let open = sqlx::query_as::<_, (String, String)>(
        "SELECT id,player FROM game_server_sessions WHERE slug=? AND ended_at IS NULL",
    )
    .bind(slug)
    .fetch_all(&mut *tx)
    .await?;
    let players = if status.state == ServerState::Up {
        status.players.as_slice()
    } else {
        &[]
    };
    for (id, name) in &open {
        if !players.iter().any(|p| p.name == *name) {
            sqlx::query("UPDATE game_server_sessions SET ended_at=? WHERE id=?")
                .bind(at)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
    }
    let mut names = std::collections::HashSet::new();
    for player in players {
        if names.insert(&player.name) && !open.iter().any(|(_, n)| n == &player.name) {
            sqlx::query("INSERT INTO game_server_sessions(id,slug,player,started_at,seen_at) VALUES(?,?,?,?,?)")
                .bind(s.id()).bind(slug).bind(&player.name).bind(at).bind(at).execute(&mut *tx).await?;
        }
    }
    sqlx::query("UPDATE game_server_sessions SET seen_at=? WHERE slug=? AND ended_at IS NULL AND seen_at<=?")
        .bind(at).bind(slug).bind(at - 60).execute(&mut *tx).await?;
    sqlx::query(
        "DELETE FROM game_server_sessions WHERE slug=? AND ended_at IS NOT NULL AND ended_at<?",
    )
    .bind(slug)
    .bind(at - 30 * 86400)
    .execute(&mut *tx)
    .await?;
    let minute = at / 60 * 60;
    sqlx::query(
        "INSERT OR REPLACE INTO game_server_samples(slug,key,at,value) VALUES(?,'players',?,?)",
    )
    .bind(slug)
    .bind(minute)
    .bind(status.players.len() as f64)
    .execute(&mut *tx)
    .await?;
    for stat in status
        .stats
        .iter()
        .filter(|stat| stat.graph && stat.key != "players")
    {
        sqlx::query(
            "INSERT OR REPLACE INTO game_server_samples(slug,key,at,value) SELECT ?,?,?,? WHERE EXISTS(SELECT 1 FROM game_server_samples WHERE slug=? AND key=?) OR (SELECT COUNT(DISTINCT key) FROM game_server_samples WHERE slug=? AND key!='players')<4",
        )
        .bind(slug)
        .bind(&stat.key)
        .bind(minute)
        .bind(stat.value)
        .bind(slug)
        .bind(&stat.key)
        .bind(slug)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("DELETE FROM game_server_samples WHERE at<?")
        .bind(at - 25 * 3600)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut all = s.servers.live.lock().await;
    let live = all.get_mut(slug).unwrap();
    for stat in &status.stats {
        live.stats.insert(stat.key.clone(), stat.clone());
    }
    if live.stats.len() > 16 {
        live.stats
            .retain(|key, _| status.stats.iter().any(|stat| stat.key == *key));
    }
    live.updated_at = Some(at);
    live.status = Some(status);
    drop(all);
    announce(s, slug).await
}

async fn run(s: AppState, a: Auth, mut ws: WebSocket) {
    let first = tokio::time::timeout(Duration::from_secs(5), ws.recv()).await;
    let Ok(Some(Ok(Frame::Text(text)))) = first else {
        return;
    };
    let Ok(RelayFrame::Hello { server }) = serde_json::from_str(&text) else {
        return;
    };
    let slug = server.slug.clone();
    let id = s.id();
    let (tx, mut rx) = mpsc::channel(128);
    {
        let _g = s.writes.lock().await;
        let owner = sqlx::query_scalar::<_, Option<String>>(
            "SELECT token_id FROM game_servers WHERE slug=?",
        )
        .bind(&slug)
        .fetch_optional(&s.db)
        .await;
        let Ok(owner) = owner else {
            return;
        };
        if owner
            .flatten()
            .is_some_and(|token_id| token_id != a.credential)
        {
            let _ = ws
                .send(Frame::Close(Some(CloseFrame {
                    code: 4003,
                    reason: "This server belongs to another relay key; revoke that key to move it"
                        .into(),
                })))
                .await;
            return;
        }
        if hello(&s, &a.credential, server).await.is_err() || end_sessions(&s, &slug).await.is_err()
        {
            return;
        }
        let mut all = s.servers.live.lock().await;
        let live = all.entry(slug.clone()).or_default();
        live.status = None;
        if live
            .busy
            .as_ref()
            .is_some_and(|(_, _, _, at)| at.elapsed() >= Duration::from_secs(300))
        {
            live.busy = None;
        }
        if let Some((command_id, action, by, _)) = &live.busy {
            let _ = tx.try_send(RelayFrame::Command {
                command_id: command_id.clone(),
                action: action.clone(),
                by: by.clone(),
            });
        }
        live.connection = Some(Connection {
            id: id.clone(),
            tx,
            pending: HashMap::new(),
        });
        drop(all);
        let _ = announce(&s, &slug).await;
    }
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut seen = Instant::now();
    let mut ping = Instant::now();
    let mut checked = Instant::now();
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if s.servers.live.lock().await.get(&slug).and_then(|l| l.connection.as_ref()).is_none_or(|c| c.id != id)
                    || seen.elapsed() > Duration::from_secs(45) { break; }
                if let Some(live) = s.servers.live.lock().await.get_mut(&slug) {
                    if live.busy.as_ref().is_some_and(|(_, _, _, at)| at.elapsed() >= Duration::from_secs(300)) {
                        live.busy = None;
                    }
                    if let Some(c) = live.connection.as_mut() {
                        c.pending.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(300));
                    }
                }
                if checked.elapsed() >= Duration::from_secs(2) {
                    if !a.valid(&s).await || sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id=? AND removed_at IS NULL")
                        .bind(&a.user.id).fetch_optional(&s.db).await.ok().flatten().as_deref() != Some("admin") { break; }
                    checked = Instant::now();
                }
                if ping.elapsed() >= Duration::from_secs(15) {
                    if !matches!(tokio::time::timeout(Duration::from_secs(2), ws.send(Frame::Ping(vec![].into()))).await, Ok(Ok(()))) { break; }
                    ping = Instant::now();
                }
            }
            frame = rx.recv() => {
                let Some(frame) = frame else { break; };
                if !matches!(tokio::time::timeout(Duration::from_secs(2), ws.send(Frame::Text(serde_json::to_string(&frame).unwrap().into()))).await, Ok(Ok(()))) { break; }
            }
            incoming = ws.recv() => {
                match incoming {
                    Some(Ok(Frame::Pong(_) | Frame::Ping(_))) => seen = Instant::now(),
                    Some(Ok(Frame::Text(text))) => {
                        seen = Instant::now();
                        let Ok(frame) = serde_json::from_str::<RelayFrame>(&text) else { break; };
                        let _g = s.writes.lock().await;
                        if s.servers.live.lock().await.get(&slug).and_then(|l| l.connection.as_ref()).is_none_or(|c| c.id != id)
                            && !matches!(frame, RelayFrame::Result { .. }) { break; }
                        if !a.valid(&s).await || sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id=? AND removed_at IS NULL")
                            .bind(&a.user.id).fetch_optional(&s.db).await.ok().flatten().as_deref() != Some("admin") { break; }
                        match frame {
                            RelayFrame::Hello { server } => {
                                if server.slug != slug || hello(&s, &a.credential, server).await.is_err() { break; }
                                if announce(&s, &slug).await.is_err() { break; }
                            }
                            RelayFrame::Status { status: value } => { if status(&s, &slug, value).await.is_err() { break; } }
                            result @ RelayFrame::Result { .. } => {
                                let RelayFrame::Result { ref command_id, .. } = result else { unreachable!() };
                                if let Some(live) = s.servers.live.lock().await.get_mut(&slug) {
                                    if live.busy.as_ref().is_some_and(|(busy_id, _, _, _)| busy_id == command_id) {
                                        live.busy = None;
                                    }
                                    if let Some((_, tx)) = live.connection.as_mut().filter(|c| c.id == id).and_then(|c| c.pending.remove(command_id)) { let _ = tx.send(result); }
                                }
                            }
                            _ => break,
                        }
                    }
                    _ => break,
                }
            }
        }
    }
    let _g = s.writes.lock().await;
    let mut all = s.servers.live.lock().await;
    if let Some(live) = all
        .get_mut(&slug)
        .filter(|l| l.connection.as_ref().is_some_and(|c| c.id == id))
    {
        live.connection = None;
        drop(all);
        let _ = end_sessions(&s, &slug).await;
        let _ = announce(&s, &slug).await;
    }
}
