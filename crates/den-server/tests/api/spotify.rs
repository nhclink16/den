use super::realtime::Socket;
use super::*;
use axum::{
    extract::{Form, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Copy)]
enum ProviderMode {
    Healthy,
    HeldRefresh,
    MissingScope,
    Revoked,
    Limited,
    Slow,
}

#[derive(Clone)]
struct ProviderState {
    mode: ProviderMode,
    token_calls: Arc<AtomicUsize>,
    refresh_release: Arc<tokio::sync::Notify>,
    playback_calls: Arc<AtomicUsize>,
    auth_headers: Arc<tokio::sync::Mutex<Vec<String>>>,
    playback_headers: Arc<tokio::sync::Mutex<Vec<String>>>,
}

struct StubSpotify {
    url: String,
    state: ProviderState,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for StubSpotify {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl StubSpotify {
    async fn start(mode: ProviderMode) -> Self {
        let state = ProviderState {
            mode,
            token_calls: Arc::new(AtomicUsize::new(0)),
            refresh_release: Arc::new(tokio::sync::Notify::new()),
            playback_calls: Arc::new(AtomicUsize::new(0)),
            auth_headers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            playback_headers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .route("/api/token", post(stub_token))
            .route("/v1/me", get(stub_profile))
            .route("/v1/me/player/currently-playing", get(stub_playing))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, state, task }
    }
}

async fn stub_token(
    State(state): State<ProviderState>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    state.token_calls.fetch_add(1, Ordering::SeqCst);
    state.auth_headers.lock().await.push(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
    );
    if matches!(state.mode, ProviderMode::Slow) {
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    if form.get("grant_type").map(String::as_str) == Some("refresh_token") {
        if matches!(state.mode, ProviderMode::HeldRefresh) {
            state.refresh_release.notified().await;
        }
        if matches!(state.mode, ProviderMode::Revoked) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_grant"})),
            )
                .into_response();
        }
        return (
            StatusCode::OK,
            Json(json!({
                "access_token":"stub-access-rotated",
                "refresh_token":"stub-refresh-rotated",
                "expires_in":3600,
                "scope":"user-read-currently-playing"
            })),
        )
            .into_response();
    }
    let scope = if matches!(state.mode, ProviderMode::MissingScope) {
        "user-read-playback-state"
    } else {
        "user-read-currently-playing"
    };
    let expires = if matches!(state.mode, ProviderMode::Limited) {
        3600
    } else {
        60
    };
    (
        StatusCode::OK,
        Json(json!({
            "access_token":"stub-access-initial",
            "refresh_token":"stub-refresh-initial",
            "expires_in":expires,
            "scope":scope
        })),
    )
        .into_response()
}

async fn stub_profile(State(_): State<ProviderState>, headers: HeaderMap) -> impl IntoResponse {
    let authorized = headers.get("authorization").and_then(|v| v.to_str().ok())
        == Some("Bearer stub-access-initial");
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"display_name":"Stub Listener","id":"stub-listener"})).into_response()
}

async fn stub_playing(State(state): State<ProviderState>, headers: HeaderMap) -> impl IntoResponse {
    state.playback_calls.fetch_add(1, Ordering::SeqCst);
    state.playback_headers.lock().await.push(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
    );
    if matches!(state.mode, ProviderMode::Limited) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(reqwest::header::RETRY_AFTER, "30")],
        )
            .into_response();
    }
    Json(json!({
        "is_playing":true,
        "progress_ms":42_000,
        "item":{
            "name":"Stub Song",
            "duration_ms":180_000,
            "artists":[{"name":"Stub Artist"}],
            "album":{"images":[
                {"url":"https://i.scdn.co/image/large","width":640},
                {"url":"https://i.scdn.co/image/card","width":300}
            ]}
        }
    }))
    .into_response()
}

async fn authorization(t: &Test, token: &str) -> (SpotifyAuthorization, String) {
    let authorization: SpotifyAuthorization = serde_json::from_value(
        t.post("/users/me/spotify/authorize", token, json!({}))
            .await,
    )
    .unwrap();
    let url = reqwest::Url::parse(&authorization.url).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    (authorization, state)
}

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
async fn disconnect_wins_over_a_refresh_already_in_flight() {
    let provider = StubSpotify::start(ProviderMode::Slow).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_disconnect_race").await;
    let (_, state) = authorization(&t, &member.token).await;
    t.post(
        "/users/me/spotify/callback",
        &member.token,
        json!({"code":"stub-code","state":state}),
    )
    .await;
    let room = voice_room(&t).await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &member.token,
        json!({"url":"https://spotify.link/disconnect-race"}),
    )
    .await;

    // The stub's one-minute token expires at the refresh margin. Hold its slow
    // refresh open, then disconnect while that provider request is in flight.
    let client = t.http.clone();
    let url = format!("{}/rooms/{room}/jam", t.url);
    let token = member.token.clone();
    let pending = tokio::spawn(async move {
        client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .json::<RoomJam>()
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while provider.state.token_calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("refresh reached the provider");

    assert_eq!(
        t.req(Method::DELETE, "/users/me/spotify", &member.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        pending.await.unwrap().jam.unwrap().now_playing.is_none(),
        "the completed refresh resurrected playback after disconnect"
    );
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Disconnected
    );
}

#[tokio::test]
async fn disconnect_cancels_an_oauth_callback_already_in_flight() {
    let provider = StubSpotify::start(ProviderMode::Slow).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_callback_race").await;
    let (_, state) = authorization(&t, &member.token).await;

    let client = t.http.clone();
    let url = format!("{}/users/me/spotify/callback", t.url);
    let token = member.token.clone();
    let pending = tokio::spawn(async move {
        client
            .post(url)
            .bearer_auth(token)
            .json(&json!({"code":"stub-code","state":state}))
            .send()
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while provider.state.token_calls.load(Ordering::SeqCst) < 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("callback reached the provider");

    assert_eq!(
        t.req(Method::DELETE, "/users/me/spotify", &member.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        pending.await.unwrap().status(),
        StatusCode::BAD_REQUEST,
        "the callback reconnected Spotify after Disconnect completed"
    );
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Disconnected
    );

    // A callback that has not reached the provider is cancelled too, without
    // spending the code after the user explicitly disconnected.
    let (_, state) = authorization(&t, &member.token).await;
    assert_eq!(
        t.req(Method::DELETE, "/users/me/spotify", &member.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let calls = provider.state.token_calls.load(Ordering::SeqCst);
    let cancelled = t
        .req(Method::POST, "/users/me/spotify/callback", &member.token)
        .json(&json!({"code":"unused-code","state":state}))
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::BAD_REQUEST);
    assert_eq!(provider.state.token_calls.load(Ordering::SeqCst), calls);
}

#[tokio::test]
async fn a_new_authorization_waits_for_older_account_work() {
    let provider = StubSpotify::start(ProviderMode::Slow).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_authorize_race").await;
    let (_, state) = authorization(&t, &member.token).await;
    t.post(
        "/users/me/spotify/callback",
        &member.token,
        json!({"code":"initial-code","state":state}),
    )
    .await;
    let room = voice_room(&t).await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &member.token,
        json!({"url":"https://spotify.link/authorize-race"}),
    )
    .await;

    // The initial one-minute access token expires at the refresh margin. Hold
    // the provider refresh in flight while a new authorization starts. The new
    // attempt must not return until older account work can no longer land.
    let client = t.http.clone();
    let url = format!("{}/rooms/{room}/jam", t.url);
    let token = member.token.clone();
    let refresh =
        tokio::spawn(async move { client.get(url).bearer_auth(token).send().await.unwrap() });
    tokio::time::timeout(Duration::from_secs(1), async {
        while provider.state.token_calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("older refresh reached the provider");

    let client = t.http.clone();
    let url = format!("{}/users/me/spotify/authorize", t.url);
    let token = member.token.clone();
    let mut newer = tokio::spawn(async move {
        client
            .post(url)
            .bearer_auth(token)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(75), &mut newer)
            .await
            .is_err(),
        "new authorization returned while older account work could still write"
    );
    assert_eq!(refresh.await.unwrap().status(), StatusCode::OK);
    assert_eq!(newer.await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Connected
    );
}

#[tokio::test]
async fn a_rotated_refresh_token_is_not_used_until_it_is_stored() {
    let provider = StubSpotify::start(ProviderMode::HeldRefresh).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(10)).await;
    let member = t.member("spotify_rotation_store").await;
    let (_, state) = authorization(&t, &member.token).await;
    t.post(
        "/users/me/spotify/callback",
        &member.token,
        json!({"code":"initial-code","state":state}),
    )
    .await;
    let room = voice_room(&t).await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &member.token,
        json!({"url":"https://spotify.link/rotation-store"}),
    )
    .await;

    let client = t.http.clone();
    let url = format!("{}/rooms/{room}/jam", t.url);
    let token = member.token.clone();
    let refresh = tokio::spawn(async move {
        client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .json::<RoomJam>()
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while provider.state.token_calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("refresh reached the provider");

    // Spotify may invalidate the old token as soon as it rotates. Make the
    // replacement write fail and prove Den does not cache the accompanying
    // access token while discarding the only usable refresh token.
    let mut blocker = t.state.db.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    provider.state.refresh_release.notify_one();
    let refreshed = refresh.await.unwrap();
    assert!(
        refreshed.jam.unwrap().now_playing.is_none(),
        "Den used a rotated grant whose replacement refresh token was not stored"
    );
    sqlx::query("ROLLBACK")
        .execute(&mut *blocker)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_authorize_state_is_single_use_and_bound_to_the_user_who_started_it() {
    let provider = StubSpotify::start(ProviderMode::Healthy).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_state").await;
    let attacker = t.member("spotify_attacker").await;
    assert_eq!(
        t.http
            .post(format!("{}/users/me/spotify/authorize", t.url))
            .header("Cookie", format!("den_session={}", member.token))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN,
        "cookie-authenticated OAuth writes require origin and CSRF"
    );
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
    // This is the only permission Spotify documents for the sole playback
    // endpoint Den calls. Do not ask for device or broader playback state.
    assert_eq!(params["scope"], "user-read-currently-playing");
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

#[tokio::test]
async fn oauth_exchange_refresh_and_now_playing_use_the_stub_provider() {
    let provider = StubSpotify::start(ProviderMode::Healthy).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_oauth").await;
    let other = t.member("spotify_oauth_other").await;
    let mut legacy = socket(&t, &member.token, "").await;
    let mut opted_in = socket(&t, &member.token, "?jam=true").await;
    let mut other_user = socket(&t, &other.token, "?jam=true").await;
    let (_, state) = authorization(&t, &member.token).await;
    let connected: SpotifyAccount = serde_json::from_value(
        t.post(
            "/users/me/spotify/callback",
            &member.token,
            json!({"code":"stub-code","state":state}),
        )
        .await,
    )
    .unwrap();
    assert_eq!(connected.connection, SpotifyConnection::Connected);
    assert_eq!(connected.account_name.as_deref(), Some("Stub Listener"));
    let text_room = t.general().await;
    let barrier = t
        .post(
            &format!("/channels/{text_room}/messages"),
            &member.token,
            json!({"content":"Spotify account event barrier"}),
        )
        .await;
    let barrier = barrier["id"].as_str().unwrap();
    assert!(spotify_until_message(&mut legacy, barrier).await.is_empty());
    assert_eq!(
        spotify_until_message(&mut opted_in, barrier)
            .await
            .iter()
            .map(|account| account.connection)
            .collect::<Vec<_>>(),
        [SpotifyConnection::Connected]
    );
    assert!(
        spotify_until_message(&mut other_user, barrier)
            .await
            .is_empty(),
        "another user must not receive private Spotify account state"
    );

    let (nonce, ciphertext, scopes, refresh_expires_at): (Vec<u8>, Vec<u8>, String, i64) = sqlx::query_as(
        "SELECT refresh_nonce,refresh_ciphertext,scopes,expires_at FROM spotify_accounts WHERE user_id=?",
    )
    .bind(&member.user.id)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(nonce.len(), 12);
    assert!(
        !ciphertext
            .windows(b"stub-refresh-initial".len())
            .any(|window| window == b"stub-refresh-initial"),
        "the refresh token must not be plaintext in SQLite"
    );
    assert_eq!(scopes, "user-read-currently-playing");

    let room = voice_room(&t).await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &member.token,
        json!({"url":"https://open.spotify.com/jam/oauth-stub"}),
    )
    .await;
    let live = jam(&t, &room, &member.token).await.unwrap();
    let playing = live.now_playing.unwrap();
    assert_eq!(playing.track, "Stub Song");
    assert_eq!(playing.artists, "Stub Artist");
    assert_eq!(
        playing.album_art.as_deref(),
        Some("https://i.scdn.co/image/card")
    );
    assert_eq!(playing.progress_ms, Some(42_000));
    assert!(playing.is_playing);

    assert_eq!(provider.state.token_calls.load(Ordering::SeqCst), 2);
    assert_eq!(provider.state.playback_calls.load(Ordering::SeqCst), 1);
    let expected_basic = format!(
        "Basic {}",
        STANDARD.encode("9efa4ca0d79a4be5a934a22c229ad656:test-client-secret")
    );
    assert!(provider
        .state
        .auth_headers
        .lock()
        .await
        .iter()
        .all(|header| header == &expected_basic));
    assert_eq!(
        provider.state.playback_headers.lock().await.as_slice(),
        ["Bearer stub-access-rotated"]
    );
    let rotated: Vec<u8> =
        sqlx::query_scalar("SELECT refresh_ciphertext FROM spotify_accounts WHERE user_id=?")
            .bind(&member.user.id)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_ne!(
        rotated, ciphertext,
        "a rotated refresh token must replace the old grant"
    );
    assert!(!rotated
        .windows(b"stub-refresh-rotated".len())
        .any(|window| window == b"stub-refresh-rotated"));
    let expires_after_rotation: i64 =
        sqlx::query_scalar("SELECT expires_at FROM spotify_accounts WHERE user_id=?")
            .bind(&member.user.id)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(
        expires_after_rotation, refresh_expires_at,
        "access-token refresh must not extend Spotify's six-month grant"
    );
}

#[tokio::test]
async fn oauth_rejects_missing_scopes() {
    let provider = StubSpotify::start(ProviderMode::MissingScope).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_scope").await;
    let (_, state) = authorization(&t, &member.token).await;
    let rejected = t
        .req(Method::POST, "/users/me/spotify/callback", &member.token)
        .json(&json!({"code":"stub-code","state":state}))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM spotify_accounts")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn oauth_bounds_a_slow_provider_and_keeps_secrets_opaque() {
    let slow = StubSpotify::start(ProviderMode::Slow).await;
    let t = Test::with_spotify_provider(slow.url.clone(), Duration::from_millis(50)).await;
    let member = t.member("spotify_timeout").await;
    let (_, state) = authorization(&t, &member.token).await;
    let started = std::time::Instant::now();
    let response = t
        .req(Method::POST, "/users/me/spotify/callback", &member.token)
        .json(&json!({"code":"never-log-this-code","state":state}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(started.elapsed() < Duration::from_millis(250));
    let body = response.text().await.unwrap();
    assert!(body.contains("spotify_unreachable"));
    assert!(!body.contains("never-log-this-code"));
    assert!(!body.contains("test-client-secret"));
}

#[tokio::test]
async fn a_revoked_account_degrades_without_losing_the_jam() {
    let revoked = StubSpotify::start(ProviderMode::Revoked).await;
    let t = Test::with_spotify_provider(revoked.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_revoked").await;
    let (_, state) = authorization(&t, &member.token).await;
    t.post(
        "/users/me/spotify/callback",
        &member.token,
        json!({"code":"stub-code","state":state}),
    )
    .await;
    let room = voice_room(&t).await;
    let started: Jam = serde_json::from_value(
        t.post(
            &format!("/rooms/{room}/jam"),
            &member.token,
            json!({"url":"https://spotify.link/revoked"}),
        )
        .await,
    )
    .unwrap();
    let live = jam(&t, &room, &member.token).await.unwrap();
    assert_eq!(live.id, started.id);
    assert!(live.now_playing.is_none());
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Reauthorize
    );
}

#[tokio::test]
async fn a_rate_limited_account_honors_retry_after_without_reauthorizing() {
    let limited = StubSpotify::start(ProviderMode::Limited).await;
    let t = Test::with_spotify_provider(limited.url.clone(), Duration::from_secs(2)).await;
    let member = t.member("spotify_limited").await;
    let (_, state) = authorization(&t, &member.token).await;
    t.post(
        "/users/me/spotify/callback",
        &member.token,
        json!({"code":"stub-code","state":state}),
    )
    .await;
    let room = voice_room(&t).await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &member.token,
        json!({"url":"https://spotify.link/limited"}),
    )
    .await;
    assert!(jam(&t, &room, &member.token)
        .await
        .unwrap()
        .now_playing
        .is_none());
    assert!(jam(&t, &room, &member.token)
        .await
        .unwrap()
        .now_playing
        .is_none());
    assert_eq!(
        limited.state.playback_calls.load(Ordering::SeqCst),
        1,
        "Retry-After should suppress a second playback request"
    );
    assert_eq!(
        account(&t, &member.token).await.connection,
        SpotifyConnection::Connected,
        "a rate limit is transient, not a revoked grant"
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

async fn spotify_until_message(socket: &mut Socket, message_id: &str) -> Vec<SpotifyAccount> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut updates = Vec::new();
        loop {
            match socket.next().await.expect("stream stays open").unwrap() {
                Frame::Text(text) => match serde_json::from_str::<Event>(&text).unwrap() {
                    Event::SpotifyAccountUpdated { account, .. } => updates.push(account),
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
    .expect("socket still receives chat after Spotify update")
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
    assert_eq!(
        t.http
            .post(format!("{}{path}", t.url))
            .header("Cookie", format!("den_session={}", host.token))
            .json(&json!({"url":"https://spotify.link/csrf"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN,
        "cookie-authenticated Jam writes require origin and CSRF"
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

#[tokio::test]
async fn jams_in_a_dm_are_private_to_its_members() {
    let t = Test::new().await;
    let host = t.member("jam_private_host").await;
    let guest = t.member("jam_private_guest").await;
    let outsider = t.member("jam_private_outsider").await;
    let dm: Channel = serde_json::from_value(
        t.post("/dms", &host.token, json!({"member_ids":[guest.user.id]}))
            .await,
    )
    .unwrap();
    let path = format!("/rooms/{}/jam", dm.id);
    t.post(
        &path,
        &host.token,
        json!({"url":"https://spotify.link/private"}),
    )
    .await;
    for (method, target, body) in [
        (Method::GET, path.clone(), None),
        (
            Method::POST,
            path.clone(),
            Some(json!({"url":"https://spotify.link/stolen"})),
        ),
        (Method::POST, format!("{path}/join"), Some(json!({}))),
        (Method::DELETE, path.clone(), None),
    ] {
        let mut request = t.req(method.clone(), &target, &outsider.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        assert_eq!(
            request.send().await.unwrap().status(),
            StatusCode::NOT_FOUND,
            "{method} {target} must not reveal a private DM"
        );
    }
    assert_eq!(
        jam(&t, &dm.id, &guest.token).await.unwrap().host_id,
        host.user.id
    );
}

// Presence is registered after the Resync frame, so wait for the server to count a
// socket before polling; otherwise the poll can run before the person is online.
async fn online(t: &Test, user: &str) {
    for _ in 0..100 {
        let p: PresenceState = t
            .req(Method::GET, "/presence", &t.admin.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if p.online_user_ids.iter().any(|id| id == user) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{user} never came online");
}

#[tokio::test]
async fn listening_is_shared_only_by_online_people_who_chose_to() {
    let provider = StubSpotify::start(ProviderMode::Healthy).await;
    let t = Test::with_spotify_provider(provider.url.clone(), Duration::from_secs(2)).await;
    let here = t.member("listener_here").await;
    let away = t.member("listener_away").await;
    for member in [&here, &away] {
        let (_, state) = authorization(&t, &member.token).await;
        t.post(
            "/users/me/spotify/callback",
            &member.token,
            json!({"code":"stub-code","state":state}),
        )
        .await;
    }
    let _tab = socket(&t, &here.token, "").await;
    online(&t, &here.user.id).await;
    let activities = |t: &Test| t.req(Method::GET, "/activities", &t.admin.token).send();
    let sharing = |t: &Test, token: &str, on: bool| {
        t.req(Method::PUT, "/users/me/spotify/sharing", token)
            .json(&json!({"share_listening": on}))
            .send()
    };

    // Connected for Jams only: nothing shared, and Spotify is not even asked.
    let before = provider.state.playback_calls.load(Ordering::SeqCst);
    t.state.poll_spotify_activities().await;
    let all: Vec<UserActivities> = activities(&t).await.unwrap().json().await.unwrap();
    assert!(all.is_empty(), "connecting is not consent to share");
    assert_eq!(provider.state.playback_calls.load(Ordering::SeqCst), before);
    assert!(!account(&t, &here.token).await.share_listening);

    // Opting in shows the track to everyone while online; the offline account stays out.
    for member in [&here, &away] {
        let on: SpotifyAccount = sharing(&t, &member.token, true)
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(on.share_listening);
    }
    t.state.poll_spotify_activities().await;
    let all: Vec<UserActivities> = activities(&t).await.unwrap().json().await.unwrap();
    let mine = all
        .iter()
        .find(|u| u.user_id == here.user.id)
        .expect("the online listener");
    let song = &mine.activities[0];
    assert_eq!(
        (song.slot.as_str(), song.kind),
        ("spotify", ActivityKind::Listening)
    );
    assert_eq!(
        (song.name.as_str(), song.details.as_deref()),
        ("Stub Song", Some("Stub Artist"))
    );
    assert!(song.image_url.is_some(), "album art rides along");
    assert!(
        all.iter().all(|u| u.user_id != away.user.id),
        "nobody sees an offline account"
    );

    // Opting out takes it down at once, not at its expiry.
    sharing(&t, &here.token, false).await.unwrap();
    let all: Vec<UserActivities> = activities(&t).await.unwrap().json().await.unwrap();
    assert!(all.is_empty());

    // So does disconnecting, and a disconnected account cannot opt in.
    sharing(&t, &here.token, true).await.unwrap();
    t.state.poll_spotify_activities().await;
    t.req(Method::DELETE, "/users/me/spotify", &here.token)
        .send()
        .await
        .unwrap();
    let all: Vec<UserActivities> = activities(&t).await.unwrap().json().await.unwrap();
    assert!(all.is_empty());
    assert_eq!(
        sharing(&t, &here.token, true).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );

    // Clients cannot forge the server's slot.
    let forged = t
        .req(Method::PUT, "/users/me/activities/spotify", &here.token)
        .json(&json!({"kind":"listening","name":"Fake"}))
        .send()
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::BAD_REQUEST);
}
