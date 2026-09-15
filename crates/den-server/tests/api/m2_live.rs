use super::realtime::{socket, Socket};
use super::*;

async fn next(socket: &mut Socket) -> Event {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Frame::Text(v) => return serde_json::from_str(&v).unwrap(),
                Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
                _ => {}
            }
        }
    })
    .await
    .unwrap()
}
async fn until(socket: &mut Socket, predicate: impl Fn(&Event) -> bool) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let v = next(socket).await;
        let done = predicate(&v);
        events.push(v);
        if done {
            return events;
        }
    }
}
#[tokio::test]
async fn websocket_targets_notifications_and_read_state_without_dm_leaks() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let alice = t.member("alice").await;
    let cid = t.general().await;
    let mut peer = socket(&t, &bob.token).await;
    let mut outside = socket(&t, &t.admin.token).await;
    assert!(matches!(next(&mut peer).await, Event::Resync { .. }));
    assert!(matches!(next(&mut outside).await, Event::Resync { .. }));
    let path = format!("/channels/{cid}/messages");
    t.post(&path, &alice.token, json!({"content":"ordinary"}))
        .await;
    let events = until(&mut peer, |e| matches!(e, Event::ReadStateUpdated { .. })).await;
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::Notification { .. })));
    t.post(&path, &alice.token, json!({"content":"hello @bob"}))
        .await;
    let events = until(&mut peer, |e| matches!(e, Event::ReadStateUpdated { .. })).await;
    assert_eq!(events.iter().filter(|e|matches!(e,Event::Notification{user_id,reason:NotificationReason::Mention,..} if user_id==&bob.user.id)).count(),1);
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let private = dm["id"].as_str().unwrap();
    let msg = t
        .post(
            &format!("/channels/{private}/messages"),
            &alice.token,
            json!({"content":"private"}),
        )
        .await;
    let events = until(
        &mut peer,
        |e| matches!(e,Event::ReadStateUpdated{state,..} if state.channel_id==private),
    )
    .await;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Notification {
            reason: NotificationReason::Dm,
            ..
        }
    )));
    let tail = t
        .post(&path, &alice.token, json!({"content":"public barrier"}))
        .await;
    let events = until(
        &mut outside,
        |e| matches!(e,Event::MessageCreated(m) if m.id==tail["id"]),
    )
    .await;
    for e in events {
        match e {
            Event::Notification { user_id, .. } | Event::ReadStateUpdated { user_id, .. } => {
                assert_eq!(user_id, t.admin.user.id)
            }
            Event::MessageCreated(m) => assert_ne!(m.channel_id, private),
            _ => {}
        }
    }
    let r = t
        .req(
            Method::PUT,
            &format!("/channels/{private}/read"),
            &bob.token,
        )
        .json(&json!({"message_id":msg["id"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let events=until(&mut peer,|e|matches!(e,Event::ReadStateUpdated{state,..} if state.channel_id==private && state.unread_count==0)).await;
    assert!(events
        .iter()
        .all(|e| !matches!(e,Event::ReadStateUpdated{user_id,..} if user_id!=&bob.user.id)));
}
#[tokio::test]
async fn presence_counts_tabs_and_typing_uses_authenticated_identity_and_membership() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let mut first = socket(&t, &alice.token).await;
    let mut second = socket(&t, &alice.token).await;
    let mut peer = socket(&t, &bob.token).await;
    for s in [&mut first, &mut second, &mut peer] {
        assert!(matches!(next(s).await, Event::Resync { .. }));
    }
    first.close(None).await.unwrap();
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let cid = dm["id"].as_str().unwrap();
    second
        .send(Frame::Text(
            serde_json::to_string(&ClientEvent::Typing {
                channel_id: cid.into(),
                thread_id: None,
            })
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    let events = until(&mut peer, |e| matches!(e, Event::Typing { .. })).await;
    assert!(events.iter().any(|e|matches!(e,Event::Typing{user_id,channel_id,..} if user_id==&alice.user.id && channel_id==cid)));
    assert!(!events
        .iter()
        .any(|e| matches!(e,Event::Presence{user_id,online:false} if user_id==&alice.user.id)));
    let online: PresenceState = t
        .req(Method::GET, "/presence", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(online.online_user_ids.contains(&alice.user.id));
    second.close(None).await.unwrap();
    until(
        &mut peer,
        |e| matches!(e,Event::Presence{user_id,online:false} if user_id==&alice.user.id),
    )
    .await;
    let mut outside = socket(&t, &t.admin.token).await;
    assert!(matches!(next(&mut outside).await, Event::Resync { .. }));
    outside
        .send(Frame::Text(
            json!({"type":"typing","channel_id":cid}).to_string().into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match outside.next().await {
                None | Some(Err(_)) | Some(Ok(Frame::Close(_))) => break,
                Some(Ok(Frame::Ping(v))) => {
                    let _ = outside.send(Frame::Pong(v)).await;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let mut spoof = socket(&t, &alice.token).await;
    assert!(matches!(next(&mut spoof).await, Event::Resync { .. }));
    spoof
        .send(Frame::Text(
            json!({"type":"typing","channel_id":cid,"user_id":bob.user.id})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match spoof.next().await {
                None | Some(Err(_)) | Some(Ok(Frame::Close(_))) => break,
                Some(Ok(Frame::Ping(v))) => {
                    let _ = spoof.send(Frame::Pong(v)).await;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
}
