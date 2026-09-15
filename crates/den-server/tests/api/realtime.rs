use super::*;

pub(super) type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
pub(super) async fn socket(t: &Test, token: &str) -> Socket {
    let mut request = format!("{}/ws", t.url.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    connect_async(request).await.unwrap().0
}
async fn event(socket: &mut Socket) -> Event {
    loop {
        match socket.next().await.unwrap().unwrap() {
            Frame::Text(v) => {
                let e: Event = serde_json::from_str(&v).unwrap();
                if matches!(
                    e,
                    Event::MessageCreated(_)
                        | Event::MessageEdited(_)
                        | Event::MessageDeleted { .. }
                        | Event::Resync { .. }
                ) {
                    return e;
                }
            }
            Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
}

#[tokio::test]
async fn receive_only_unknown_events_do_not_close_or_escape_the_stream() {
    let t = Test::new().await;
    let mut peer = socket(&t, &t.admin.token).await;
    assert!(matches!(event(&mut peer).await, Event::Resync { .. }));
    t.state.events.send(Event::Unknown).unwrap();
    let after = Event::Presence {
        user_id: t.admin.user.id.clone(),
        online: false,
    };
    t.state.events.send(after.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match peer.next().await.expect("stream stays open").unwrap() {
                Frame::Text(text) => {
                    let received: Event = serde_json::from_str(&text).unwrap();
                    assert_ne!(received, Event::Unknown, "fallback must not be broadcast");
                    if received == after {
                        break;
                    }
                }
                Frame::Ping(bytes) => peer.send(Frame::Pong(bytes)).await.unwrap(),
                Frame::Close(_) => panic!("fallback closed the stream"),
                _ => {}
            }
        }
    })
    .await
    .expect("known event must arrive after the ignored fallback");
}

#[tokio::test]
async fn websocket_filters_dms_marks_reconnect_and_closes_revoked_bot_tokens() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bot: BotCreated = serde_json::from_value(
        t.post(
            "/bots",
            &alice.token,
            json!({"username":"clanker","display_name":"Clanker"}),
        )
        .await,
    )
    .unwrap();
    let mut peer = socket(&t, &bot.credential.token).await;
    let mut outsider = socket(&t, &t.admin.token).await;
    assert!(matches!(event(&mut peer).await, Event::Resync { .. }));
    assert!(matches!(event(&mut outsider).await, Event::Resync { .. }));
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bot.user.id]}))
        .await;
    let sent = t
        .post(
            &format!("/channels/{}/messages", dm["id"].as_str().unwrap()),
            &bot.credential.token,
            json!({"content":"private bot message"}),
        )
        .await;
    match tokio::time::timeout(Duration::from_secs(2), event(&mut peer))
        .await
        .unwrap()
    {
        Event::MessageCreated(m) => {
            assert_eq!(m.author_id, bot.user.id);
            assert_eq!(m.id, sent["id"]);
        }
        _ => panic!("expected message"),
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(200), event(&mut outsider))
            .await
            .is_err()
    );
    peer.close(None).await.unwrap();
    let mut peer = socket(&t, &bot.credential.token).await;
    assert!(matches!(event(&mut peer).await, Event::Resync { .. }));
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/tokens/{}", bot.credential.credential.id),
            &alice.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    let replacement = t
        .post(
            "/tokens",
            &alice.token,
            json!({"name":"replacement","user_id":bot.user.id}),
        )
        .await;
    assert_eq!(
        t.req(
            Method::GET,
            "/users/me",
            replacement["token"].as_str().unwrap()
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        t.req(Method::POST, "/tokens", &t.admin.token)
            .json(&json!({"name":"steal","user_id":bot.user.id}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let general = t.general().await;
    t.post(
        &format!("/channels/{general}/messages"),
        &t.admin.token,
        json!({"content":"trigger revocation check"}),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            match peer.next().await {
                None | Some(Ok(Frame::Close(_))) => break,
                Some(Ok(Frame::Ping(v))) => {
                    let _ = peer.send(Frame::Pong(v)).await;
                }
                Some(Err(_)) => break,
                Some(Ok(Frame::Text(v))) => {
                    if matches!(
                        serde_json::from_str::<Event>(&v).unwrap(),
                        Event::MessageCreated(_)
                    ) {
                        panic!("revoked token received event");
                    }
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let mut request = format!("{}/ws", t.url.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Cookie",
        format!("den_session={}", alice.token).parse().unwrap(),
    );
    request
        .headers_mut()
        .insert("Origin", "https://evil.example".parse().unwrap());
    assert!(connect_async(request).await.is_err());
}
