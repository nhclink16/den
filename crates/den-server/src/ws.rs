use crate::{auth::Auth, chat::visible, *};
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};

#[utoipa::path(get,path="/presence",responses((status=200,body=PresenceState)))]
pub(crate) async fn presence(State(s): State<AppState>, _a: Auth) -> Json<PresenceState> {
    let mut presence = snapshot(&s);
    for o in objects_snapshot(&s) {
        if visible(&s, &_a.user.id, &o.channel_id).await.is_ok() {
            presence.objects.push(o);
        }
    }
    Json(presence)
}
fn snapshot(s: &AppState) -> PresenceState {
    let mut online_user_ids = s
        .presence
        .lock()
        .expect("presence mutex")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    online_user_ids.sort();
    PresenceState {
        online_user_ids,
        objects: Vec::new(),
    }
}
struct Connected {
    state: AppState,
    user: String,
}
impl Connected {
    fn new(state: AppState, user: String) -> Self {
        let mut users = state.presence.lock().expect("presence mutex");
        let count = users.entry(user.clone()).or_default();
        *count += 1;
        if *count == 1 {
            let _ = state.events.send(Event::Presence {
                user_id: user.clone(),
                online: true,
            });
        }
        drop(users);
        Self { state, user }
    }
}
impl Drop for Connected {
    fn drop(&mut self) {
        let mut users = self.state.presence.lock().expect("presence mutex");
        if let Some(count) = users.get_mut(&self.user) {
            *count -= 1;
            if *count == 0 {
                users.remove(&self.user);
                let _ = self.state.events.send(Event::Presence {
                    user_id: self.user.clone(),
                    online: false,
                });
            }
        }
    }
}
#[utoipa::path(get,path="/ws",responses((status=101,description="Event stream; first event requires resync"),(status=401,body=ApiError)))]
pub(crate) async fn connect(State(s): State<AppState>, a: Auth, ws: WebSocketUpgrade) -> Response {
    let rx = s.events.subscribe();
    ws.max_message_size(128 * 1024)
        .max_frame_size(128 * 1024)
        .on_upgrade(move |socket| run(s, a, socket, rx))
}
async fn event(socket: &mut WebSocket, event: &Event) -> bool {
    let Ok(json) = serde_json::to_string(event) else {
        return false;
    };
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            socket.send(if matches!(event, Event::TerminalOutput { .. }) {
                Frame::Binary(json.into_bytes().into())
            } else {
                Frame::Text(json.into())
            })
        )
        .await,
        Ok(Ok(()))
    )
}
async fn allowed(s: &AppState, a: &Auth, v: &Event) -> bool {
    let channel = match v {
        Event::TerminalOutput { .. } => return false,
        Event::TerminalState { session } => {
            return terminal::can_view(s, &a.user.id, &session.id).await
        }
        Event::AccessDecided { user_id, .. } => return user_id == &a.user.id,
        Event::Notification {
            user_id, message, ..
        } => {
            if user_id != &a.user.id {
                return false;
            }
            Some(&message.channel_id)
        }
        Event::ReadStateUpdated { user_id, state } => {
            if user_id != &a.user.id {
                return false;
            }
            Some(&state.channel_id)
        }
        Event::AppearanceUpdated { user_id, .. }
        | Event::NotificationPreferencesUpdated { user_id, .. } => return user_id == &a.user.id,
        Event::MessageCreated(m) | Event::MessageEdited(m) => Some(&m.channel_id),
        Event::MessageDeleted { channel_id, .. }
        | Event::Typing { channel_id, .. }
        | Event::ReactionsUpdated { channel_id, .. }
        | Event::ObjectPatched { channel_id, .. }
        | Event::ObjectPresence { channel_id, .. }
        | Event::CallState { channel_id, .. } => Some(channel_id),
        Event::ObjectCursor { .. } => return false, // Checked against socket-local open objects below.
        Event::SettingsUpdated { .. } | Event::Presence { .. } | Event::Resync { .. } => None,
    };
    if let Some(channel) = channel {
        visible(s, &a.user.id, channel).await.is_ok()
    } else {
        true
    }
}
async fn run(s: AppState, a: Auth, mut socket: WebSocket, mut rx: broadcast::Receiver<Event>) {
    if !a.valid(&s).await {
        return;
    }
    if !event(
        &mut socket,
        &Event::Resync {
            reason: "connected; refetch channel state, presence, and /calls".into(),
        },
    )
    .await
    {
        return;
    }
    let _connected = Connected::new(s.clone(), a.user.id.clone());
    let connection = s.id();
    let mut terminals = std::collections::HashSet::<String>::new();
    let mut timer = tokio::time::interval(Duration::from_millis(500));
    let mut last_seen = Instant::now();
    let mut opened = OpenObjects {
        state: s.clone(),
        user: a.user.id.clone(),
        ids: HashMap::new(),
    };
    let mut cursors = (Instant::now(), 0u32);
    let mut last_typing = HashMap::<String, Instant>::new();
    loop {
        tokio::select! {
            _=timer.tick()=>{
                if !a.valid(&s).await || last_seen.elapsed()>Duration::from_secs(45) {break;}
                for id in terminals.clone() {
                    if !terminal::can_view(&s,&a.user.id,&id).await {terminals.remove(&id);let _=terminal::viewer(&s,&id,&a.user.id,&connection,false).await;}
                }
                if !matches!(tokio::time::timeout(Duration::from_secs(5),socket.send(Frame::Ping(Vec::new().into()))).await,Ok(Ok(()))) {break;}
            },
            incoming=socket.recv()=>{
                match incoming {
                    Some(Ok(Frame::Pong(_)|Frame::Ping(_)))=>last_seen=Instant::now(),
                    Some(Ok(Frame::Binary(bytes)))=>{
                        if !a.valid(&s).await {break;}
                        last_seen=Instant::now();
                        let Ok(frame)=serde_json::from_slice::<TerminalFrame>(&bytes) else{break;};
                        match frame {
                            TerminalFrame::TerminalOpen{session_id}=>{
                                if terminals.len()>=16 || !terminal::can_view(&s,&a.user.id,&session_id).await {continue;}
                                if let Ok(t)=terminal::load(&s,&session_id).await {
                                    terminals.insert(session_id.clone());
                                    let _=terminal::viewer(&s,&session_id,&a.user.id,&connection,true).await;
                                    let _=hosts::send(&s,&t.host_id,HostFrame::Replay{session_id,connection_id:connection.clone()}).await;
                                }
                            }
                            TerminalFrame::TerminalClose{session_id}=>{terminals.remove(&session_id);let _=terminal::viewer(&s,&session_id,&a.user.id,&connection,false).await;}
                            frame=>{let _=terminal::input(&s,&a.user.id,frame).await;}
                        }
                    },
                    Some(Ok(Frame::Text(text)))=>{
                        if !a.valid(&s).await {break;}
                        last_seen=Instant::now();
                        let Ok(v)=serde_json::from_str::<ClientEvent>(&text) else {break;};
                        match v {
                            ClientEvent::Typing { channel_id } => {
                                if visible(&s,&a.user.id,&channel_id).await.is_err(){break;}
                                last_typing.retain(|_,at|at.elapsed()<Duration::from_secs(2));
                                if last_typing.len()<20 && !last_typing.contains_key(&channel_id) {
                                    last_typing.insert(channel_id.clone(),Instant::now());
                                    let _=s.events.send(Event::Typing{channel_id,user_id:a.user.id.clone()});
                                }
                            }
                            ClientEvent::ObjectOpen { object_id } => {
                                if opened.ids.contains_key(&object_id) || opened.ids.len() >= 20 {continue;}
                                let Ok(o) = objects::load(&s, &object_id).await else {continue;};
                                if visible(&s, &a.user.id, &o.summary.channel_id).await.is_err() {continue;}
                                opened.change(&object_id, &o.summary.channel_id, true);
                            }
                            ClientEvent::ObjectClose { object_id } => {
                                if let Some(channel) = opened.ids.get(&object_id).cloned() {opened.change(&object_id, &channel, false);}
                            }
                            ClientEvent::ObjectCursor { object_id, x, y, page_id } => {
                                if cursors.0.elapsed() >= Duration::from_secs(1) {cursors = (Instant::now(), 0);}
                                if cursors.1 >= 20 || !x.is_finite() || !y.is_finite() || page_id.len()>256 {continue;}
                                cursors.1 += 1;
                                let Some(channel) = opened.ids.get(&object_id) else {continue;};
                                if visible(&s, &a.user.id, channel).await.is_err() {continue;}
                                let _ = s.events.send(Event::ObjectCursor {id: object_id, user_id: a.user.id.clone(), x, y, page_id});
                            }
                        }
                    },Some(Ok(Frame::Close(_)))|None|Some(Err(_))=>break,

                }
            },
            incoming=rx.recv()=>{
                if !a.valid(&s).await {break;}
                match incoming {
                    Ok(v)=>{
                        let permitted = if let Event::TerminalOutput{session_id,connection_id,..} = &v {
                            terminals.contains(session_id) && connection_id.as_ref().is_none_or(|id|id==&connection) && terminal::can_view(&s,&a.user.id,session_id).await
                        } else if let Event::ObjectCursor { id, user_id, .. } = &v {
                            if let Some(channel) = opened.ids.get(id) {user_id != &a.user.id && visible(&s, &a.user.id, channel).await.is_ok()} else {false}
                        } else {allowed(&s,&a,&v).await};
                        if permitted && !event(&mut socket,&v).await {break;}
                    },
                    Err(broadcast::error::RecvError::Lagged(_))=>{let _=event(&mut socket,&Event::Resync{reason:"slow consumer; reconnect and refetch".into()}).await;break;},
                    Err(_)=>break,
                }
            }
        }
    }
    for id in terminals {
        let _ = terminal::viewer(&s, &id, &a.user.id, &connection, false).await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.send(Frame::Close(None))).await;
}

fn objects_snapshot(s: &AppState) -> Vec<ObjectPresence> {
    s.object_presence
        .lock()
        .expect("object presence")
        .iter()
        .map(|(id, (channel, users))| {
            let mut user_ids: Vec<_> = users.keys().cloned().collect();
            user_ids.sort();
            ObjectPresence {
                id: id.clone(),
                channel_id: channel.clone(),
                user_ids,
            }
        })
        .collect()
}
// Per-socket ownership makes closing one device preserve the user's other devices.
struct OpenObjects {
    state: AppState,
    user: String,
    ids: HashMap<String, String>,
}
impl OpenObjects {
    fn change(&mut self, id: &str, channel: &str, open: bool) {
        let mut all = self.state.object_presence.lock().expect("object presence");
        let (_, users) = all
            .entry(id.into())
            .or_insert_with(|| (channel.into(), HashMap::new()));
        let before = users.contains_key(&self.user);
        if open {
            *users.entry(self.user.clone()).or_default() += 1;
            self.ids.insert(id.into(), channel.into());
        } else {
            if let Some(n) = users.get_mut(&self.user) {
                *n -= 1;
                if *n == 0 {
                    users.remove(&self.user);
                }
            }
            self.ids.remove(id);
        }
        if before != users.contains_key(&self.user) {
            let mut user_ids: Vec<_> = users.keys().cloned().collect();
            user_ids.sort();
            let _ = self.state.events.send(Event::ObjectPresence {
                id: id.into(),
                channel_id: channel.into(),
                user_ids,
            });
        }
        if users.is_empty() {
            all.remove(id);
        }
    }
}
impl Drop for OpenObjects {
    fn drop(&mut self) {
        for (id, channel) in self.ids.clone() {
            self.change(&id, &channel, false);
        }
    }
}
