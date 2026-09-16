use super::realtime::Socket;
use super::*;

async fn voice_room(t: &Test) -> String {
    let channels: Vec<Channel> = t
        .req(Method::GET, "/channels", &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    channels
        .into_iter()
        .find(|c| c.kind == ChannelKind::Voice)
        .unwrap()
        .id
}
async fn account(t: &Test, token: &str) -> SpotifyAccount {
    t.req(Method::GET, "/users/me/spotify", token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn jam(t: &Test, room: &str, token: &str) -> Option<Jam> {
    t.req(Method::GET, &format!("/rooms/{room}/jam"), token)
        .send()
        .await
        .unwrap()
        .json::<RoomJam>()
        .await
        .unwrap()
        .jam
}
/// A connected account without an OAuth round trip. The ciphertext is opaque to
/// every read path under test; only a refresh would try to open it.
async fn connect(t: &Test, user: &str, expires_at: i64, needs_reauth: i64) {
    sqlx::query("INSERT INTO spotify_accounts(user_id,refresh_nonce,refresh_ciphertext,key_version,account_name,scopes,connected_at,expires_at,needs_reauth) VALUES(?,?,?,1,?,?,?,?,?)")
        .bind(user)
        .bind(vec![0u8; 12])
        .bind(vec![9u8; 32])
        .bind("Nicholas")
        .bind("user-read-playback-state user-read-currently-playing")
        .bind(1_700_000_000i64)
        .bind(expires_at)
        .bind(needs_reauth)
        .execute(&t.state.db)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_server_without_a_client_secret_disables_spotify_instead_of_failing() {
    let t = Test::new().await;
    let member = t.member("spotify_nosecret").await;
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Unavailable
    );
    for (method, path) in [
        (Method::POST, "/users/me/spotify/authorize"),
        (Method::POST, "/users/me/spotify/callback"),
    ] {
        let r = t
            .req(method, path, &member.token)
            .json(&json!({"code":"c","state":"s"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        let body: Value = r.json().await.unwrap();
        assert_eq!(body["error"], "spotify_unavailable");
    }
    // Disconnecting something that was never connected is still a clean no-op.
    assert_eq!(
        t.req(Method::DELETE, "/users/me/spotify", &member.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        t.req(Method::GET, "/users/me/spotify", "")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_expired_or_rejected_refresh_token_reads_as_reauthorize() {
    let t = Test::with_spotify().await;
    let member = t.member("spotify_expiry").await;
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Disconnected
    );
    let fresh = 4_000_000_000i64;
    connect(&t, &member.user.id, fresh, 0).await;
    let view = account(&t, &member.token).await;
    assert_eq!(view.connection, SpotifyConnection::Connected);
    assert_eq!(view.account_name.as_deref(), Some("Nicholas"));
    assert_eq!(view.expires_at, Some(fresh));

    // The 180-day cliff, and an explicit rejection, are the same normal state.
    for (expires_at, needs_reauth) in [(1_700_000_001i64, 0i64), (fresh, 1)] {
        sqlx::query("UPDATE spotify_accounts SET expires_at=?,needs_reauth=? WHERE user_id=?")
            .bind(expires_at)
            .bind(needs_reauth)
            .bind(&member.user.id)
            .execute(&t.state.db)
            .await
            .unwrap();
        let view = account(&t, &member.token).await;
        assert_eq!(view.connection, SpotifyConnection::Reauthorize);
        // Still an account, so Settings can offer Reconnect rather than Connect.
        assert_eq!(view.account_name.as_deref(), Some("Nicholas"));
    }
}

#[tokio::test]
async fn disconnecting_deletes_the_stored_refresh_token() {
    let t = Test::with_spotify().await;
    let member = t.member("spotify_disconnect").await;
    let other = t.member("spotify_keeper").await;
    connect(&t, &member.user.id, 4_000_000_000, 0).await;
    connect(&t, &other.user.id, 4_000_000_000, 0).await;
    assert_eq!(
        t.req(Method::DELETE, "/users/me/spotify", &member.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let rows: Vec<String> = sqlx::query_scalar("SELECT user_id FROM spotify_accounts")
        .fetch_all(&t.state.db)
        .await
        .unwrap();
    assert_eq!(rows, vec![other.user.id.clone()]);
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Disconnected
    );
    assert_eq!(
        account(&t, &other.token).await.connection,
        SpotifyConnection::Connected
    );
}

#[tokio::test]
async fn an_authorize_state_is_single_use_and_bound_to_the_user_who_started_it() {
    let t = Test::with_spotify().await;
    let member = t.member("spotify_state").await;
    let attacker = t.member("spotify_attacker").await;
    let authorization: SpotifyAuthorization = serde_json::from_value(
        t.post("/users/me/spotify/authorize", &member.token, json!({}))
            .await,
    )
    .unwrap();
    let url = reqwest::Url::parse(&authorization.url).unwrap();
    assert_eq!(url.host_str(), Some("accounts.spotify.com"));
    let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(
        params["client_id"], "9efa4ca0d79a4be5a934a22c229ad656",
        "the public client id travels in the authorize URL"
    );
    assert_eq!(params["response_type"], "code");
    // Read only. No playback control, no writes.
    assert_eq!(
        params["scope"],
        "user-read-playback-state user-read-currently-playing"
    );
    // Derived from DEN_ORIGIN, which the tests bind to a loopback IP. Spotify
    // refuses `localhost`, and a mismatch breaks the callback or the cookie.
    assert_eq!(
        params["redirect_uri"],
        format!("{}/spotify/callback", t.url)
    );
    assert!(!authorization.url.contains("test-client-secret"));
    let state = params["state"].clone();
    assert!(state.len() >= 32);

    // Another user cannot redeem it, and redeeming consumes it either way.
    let stolen = t
        .req(Method::POST, "/users/me/spotify/callback", &attacker.token)
        .json(&json!({"code":"authorization-code","state":state}))
        .send()
        .await
        .unwrap();
    assert_eq!(stolen.status(), StatusCode::BAD_REQUEST);
    let replayed = t
        .req(Method::POST, "/users/me/spotify/callback", &member.token)
        .json(&json!({"code":"authorization-code","state":state}))
        .send()
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Disconnected
    );
}

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
// A later chat event is a barrier: everything the server sent before it on this
// same socket has arrived. No timeout-based absence assertion is needed.
async fn jams_until_message(socket: &mut Socket, message_id: &str) -> Vec<Option<Jam>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut updates = Vec::new();
        loop {
            match socket.next().await.expect("stream stays open").unwrap() {
                Frame::Text(text) => match serde_json::from_str::<Event>(&text).unwrap() {
                    Event::JamUpdated { jam, .. } => updates.push(jam),
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
    .expect("socket still receives chat after jam mutations")
}

#[tokio::test]
async fn a_jam_is_started_joined_and_ended_by_its_host_and_gated_on_the_socket() {
    let t = Test::new().await;
    let host = t.member("jam_host").await;
    let guest = t.member("jam_guest").await;
    let room = voice_room(&t).await;
    let text = t.general().await;
    let path = format!("/rooms/{room}/jam");
    let mut legacy = socket(&t, &guest.token, "").await;
    let mut opted_in = socket(&t, &guest.token, "?jam=true").await;

    assert!(jam(&t, &room, &host.token).await.is_none());
    assert_eq!(
        t.req(Method::GET, &path, "").send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let rejected = t
        .req(Method::POST, &path, &host.token)
        .json(&json!({"url":"https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT"}))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

    let started: Jam = serde_json::from_value(
        t.post(
            &path,
            &host.token,
            json!({"url":"https://open.spotify.com/jam/abc123?si=share&utm_source=copy"}),
        )
        .await,
    )
    .unwrap();
    assert_eq!(started.host_id, host.user.id);
    // The share token survives; the tracking parameter does not.
    assert_eq!(started.url, "https://open.spotify.com/jam/abc123?si=share");
    assert_eq!(started.joined_user_ids, vec![host.user.id.clone()]);
    // Nobody connected Spotify, so the card has no track and claims none.
    assert!(started.now_playing.is_none());

    let joined: Jam = serde_json::from_value(
        t.post(&format!("{path}/join"), &guest.token, json!({}))
            .await,
    )
    .unwrap();
    assert_eq!(
        joined.joined_user_ids,
        vec![host.user.id.clone(), guest.user.id.clone()]
    );
    // Joining twice is one join. Spotify never tells us who is listening; this
    // count is Den's Join clicks and must not double.
    let again: Jam = serde_json::from_value(
        t.post(&format!("{path}/join"), &guest.token, json!({}))
            .await,
    )
    .unwrap();
    assert_eq!(again.joined_user_ids, joined.joined_user_ids);

    // Starting a second Jam retires the first; the unique index allows one live.
    let replacement: Jam = serde_json::from_value(
        t.post(
            &path,
            &guest.token,
            json!({"url":"https://spotify.link/xyz789"}),
        )
        .await,
    )
    .unwrap();
    assert_ne!(replacement.id, started.id);
    assert_eq!(
        jam(&t, &room, &host.token).await.map(|j| j.id),
        Some(replacement.id.clone())
    );

    assert_eq!(
        t.req(Method::DELETE, &path, &host.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN,
        "only the Jam's host or an admin ends it"
    );
    assert_eq!(
        t.req(Method::DELETE, &path, &guest.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(jam(&t, &room, &host.token).await.is_none());

    let message = t
        .post(
            &format!("/channels/{text}/messages"),
            &host.token,
            json!({"content":"Legacy stream still receives chat"}),
        )
        .await;
    let id = message["id"].as_str().unwrap();
    assert!(
        jams_until_message(&mut legacy, id).await.is_empty(),
        "clients that did not opt in must never receive jam_updated"
    );
    let updates = jams_until_message(&mut opted_in, id).await;
    assert_eq!(
        updates.len(),
        4,
        "start, join, replacement start, end; the repeat join changed nothing"
    );
    assert_eq!(updates[0].as_ref().map(|j| j.id.clone()), Some(started.id));
    assert!(updates.last().unwrap().is_none(), "the end event clears it");
}
