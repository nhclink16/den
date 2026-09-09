use crate::{auth::Auth, chat::visible, *};
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};

#[utoipa::path(get,path="/presence",responses((status=200,body=PresenceState)))]
pub(crate) async fn presence(State(s): State<AppState>, _a: Auth) -> Json<PresenceState> {
    Json(snapshot(&s))
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
    PresenceState { online_user_ids }
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
    ws.max_message_size(4096)
        .max_frame_size(4096)
        .on_upgrade(move |socket| run(s, a, socket, rx))
}
async fn event(socket: &mut WebSocket, event: &Event) -> bool {
    let Ok(json) = serde_json::to_string(event) else {
        return false;
    };
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            socket.send(Frame::Text(json.into()))
        )
        .await,
        Ok(Ok(()))
    )
}
async fn allowed(s: &AppState, a: &Auth, v: &Event) -> bool {
    let channel = match v {
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
        Event::NotificationPreferencesUpdated { user_id, .. } => return user_id == &a.user.id,
        Event::MessageCreated(m) | Event::MessageEdited(m) => Some(&m.channel_id),
        Event::MessageDeleted { channel_id, .. }
        | Event::Typing { channel_id, .. }
        | Event::ReactionsUpdated { channel_id, .. } => Some(channel_id),
        Event::Presence { .. } | Event::Resync { .. } => None,
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
            reason: "connected; refetch channel state and presence".into(),
        },
    )
    .await
    {
        return;
    }
    let _connected = Connected::new(s.clone(), a.user.id.clone());
    let mut timer = tokio::time::interval(Duration::from_secs(5));
    let mut last_seen = Instant::now();
    let mut last_typing = HashMap::<String, Instant>::new();
    loop {
        tokio::select! {
            _=timer.tick()=>{
                if !a.valid(&s).await || last_seen.elapsed()>Duration::from_secs(45) {break;}
                if !matches!(tokio::time::timeout(Duration::from_secs(5),socket.send(Frame::Ping(Vec::new().into()))).await,Ok(Ok(()))) {break;}
            },
            incoming=socket.recv()=>{
                match incoming {
                    Some(Ok(Frame::Pong(_)|Frame::Ping(_)))=>last_seen=Instant::now(),
                    Some(Ok(Frame::Text(text)))=>{
                        if !a.valid(&s).await {break;}
                        let Ok(ClientEvent::Typing{channel_id})=serde_json::from_str(&text) else {break;};
                        if visible(&s,&a.user.id,&channel_id).await.is_err(){break;}
                        last_typing.retain(|_,at|at.elapsed()<Duration::from_secs(2));
                        if last_typing.len()<20 && !last_typing.contains_key(&channel_id) {
                            last_typing.insert(channel_id.clone(),Instant::now());
                            let _=s.events.send(Event::Typing{channel_id,user_id:a.user.id.clone()});
                        }
                    },Some(Ok(Frame::Close(_)))|None|Some(Err(_))=>break,
                    Some(Ok(_))=>break,
                }
            },
            incoming=rx.recv()=>{
                if !a.valid(&s).await {break;}
                match incoming {
                    Ok(v)=>{if allowed(&s,&a,&v).await && !event(&mut socket,&v).await {break;}},
                    Err(broadcast::error::RecvError::Lagged(_))=>{let _=event(&mut socket,&Event::Resync{reason:"slow consumer; reconnect and refetch".into()}).await;break;},
                    Err(_)=>break,
                }
            }
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.send(Frame::Close(None))).await;
}
