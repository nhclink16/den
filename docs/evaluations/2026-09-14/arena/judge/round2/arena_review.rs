use super::*;

#[tokio::test]
async fn cookie_parser_preserves_trim_and_first_matching_cookie() {
    let t = Test::new().await;
    let cookie = format!("noise=x;  den_session={}; den_session=invalid; tail=y", t.admin.token);
    let response = t.http.get(format!("{}/users/me", t.url))
        .header("Cookie", cookie).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let user: User = response.json().await.unwrap();
    assert_eq!(user.id, t.admin.user.id);
    let response = t.http.get(format!("{}/users/me", t.url))
        .header("Cookie", format!("den_session=invalid; den_session={}", t.admin.token))
        .send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn presence_order_remains_sorted_and_unique_across_tabs() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let mut sockets = Vec::new();
    for token in [&bob.token, &alice.token, &t.admin.token, &alice.token] {
        let mut socket = super::realtime::socket(&t, token).await;
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match socket.next().await.unwrap().unwrap() {
                    Frame::Text(v) => if matches!(serde_json::from_str::<Event>(&v).unwrap(), Event::Resync { .. }) { break; },
                    Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
                    _ => {}
                }
            }
        }).await.unwrap();
        sockets.push(socket);
    }
    let presence: PresenceState = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let p: PresenceState = t.req(Method::GET, "/presence", &t.admin.token)
                .send().await.unwrap().json().await.unwrap();
            if p.online_user_ids.len() == 3 { break p; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let mut expected = vec![t.admin.user.id.clone(), alice.user.id.clone(), bob.user.id.clone()];
    expected.sort();
    assert_eq!(presence.online_user_ids, expected);
    for mut socket in sockets { let _ = socket.close(None).await; }
}

#[tokio::test]
async fn read_state_events_are_owner_only_in_shared_channel() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let general = t.general().await;
    let mut sockets = Vec::new();
    for session in [&bob, &t.admin] {
        let mut socket = super::realtime::socket(&t, &session.token).await;
        assert!(matches!(next_event(&mut socket).await, Event::Resync { .. }));
        sockets.push((session.user.id.clone(), socket));
    }
    let path = format!("/channels/{general}/messages");
    t.post(&path, &alice.token, json!({"content":"ordinary public message"})).await;
    let barrier = t.post(&path, &alice.token, json!({"content":"ordered public barrier"})).await;
    for (owner, mut socket) in sockets {
        let mut read_states = 0;
        loop {
            match next_event(&mut socket).await {
                Event::ReadStateUpdated { user_id, state } => {
                    assert_eq!(user_id, owner, "socket received another member's read state in a shared channel");
                    assert_eq!(state.channel_id, general);
                    read_states += 1;
                }
                Event::MessageCreated(m) if m.id == barrier["id"] => break,
                _ => {}
            }
        }
        assert!(read_states > 0, "recipient's own read state must still be delivered");
        let _ = socket.close(None).await;
    }
}

async fn next_event(socket: &mut super::realtime::Socket) -> Event {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Frame::Text(v) => return serde_json::from_str(&v).unwrap(),
                Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
                _ => {}
            }
        }
    }).await.unwrap()
}
