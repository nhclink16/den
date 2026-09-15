use super::realtime::Socket;
use super::*;

async fn socket(t: &Test, token: &str, query: &str) -> Socket {
    let mut request = format!("{}/ws{query}", t.url.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    let frame = socket.next().await.unwrap().unwrap();
    assert!(
        matches!(frame, Frame::Text(text) if matches!(serde_json::from_str::<Event>(&text).unwrap(), Event::Resync { .. }))
    );
    socket
}

// A later chat event is a barrier: all preceding sound mutations have been
// processed on this same socket. No timeout-based absence assertion is needed.
async fn updates_until_message(socket: &mut Socket, message_id: &str) -> Vec<Option<String>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut updates = Vec::new();
        loop {
            match socket.next().await.expect("stream stays open").unwrap() {
                Frame::Text(text) => match serde_json::from_str::<Event>(&text).unwrap() {
                    Event::SoundsUpdated { user_id } => updates.push(user_id),
                    Event::MessageCreated(message) if message.id == message_id => return updates,
                    _ => {}
                },
                Frame::Ping(bytes) => socket.send(Frame::Pong(bytes)).await.unwrap(),
                Frame::Close(_) => panic!("stream closed before chat barrier"),
                _ => {}
            }
        }
    })
    .await
    .expect("socket still receives chat after sound mutations")
}

#[tokio::test]
async fn sounds_require_socket_opt_in_and_keep_legacy_streams_usable() {
    let t = Test::new().await;
    let alice = t.member("sounds_ws_alice").await;
    let bob = t.member("sounds_ws_bob").await;
    let channel = t.general().await;
    let mut legacy = socket(&t, &alice.token, "").await;
    let mut disabled = socket(&t, &alice.token, "?sounds=false").await;
    let mut music_only = socket(&t, &alice.token, "?music=true").await;
    let mut owner = socket(&t, &alice.token, "?sounds=true&music=true").await;
    let mut other = socket(&t, &bob.token, "?sounds=true").await;
    // The web store's native path must preserve both the opt-in and its ticket.
    let ticket: WsTicket =
        serde_json::from_value(t.post("/auth/ws-ticket", &alice.token, json!({})).await).unwrap();
    let (mut native, _) = connect_async(format!(
        "{}/ws?sounds=true&music=true&ticket={}",
        t.url.replace("http:", "ws:"),
        ticket.ticket,
    ))
    .await
    .unwrap();

    for (token, path, body, owner_updates, other_updates) in [
        (
            &alice.token,
            "/users/me/sounds",
            json!({"master_volume":35}),
            vec![Some(alice.user.id.clone())],
            vec![],
        ),
        (
            &bob.token,
            "/users/me/sounds",
            json!({"overrides":{"dm":{"type":"silent"}}}),
            vec![],
            vec![Some(bob.user.id.clone())],
        ),
        (
            &t.admin.token,
            "/settings/sounds",
            json!({"id":"bells","name":"Server bells","sounds":{}}),
            vec![None],
            vec![None],
        ),
    ] {
        t.req(Method::PUT, path, token)
            .json(&body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let message = t
            .post(
                &format!("/channels/{channel}/messages"),
                &alice.token,
                json!({"content":"Legacy stream still receives chat"}),
            )
            .await;
        let id = message["id"].as_str().unwrap();
        assert!(
            updates_until_message(&mut legacy, id).await.is_empty(),
            "Legacy clients must never receive sounds_updated"
        );
        assert!(updates_until_message(&mut disabled, id).await.is_empty());
        assert!(updates_until_message(&mut music_only, id).await.is_empty());
        assert_eq!(updates_until_message(&mut owner, id).await, owner_updates);
        assert_eq!(updates_until_message(&mut native, id).await, owner_updates);
        assert_eq!(updates_until_message(&mut other, id).await, other_updates);
    }
}
