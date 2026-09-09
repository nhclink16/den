use crate::{auth::Auth, chat::visible, *};
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};

#[utoipa::path(get,path="/ws",responses((status=101,description="Event stream; first event requires resync"),(status=401,body=ApiError)))]
pub(crate) async fn connect(State(s):State<AppState>,a:Auth,ws:WebSocketUpgrade)->Response {
    let rx=s.events.subscribe();
    ws.max_message_size(4096).max_frame_size(4096).on_upgrade(move |socket|run(s,a,socket,rx))
}
async fn event(socket:&mut WebSocket,event:&Event)->bool {
    let Ok(json)=serde_json::to_string(event) else {return false;};
    matches!(tokio::time::timeout(Duration::from_secs(5),socket.send(Frame::Text(json.into()))).await,Ok(Ok(())))
}
async fn run(s:AppState,a:Auth,mut socket:WebSocket,mut rx:broadcast::Receiver<Event>) {
    if !event(&mut socket,&Event::Resync{reason:"connected; refetch channel state".into()}).await {return;}
    let mut timer=tokio::time::interval(Duration::from_secs(5));let mut last_seen=Instant::now();
    loop {
        tokio::select! {
            _=timer.tick()=>{
                if !a.valid(&s).await || last_seen.elapsed()>Duration::from_secs(45) {break;}
                if !matches!(tokio::time::timeout(Duration::from_secs(5),socket.send(Frame::Ping(Vec::new().into()))).await,Ok(Ok(()))) {break;}
            },
            incoming=socket.recv()=>{
                match incoming {Some(Ok(Frame::Pong(_)|Frame::Ping(_)))=>last_seen=Instant::now(),Some(Ok(Frame::Close(_)))|None|Some(Err(_))=>break,
                    // Client mutations/typing are reserved for M2. Do not rebroadcast client JSON.
                    Some(Ok(_))=>break}
            },
            incoming=rx.recv()=>{
                if !a.valid(&s).await {break;}
                match incoming {
                    Ok(v)=>{
                        let channel=match &v {Event::MessageCreated(m)|Event::MessageEdited(m)=>Some(&m.channel_id),Event::MessageDeleted{channel_id,..}|Event::Typing{channel_id,..}=>Some(channel_id),_=>None};
                        if let Some(id)=channel {if visible(&s,&a.user.id,id).await.is_err(){continue;}}
                        if !event(&mut socket,&v).await {break;}
                    },
                    Err(broadcast::error::RecvError::Lagged(_))=>{let _=event(&mut socket,&Event::Resync{reason:"slow consumer; reconnect and refetch".into()}).await;break;},
                    Err(_)=>break,
                }
            }
        }
    }
    let _=tokio::time::timeout(Duration::from_secs(1),socket.send(Frame::Close(None))).await;
}
