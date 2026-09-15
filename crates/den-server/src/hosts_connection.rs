use super::*;
use axum::extract::ws::{Message as Frame, WebSocket};

// Caller holds writes so the initial snapshot and admission of new Opens are
// ordered. Read every state before issuing any command; an error is not absence.
async fn reconcile(
    s: &AppState,
    host: &str,
    inventory: Option<&[String]>,
    initial: bool,
) -> Result<Vec<HostFrame>> {
    if !initial && inventory.is_none() {
        return Ok(vec![]);
    }
    if inventory.is_some_and(|ids| ids.len() > 16) {
        return Err(Error::bad("Host inventory exceeds session capacity"));
    }
    let ids = sqlx::query_scalar::<_, String>("SELECT id FROM terminal_sessions WHERE host_id=?")
        .bind(host)
        .fetch_all(&s.db)
        .await?;
    let mut active = Vec::new();
    for id in ids {
        let t = terminal::load(s, &id).await?;
        if t.ended_at.is_none() {
            active.push(t);
        }
    }
    let mut frames = Vec::new();
    if let Some(inventory) = inventory {
        for id in inventory {
            if !active.iter().any(|t| &t.id == id) {
                frames.push(HostFrame::Close {
                    session_id: id.clone(),
                });
            }
        }
        if initial {
            for t in active {
                if inventory.contains(&t.id) {
                    // Release the 1 MiB output window after a lost Ack. Resize
                    // reattaches without Open's ability to create a new shell.
                    frames.push(HostFrame::Ack {
                        session_id: t.id.clone(),
                        bytes: 1024 * 1024,
                    });
                    frames.push(HostFrame::Resize {
                        session_id: t.id,
                        cols: t.cols,
                        rows: t.rows,
                    });
                } else {
                    terminal::finish(s, &t.id).await?;
                }
            }
        }
    } else {
        // Older hosts cannot report retained PTYs. Keep their existing replay
        // behavior until they are upgraded; absence of the field is not [].
        for t in active {
            frames.push(HostFrame::Open {
                session_id: t.id,
                cols: t.cols,
                rows: t.rows,
                shell: None,
            });
        }
    }
    Ok(frames)
}

// One deadline for the whole batch. Reconciliation transmits under the writes
// lock, so a host that stops reading must not cost two seconds per frame.
async fn transmit(ws: &mut WebSocket, frames: &[HostFrame]) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(2), async {
            for frame in frames {
                let bytes = serde_json::to_vec(frame).unwrap();
                if ws.send(Frame::Binary(bytes.into())).await.is_err() {
                    return false;
                }
            }
            true
        })
        .await,
        Ok(true)
    )
}

pub(super) async fn run(s: AppState, host: Host, mut ws: WebSocket) {
    let connection = s.id();
    let (tx, mut rx) = mpsc::channel(128);
    {
        let _g = s.writes.lock().await;
        s.hosts.connections.lock().await.insert(
            host.id.clone(),
            Connection {
                id: connection.clone(),
                tx,
                ready: false,
            },
        );
    }
    let mut ready = false;
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut seen = Instant::now();
    let connected_at = Instant::now();
    loop {
        tokio::select! {
            _=tick.tick()=>{
                let current=s.hosts.connections.lock().await.get(&host.id).map(|c|c.id.clone());
                if current.as_deref()!=Some(&connection) || seen.elapsed()>Duration::from_secs(45)
                    || (!ready && connected_at.elapsed()>Duration::from_secs(5)) {break;}
                if ws.send(Frame::Ping(vec![].into())).await.is_err(){break;}
            }
            frame=rx.recv(), if ready=>{
                let Some(frame)=frame else{break;};
                if !transmit(&mut ws, std::slice::from_ref(&frame)).await {break;}
            }
            incoming=ws.recv()=>{
                match incoming {
                    Some(Ok(Frame::Pong(_)|Frame::Ping(_)))=>seen=Instant::now(),
                    Some(Ok(Frame::Binary(b)))=>{
                        seen=Instant::now();let Ok(frame)=serde_json::from_slice::<HostFrame>(&b) else{break;};
                        if !ready && !matches!(frame,HostFrame::Hello{..}) {break;}
                        match frame {
                            HostFrame::Hello{direct_url,session_ids}=>{
                                let _g=s.writes.lock().await;
                                if s.hosts.connections.lock().await.get(&host.id).is_none_or(|c|c.id!=connection) {break;}
                                let Ok(frames)=reconcile(&s,&host.id,session_ids.as_deref(),!ready).await else {break;};
                                let direct_url=direct_url.filter(|url|valid_direct_url(url));
                                if sqlx::query("UPDATE hosts SET direct_url=?,online=1,last_seen=? WHERE id=?").bind(direct_url).bind(now()).bind(&host.id).execute(&s.db).await.is_err(){break;}
                                if !transmit(&mut ws,&frames).await {break;}
                                s.hosts.connections.lock().await.get_mut(&host.id).unwrap().ready=true;
                                ready=true;
                            }
                            HostFrame::Output{session_id,bytes}=>{
                                if !terminal::on_host(&s,&session_id,&host.id).await {break;}
                                let n=bytes.len();
                                if terminal::output(&s,&session_id,bytes).await.is_err(){break;}
                                if send(&s,&host.id,HostFrame::Ack{session_id,bytes:n}).await.is_err(){break;}
                            }
                            HostFrame::Scrollback{session_id,connection_id,bytes}=>{
                                if !terminal::on_host(&s,&session_id,&host.id).await{break;}
                                let _=s.events.send(Event::TerminalOutput{session_id,bytes,connection_id:Some(connection_id)});
                            }
                            HostFrame::Exited{session_id,..}=>{
                                if !terminal::on_host(&s,&session_id,&host.id).await{break;}
                                let _g=s.writes.lock().await;let _=terminal::finish(&s,&session_id).await;
                            }
                            _=>break,
                        }
                    }
                    _=>break,
                }
            }
        }
    }
    let mut connections = s.hosts.connections.lock().await;
    if connections
        .get(&host.id)
        .is_some_and(|c| c.id == connection)
    {
        connections.remove(&host.id);
        let _ = sqlx::query("UPDATE hosts SET online=0,last_seen=? WHERE id=?")
            .bind(now())
            .bind(&host.id)
            .execute(&s.db)
            .await;
    }
}
