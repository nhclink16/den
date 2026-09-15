use crate::{auth::Auth, hosts, *};
use axum::extract::Path;
use base64::{engine::general_purpose::STANDARD, Engine};
use tokio::io::AsyncWriteExt;

pub(crate) async fn load(s: &AppState, id: &str) -> Result<TerminalState> {
    let o = objects::load(s, id).await?;
    serde_json::from_value(
        o.state
            .get("terminal")
            .cloned()
            .ok_or_else(Error::missing)?,
    )
    .map_err(|_| Error::missing())
}
pub(crate) async fn on_host(s: &AppState, id: &str, host: &str) -> bool {
    load(s, id)
        .await
        .is_ok_and(|t| t.host_id == host && t.ended_at.is_none())
}
pub(crate) async fn can_view(s: &AppState, user: &str, id: &str) -> bool {
    let Ok(t) = load(s, id).await else {
        return false;
    };
    if !hosts::permitted(s, user, &t.host_id, false).await {
        return false;
    }
    let Ok(channels)=sqlx::query_scalar::<_,String>("SELECT o.channel_id FROM terminal_cards c JOIN objects o ON o.id=c.object_id WHERE c.session_id=?").bind(id).fetch_all(&s.db).await else{return false;};
    for c in channels {
        if chat::visible(s, user, &c).await.is_ok() {
            return true;
        }
    }
    false
}
// Keep the object/message insertion fields together at the transaction boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_object(
    s: &AppState,
    id: &str,
    channel: &str,
    user: &str,
    kind: &str,
    name: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<Object> {
    let message = s.id();
    let mut tx = s.db.begin().await?;
    sqlx::query("INSERT INTO messages(id,channel_id,author_id,content) VALUES(?,?,?,'')")
        .bind(&message)
        .bind(channel)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let state = serde_json::to_string(&serde_json::json!({key:value})).unwrap();
    sqlx::query("INSERT INTO objects(id,channel_id,message_id,kind,name,state,created_by) VALUES(?,?,?,?,?,?,?)").bind(id).bind(channel).bind(&message).bind(kind).bind(name).bind(state).bind(user).execute(&mut *tx).await?;
    tx.commit().await?;
    let msg = messages::get_message(s, &message).await?;
    let _ = s.events.send(Event::MessageCreated(msg.clone()));
    let _ = inbox::changed(s, channel, Some(&msg)).await;
    objects::load(s, id).await
}
pub(crate) async fn state(
    s: &AppState,
    id: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<()> {
    sqlx::query("UPDATE objects SET state=?,version=version+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(serde_json::to_string(&serde_json::json!({key:value})).unwrap()).bind(id).execute(&s.db).await?;
    let o = objects::load(s, id).await?;
    let _ = s.events.send(Event::ObjectPatched {
        id: id.into(),
        channel_id: o.summary.channel_id,
        version: o.summary.version,
        put: vec![],
        remove: vec![],
        author_id: o.summary.created_by,
    });
    if let Some(message) = o.summary.message_id {
        let _ = s.events.send(Event::MessageEdited(
            messages::get_message(s, &message).await?,
        ));
    }
    Ok(())
}
pub(crate) async fn save(s: &AppState, t: &TerminalState) -> Result<()> {
    let ids =
        sqlx::query_scalar::<_, String>("SELECT object_id FROM terminal_cards WHERE session_id=?")
            .bind(&t.id)
            .fetch_all(&s.db)
            .await?;
    for id in ids {
        state(s, &id, "terminal", serde_json::to_value(t).unwrap()).await?;
    }
    let _ = s.events.send(Event::TerminalState { session: t.clone() });
    Ok(())
}
#[utoipa::path(post,path="/hosts/{id}/sessions",params(("id"=String,Path)),request_body=OpenTerminal,responses((status=200,body=Object)))]
pub(crate) async fn open(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<OpenTerminal>,
) -> Result<Json<Object>> {
    // Before the default self-DM is created: a refused request must leave nothing.
    threads::unsupported_context(
        v.thread_id.as_deref(),
        v.task_id.as_deref(),
        v.reply_to.as_deref(),
    )?;
    let host = hosts::load(&s, &id).await?;
    if !hosts::permitted(&s, &a.user.id, &id, true).await {
        return Err(Error::missing());
    }
    if !host.online {
        return Err(Error::conflict("Machine is offline"));
    }
    let channel = if let Some(id) = v.channel_id {
        chat::visible(&s, &a.user.id, &id).await?
    } else {
        chat::dm(
            State(s.clone()),
            a.clone(),
            ApiJson(CreateDm {
                member_ids: vec![a.user.id.clone()],
            }),
        )
        .await?
        .0
    };
    if channel.kind == ChannelKind::Voice {
        return Err(Error::bad("Choose a text channel or DM"));
    }
    let (cols, rows) = (v.cols.unwrap_or(100), v.rows.unwrap_or(28));
    dimensions(cols, rows)?;
    let _g = s.writes.lock().await;
    let id = s.id();
    let t = TerminalState {
        id: id.clone(),
        host_id: host.id.clone(),
        host_name: host.name.clone(),
        owner_id: host.owner_id.clone(),
        cols,
        rows,
        active_controller_id: Some(host.owner_id.clone()),
        viewer_ids: vec![],
        ended_at: None,
        started_at: now(),
        recording_upload_id: None,
        recording_capped: false,
        recording_enabled: terminal_recording::preference(&s, &host.owner_id, &host.id).await?,
        control_request_ids: vec![],
    };
    let o = create_object(
        &s,
        &id,
        &channel.id,
        &a.user.id,
        "terminal",
        &host.name,
        "terminal",
        serde_json::to_value(&t).unwrap(),
    )
    .await?;
    sqlx::query("INSERT INTO terminal_sessions(id,host_id) VALUES(?,?)")
        .bind(&id)
        .bind(&host.id)
        .execute(&s.db)
        .await?;
    sqlx::query("INSERT INTO terminal_cards VALUES(?,?)")
        .bind(&id)
        .bind(&id)
        .execute(&s.db)
        .await?;
    hosts::send(
        &s,
        &host.id,
        HostFrame::Open {
            session_id: id.clone(),
            cols,
            rows,
            shell: None,
        },
    )
    .await?;
    hosts::audit(&s, &host, &a.user.id, "session_start", Some(&id)).await?;
    Ok(Json(o))
}
fn dimensions(cols: u16, rows: u16) -> Result<()> {
    if !(2..=500).contains(&cols) || !(2..=300).contains(&rows) {
        Err(Error::bad("Invalid terminal size"))
    } else {
        Ok(())
    }
}
#[utoipa::path(post,path="/sessions/{id}/controller",params(("id"=String,Path)),request_body=SetController,responses((status=200,body=TerminalState)))]
pub(crate) async fn controller(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<SetController>,
) -> Result<Json<TerminalState>> {
    let _g = s.writes.lock().await;
    let mut t = load(&s, &id).await?;
    if t.owner_id != a.user.id {
        return Err(Error::missing());
    }
    if t.ended_at.is_some() {
        return Err(Error::conflict("Session ended"));
    }
    if let Some(user) = &v.user_id {
        if !hosts::permitted(&s, user, &t.host_id, true).await {
            return Err(Error::forbidden());
        }
    }
    t.active_controller_id = v.user_id;
    t.control_request_ids.clear();
    save(&s, &t).await?;
    hosts::audit(
        &s,
        &hosts::load(&s, &t.host_id).await?,
        &a.user.id,
        "controller",
        t.active_controller_id.as_deref(),
    )
    .await?;
    Ok(Json(t))
}
#[utoipa::path(post,path="/sessions/{id}/request-control",params(("id"=String,Path)),responses((status=200,body=TerminalState)))]
pub(crate) async fn request_control(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<TerminalState>> {
    let _g = s.writes.lock().await;
    let mut t = load(&s, &id).await?;
    if t.ended_at.is_some()
        || !can_view(&s, &a.user.id, &id).await
        || !hosts::permitted(&s, &a.user.id, &t.host_id, true).await
    {
        return Err(Error::missing());
    }
    if !t.control_request_ids.contains(&a.user.id) {
        t.control_request_ids.push(a.user.id);
        save(&s, &t).await?;
    }
    Ok(Json(t))
}
#[utoipa::path(post,path="/sessions/{id}/share",params(("id"=String,Path)),request_body=ShareTerminal,responses((status=200,body=Object)))]
pub(crate) async fn share(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<ShareTerminal>,
) -> Result<Json<Object>> {
    threads::unsupported_context(
        v.thread_id.as_deref(),
        v.task_id.as_deref(),
        v.reply_to.as_deref(),
    )?;
    let _g = s.writes.lock().await;
    let t = load(&s, &id).await?;
    if !can_view(&s, &a.user.id, &id).await {
        return Err(Error::missing());
    }
    if chat::visible(&s, &a.user.id, &v.channel_id).await?.kind == ChannelKind::Voice {
        return Err(Error::bad("Choose a text channel"));
    }
    let object = s.id();
    let o = create_object(
        &s,
        &object,
        &v.channel_id,
        &a.user.id,
        "terminal",
        &t.host_name,
        "terminal",
        serde_json::to_value(&t).unwrap(),
    )
    .await?;
    sqlx::query("INSERT INTO terminal_cards VALUES(?,?)")
        .bind(object)
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(Json(o))
}
#[utoipa::path(post,path="/sessions/{id}/write",params(("id"=String,Path)),request_body=TerminalWrite,responses((status=204)))]
pub(crate) async fn write(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<TerminalWrite>,
) -> Result<StatusCode> {
    input(
        &s,
        &a.user.id,
        TerminalFrame::TerminalInput {
            session_id: id,
            bytes: v.text.into_bytes(),
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn input(s: &AppState, user: &str, frame: TerminalFrame) -> Result<()> {
    let _g = s.writes.lock().await;
    let id = match &frame {
        TerminalFrame::TerminalInput { session_id, .. }
        | TerminalFrame::TerminalResize { session_id, .. } => session_id,
        _ => return Err(Error::bad("Expected terminal input")),
    };
    let mut t = load(s, id).await?;
    if !can_view(s, user, id).await {
        return Err(Error::missing());
    }
    if t.ended_at.is_some()
        || t.active_controller_id.as_deref() != Some(user)
        || !hosts::permitted(s, user, &t.host_id, true).await
    {
        return Err(Error::forbidden());
    }
    let frame = match frame {
        TerminalFrame::TerminalInput { session_id, bytes } => {
            if bytes.len() > 16384 {
                return Err(Error::bad("Input exceeds 16 KiB"));
            }
            HostFrame::Input { session_id, bytes }
        }
        TerminalFrame::TerminalResize {
            session_id,
            cols,
            rows,
        } => {
            dimensions(cols, rows)?;
            t.cols = cols;
            t.rows = rows;
            save(s, &t).await?;
            HostFrame::Resize {
                session_id,
                cols,
                rows,
            }
        }
        _ => unreachable!(),
    };
    hosts::send(s, &t.host_id, frame).await
}
pub(crate) async fn output(s: &AppState, id: &str, bytes: Vec<u8>) -> Result<()> {
    let _g = s.writes.lock().await;
    let mut t = load(s, id).await?;
    if t.ended_at.is_some() {
        return Ok(());
    }
    if t.recording_enabled && !t.recording_capped {
        let path = s.uploads.join(format!("{id}.recording"));
        let ms = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            - t.started_at * 1000)
            .max(0);
        // CSI window resize keeps the original grid available to the replay renderer.
        let mut recorded = format!("\x1b[8;{};{}t", t.rows, t.cols).into_bytes();
        recorded.extend_from_slice(&bytes);
        let line = format!("{}\n", serde_json::json!([ms, STANDARD.encode(&recorded)]));
        let size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if size + line.len() as u64 > 64 * 1024 * 1024 {
            t.recording_capped = true;
            save(s, &t).await?;
        } else {
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;
            file.write_all(line.as_bytes()).await?;
            // Complete the blocking file write before acknowledging output or ending a session.
            file.flush().await?;
        }
    }
    let _ = s.events.send(Event::TerminalOutput {
        session_id: id.into(),
        bytes,
        connection_id: None,
    });
    Ok(())
}
pub(crate) async fn finish(s: &AppState, id: &str) -> Result<()> {
    let mut t = load(s, id).await?;
    if t.ended_at.is_some() {
        return Ok(());
    }
    t.ended_at = Some(now());
    t.active_controller_id = None;
    t.viewer_ids.clear();
    if !t.recording_enabled {
        terminal_recording::discard(s, id).await?;
    }
    let path = s.uploads.join(format!("{id}.recording"));
    if let Ok(meta) = tokio::fs::metadata(&path).await {
        if meta.len() > 0 {
            let upload = s.id();
            let o = objects::load(s, id).await?;
            tokio::fs::rename(path, s.uploads.join(&upload)).await?;
            sqlx::query("INSERT INTO uploads(id,channel_id,owner_id,filename,content_type,size,offset,complete,touched_at) VALUES(?,?,?,'terminal-recording.jsonl','application/octet-stream',?,?,1,?)").bind(&upload).bind(&o.summary.channel_id).bind(&t.owner_id).bind(meta.len() as i64).bind(meta.len() as i64).bind(now()).execute(&s.db).await?;
            sqlx::query("UPDATE terminal_sessions SET recording_upload_id=? WHERE id=?")
                .bind(&upload)
                .bind(id)
                .execute(&s.db)
                .await?;
            sqlx::query("INSERT INTO terminal_recordings VALUES(?,?)")
                .bind(&upload)
                .bind(id)
                .execute(&s.db)
                .await?;
            t.recording_upload_id = Some(upload);
        }
    }
    save(s, &t).await?;
    Ok(())
}
#[utoipa::path(delete,path="/sessions/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn close(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let _g = s.writes.lock().await;
    let t = load(&s, &id).await?;
    if t.owner_id != a.user.id {
        return Err(Error::missing());
    }
    let _ = hosts::send(
        &s,
        &t.host_id,
        HostFrame::Close {
            session_id: id.clone(),
        },
    )
    .await;
    finish(&s, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn revoke_user(s: &AppState, host: &str, user: &str) -> Result<()> {
    let ids = sqlx::query_scalar::<_, String>("SELECT ts.id FROM terminal_sessions ts JOIN objects o ON o.id=ts.id WHERE ts.host_id=? AND json_extract(o.state,'$.terminal.ended_at') IS NULL")
        .bind(host)
        .fetch_all(&s.db)
        .await?;
    for id in ids {
        let mut t = load(s, &id).await?;
        let before = t.clone();
        if !hosts::permitted(s, user, host, true).await
            && t.active_controller_id.as_deref() == Some(user)
        {
            t.active_controller_id = Some(t.owner_id.clone());
        }
        if !hosts::permitted(s, user, host, false).await {
            t.viewer_ids.retain(|u| u != user);
        }
        t.control_request_ids.retain(|u| u != user);
        if t != before {
            save(s, &t).await?;
        }
    }
    Ok(())
}
#[utoipa::path(post,path="/sessions/{id}/direct-token",params(("id"=String,Path)),responses((status=200,body=DirectToken)))]
pub(crate) async fn direct_token(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<DirectToken>> {
    if !can_view(&s, &a.user.id, &id).await {
        return Err(Error::missing());
    }
    let t = load(&s, &id).await?;
    let h = hosts::load(&s, &t.host_id).await?;
    let token = auth::secret();
    let expires_at = now() + 60;
    let mut tokens = s.hosts.direct_tokens.lock().await;
    tokens.retain(|_, v| v.2 > now());
    tokens.insert(auth::hash(&token), (a.user.id, id, expires_at));
    Ok(Json(DirectToken {
        token,
        url: h.direct_url,
        expires_at,
    }))
}
pub(crate) async fn viewer(
    s: &AppState,
    id: &str,
    user: &str,
    connection: &str,
    open: bool,
) -> Result<()> {
    let _g = s.writes.lock().await;
    let mut t = load(s, id).await?;
    let mut viewers = s.hosts.viewers.lock().await;
    let key = (id.to_string(), user.to_string());
    let connections = viewers.entry(key.clone()).or_default();
    if open {
        connections.insert(connection.into());
    } else {
        connections.remove(connection);
    }
    let present = !connections.is_empty();
    if !present {
        viewers.remove(&key);
    }
    drop(viewers);
    let before = t.viewer_ids.contains(&user.to_string());
    if present && !before {
        t.viewer_ids.push(user.into());
    } else if !present {
        t.viewer_ids.retain(|u| u != user);
    }
    if before != present {
        save(s, &t).await?;
    }
    if open && user != t.owner_id {
        let name = sqlx::query_scalar("SELECT username FROM users WHERE id=?")
            .bind(user)
            .fetch_one(&s.db)
            .await?;
        hosts::send(
            s,
            &t.host_id,
            HostFrame::Viewer {
                session_id: id.into(),
                user_id: user.into(),
                name,
            },
        )
        .await?;
    }
    Ok(())
}
