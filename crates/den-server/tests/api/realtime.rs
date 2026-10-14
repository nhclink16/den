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

async fn channel_update(socket: &mut Socket) -> Channel {
    loop {
        match socket.next().await.unwrap().unwrap() {
            Frame::Text(v) => {
                if let Event::ChannelUpdated { channel } = serde_json::from_str(&v).unwrap() {
                    return channel;
                }
            }
            Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
}
async fn soon(socket: &mut Socket) -> Channel {
    tokio::time::timeout(Duration::from_secs(2), channel_update(socket))
        .await
        .expect("a room change should arrive")
}
async fn quiet(socket: &mut Socket) -> bool {
    tokio::time::timeout(Duration::from_millis(300), channel_update(socket))
        .await
        .is_err()
}

#[tokio::test]
async fn room_changes_reach_members_live_but_never_a_dm_they_cannot_see() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let general = t.general().await;
    let mut alice_ws = socket(&t, &alice.token).await;
    let mut bob_ws = socket(&t, &bob.token).await;
    assert!(matches!(event(&mut alice_ws).await, Event::Resync { .. }));
    assert!(matches!(event(&mut bob_ws).await, Event::Resync { .. }));

    // An admin changes the icon: every member hears about it without a reload.
    let r = t
        .req(
            Method::PUT,
            &format!("/channels/{general}/icon"),
            &t.admin.token,
        )
        .json(&json!({"icon":"flame"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let seen = soon(&mut alice_ws).await;
    assert_eq!(seen.id, general);
    assert_eq!(seen.icon.as_deref(), Some("flame"));
    assert_eq!(soon(&mut bob_ws).await.icon.as_deref(), Some("flame"));

    // A rename carries the new name and keeps the icon.
    let before: Channel = t
        .req(Method::GET, &format!("/channels/{general}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r = t
        .req(Method::PUT, &format!("/channels/{general}"), &t.admin.token)
        .json(&json!({"name":"lobby","category_id":before.category_id,"position":before.position}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let seen = soon(&mut alice_ws).await;
    assert_eq!(
        (seen.name.as_str(), seen.icon.as_deref()),
        ("lobby", Some("flame"))
    );
    assert_eq!(soon(&mut bob_ws).await.name, "lobby");

    // A refused change announces nothing.
    let r = t
        .req(
            Method::PUT,
            &format!("/channels/{general}/icon"),
            &alice.token,
        )
        .json(&json!({"icon":"skull"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    assert!(quiet(&mut bob_ws).await);

    // No route changes a DM, so send one directly: the socket filter alone must
    // keep it from anyone outside the DM.
    let dm: Channel = serde_json::from_value(
        t.post(
            "/dms",
            &t.admin.token,
            json!({"member_ids":[alice.user.id]}),
        )
        .await,
    )
    .unwrap();
    t.state
        .events
        .send(Event::ChannelUpdated {
            channel: dm.clone(),
        })
        .unwrap();
    assert_eq!(soon(&mut alice_ws).await.id, dm.id);
    assert!(quiet(&mut bob_ws).await);
}
