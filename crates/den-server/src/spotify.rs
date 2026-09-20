//! Spotify account linking. Read only: the two scopes below let Den see what
//! the host is playing and nothing else. No playback control, no writes.
//!
//! Only refresh tokens are persisted, encrypted, per user. Access tokens live in
//! memory and die with the process. A missing client secret is a disabled state,
//! never a crash: every route answers, and the account reads `unavailable`.
use crate::{
    auth::{self, Auth},
    *,
};
use rand::{rngs::OsRng, RngCore};
use ring::aead;
use serde::Deserialize;
use sqlx::Row;
use std::path::Path;

/// Client IDs are public and travel in the authorize URL.
const CLIENT_ID: &str = "9efa4ca0d79a4be5a934a22c229ad656";
const SCOPES: &str = "user-read-playback-state user-read-currently-playing";
const AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
const TOKEN: &str = "https://accounts.spotify.com/api/token";
const PROFILE: &str = "https://api.spotify.com/v1/me";
const PLAYING: &str = "https://api.spotify.com/v1/me/player/currently-playing";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Spotify stops honouring a refresh token 180 days after it was issued.
const REFRESH_LIFETIME: i64 = 180 * 24 * 60 * 60;
/// The user has to get through Spotify's own login inside this window.
const STATE_TTL: Duration = Duration::from_secs(600);
/// Re-reading the host's playback faster than this buys nothing and costs quota.
const PLAYBACK_TTL: Duration = Duration::from_secs(5);
/// Renew an access token before it lapses mid-request.
const ACCESS_MARGIN: Duration = Duration::from_secs(60);

pub(crate) struct Credentials {
    secret: String,
    key: aead::LessSafeKey,
    client: reqwest::Client,
    token_url: String,
    profile_url: String,
    playing_url: String,
}
/// `credentials` is `None` on a server with no client secret; the maps stay
/// empty there. Nothing in this struct reaches disk.
#[derive(Default)]
pub(crate) struct Spotify {
    credentials: Option<Credentials>,
    /// SHA-256 of a pending `state`, exactly as `tickets.rs` stores its tickets.
    states: Mutex<HashMap<String, (String, Instant, u64)>>,
    access: Mutex<HashMap<String, (String, Instant)>>,
    playback: Mutex<HashMap<String, (Option<SpotifyNowPlaying>, Instant)>>,
    /// Account lifecycle generation. A disconnect or a new authorization makes
    /// provider work already in flight for the previous grant ineligible.
    epochs: Mutex<HashMap<String, u64>>,
    /// Refresh and playback cache misses are rare. Serializing each kind keeps a
    /// reconnect burst from spending the same refresh token or Spotify quota in
    /// parallel, without a per-user lock registry for five people.
    refresh: Mutex<()>,
    sample: Mutex<()>,
}

impl AppState {
    /// Configure Spotify once before sharing the server state. No secret means
    /// one startup log and an `unavailable` account for everyone. Tests use
    /// `with_spotify` instead; they must not read process environment.
    pub async fn with_spotify_from_env(self) -> anyhow::Result<Self> {
        let secret = std::env::var("DEN_SPOTIFY_CLIENT_SECRET").unwrap_or_default();
        if secret.is_empty() {
            tracing::info!("Spotify is not configured; Jam cards work, now-playing is disabled");
            return Ok(self);
        }
        // Outside the database on purpose: a leaked .db alone must not decrypt.
        let path = PathBuf::from(
            std::env::var("DEN_SPOTIFY_KEY_FILE").unwrap_or_else(|_| "data/spotify.key".into()),
        );
        let key = data_key(&path).await?;
        self.with_spotify(secret, key)
    }
    pub fn with_spotify(mut self, secret: String, key: [u8; 32]) -> anyhow::Result<Self> {
        self.configure_spotify(
            secret,
            key,
            TOKEN.into(),
            PROFILE.into(),
            PLAYING.into(),
            REQUEST_TIMEOUT,
        )?;
        Ok(self)
    }
    /// Test-only provider injection. The ordinary server always uses Spotify's
    /// fixed HTTPS endpoints; integration tests use a loopback stub and no real
    /// account or secret.
    #[doc(hidden)]
    pub fn with_spotify_test_endpoint(
        mut self,
        secret: String,
        key: [u8; 32],
        base_url: String,
        timeout: Duration,
    ) -> anyhow::Result<Self> {
        let base = base_url.trim_end_matches('/');
        self.configure_spotify(
            secret,
            key,
            format!("{base}/api/token"),
            format!("{base}/v1/me"),
            format!("{base}/v1/me/player/currently-playing"),
            timeout,
        )?;
        Ok(self)
    }
    fn configure_spotify(
        &mut self,
        secret: String,
        key: [u8; 32],
        token_url: String,
        profile_url: String,
        playing_url: String,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &key)
            .map_err(|_| anyhow::anyhow!("Invalid Spotify data key"))?;
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout.min(CONNECT_TIMEOUT))
            .build()?;
        Arc::get_mut(&mut self.0)
            .expect("configure before sharing")
            .spotify
            .credentials = Some(Credentials {
            secret,
            key: aead::LessSafeKey::new(key),
            client,
            token_url,
            profile_url,
            playing_url,
        });
        Ok(())
    }
}

/// Same shape as the bootstrap key: create once at 0600, then read it back.
async fn data_key(path: &Path) -> anyhow::Result<[u8; 32]> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(auth::secret().as_bytes())?;
            file.sync_all()?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let metadata = tokio::fs::symlink_metadata(path).await?;
    anyhow::ensure!(
        metadata.file_type().is_file(),
        "DEN_SPOTIFY_KEY_FILE must be a regular file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        anyhow::ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "DEN_SPOTIFY_KEY_FILE must not be accessible by group or other users"
        );
    }
    let text = tokio::fs::read_to_string(path).await?;
    let text = text.trim();
    let invalid = || anyhow::anyhow!("DEN_SPOTIFY_KEY_FILE must hold 32 hex-encoded bytes");
    anyhow::ensure!(text.len() == 64, invalid());
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|_| invalid())?;
    }
    Ok(key)
}

fn unavailable() -> Error {
    Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "spotify_unavailable",
        "Spotify is not set up on this server.".into(),
    )
}

async fn epoch(s: &AppState, user: &str) -> u64 {
    s.spotify
        .epochs
        .lock()
        .await
        .get(user)
        .copied()
        .unwrap_or(0)
}

async fn advance_epoch(s: &AppState, user: &str) -> u64 {
    let mut epochs = s.spotify.epochs.lock().await;
    let epoch = epochs.entry(user.into()).or_default();
    *epoch += 1;
    *epoch
}

fn credentials(s: &AppState) -> Result<&Credentials> {
    s.spotify.credentials.as_ref().ok_or_else(unavailable)
}
fn opaque(what: &'static str) -> Error {
    // Spotify's own error bodies and transport errors can carry the code, the
    // token, or the secret in a URL. Replace them wholesale, never forward them.
    tracing::warn!(step = what, "Spotify request failed");
    Error(
        StatusCode::BAD_GATEWAY,
        "spotify_unreachable",
        "Spotify did not answer. Try again in a moment.".into(),
    )
}

fn seal(c: &Credentials, user_id: &str, token: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let mut buffer = token.as_bytes().to_vec();
    c.key
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(user_id.as_bytes()),
            &mut buffer,
        )
        .map_err(|_| {
            Error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Cannot store the Spotify connection".into(),
            )
        })?;
    Ok((nonce.to_vec(), buffer))
}
/// `None` for a wrong key, a tampered row, or a row moved to another user.
fn unseal(c: &Credentials, user_id: &str, nonce: &[u8], ciphertext: &[u8]) -> Option<String> {
    let nonce = aead::Nonce::try_assume_unique_for_key(nonce).ok()?;
    let mut buffer = ciphertext.to_vec();
    let plain = c
        .key
        .open_in_place(nonce, aead::Aad::from(user_id.as_bytes()), &mut buffer)
        .ok()?;
    String::from_utf8(plain.to_vec()).ok()
}

struct Stored {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    key_version: i64,
    account_name: Option<String>,
    connected_at: i64,
    expires_at: i64,
    needs_reauth: bool,
}
async fn stored(s: &AppState, user: &str) -> Result<Option<Stored>> {
    Ok(sqlx::query("SELECT refresh_nonce,refresh_ciphertext,key_version,account_name,connected_at,expires_at,needs_reauth FROM spotify_accounts WHERE user_id=?")
        .bind(user)
        .fetch_optional(&s.db)
        .await?
        .map(|r| Stored {
            nonce: r.get("refresh_nonce"),
            ciphertext: r.get("refresh_ciphertext"),
            key_version: r.get("key_version"),
            account_name: r.get("account_name"),
            connected_at: r.get("connected_at"),
            expires_at: r.get("expires_at"),
            needs_reauth: r.get::<i64, _>("needs_reauth") != 0,
        }))
}

pub(crate) async fn view(s: &AppState, user: &str) -> Result<SpotifyAccount> {
    if s.spotify.credentials.is_none() {
        return Ok(SpotifyAccount {
            connection: SpotifyConnection::Unavailable,
            account_name: None,
            connected_at: None,
            expires_at: None,
        });
    }
    let Some(row) = stored(s, user).await? else {
        return Ok(SpotifyAccount {
            connection: SpotifyConnection::Disconnected,
            account_name: None,
            connected_at: None,
            expires_at: None,
        });
    };
    // The 180-day cliff is a state to re-authorise from, not a failure.
    let dead = row.needs_reauth || row.expires_at <= now() || row.key_version != 1;
    Ok(SpotifyAccount {
        connection: if dead {
            SpotifyConnection::Reauthorize
        } else {
            SpotifyConnection::Connected
        },
        account_name: row.account_name,
        connected_at: Some(row.connected_at),
        expires_at: Some(row.expires_at),
    })
}
async fn announce(s: &AppState, user: &str) {
    if let Ok(account) = view(s, user).await {
        let _ = s.events.send(Event::SpotifyAccountUpdated {
            user_id: user.into(),
            account,
        });
    }
}

#[derive(Deserialize)]
struct Granted {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}
async fn grant(c: &Credentials, form: &[(&str, &str)]) -> Result<Option<Granted>> {
    let response = c
        .client
        .post(&c.token_url)
        .basic_auth(CLIENT_ID, Some(&c.secret))
        .form(form)
        .send()
        .await
        .map_err(|_| opaque("token"))?;
    let status = response.status();
    if status.is_success() {
        return response.json().await.map(Some).map_err(|_| opaque("token"));
    }
    // 400 invalid_grant is the authoritative "this refresh token is dead".
    #[derive(Deserialize)]
    struct Rejected {
        error: Option<String>,
    }
    let dead = status == reqwest::StatusCode::BAD_REQUEST
        && response
            .json::<Rejected>()
            .await
            .ok()
            .and_then(|v| v.error)
            .as_deref()
            == Some("invalid_grant");
    if dead {
        return Ok(None);
    }
    Err(opaque("token"))
}

fn has_required_scopes(granted: &Granted) -> bool {
    let Some(scope) = granted.scope.as_deref() else {
        // OAuth permits omitting `scope` when it is identical to the request.
        return true;
    };
    SCOPES
        .split_ascii_whitespace()
        .all(|required| scope.split_ascii_whitespace().any(|v| v == required))
}

/// The redirect Spotify must have registered. Derived from `DEN_ORIGIN`, which
/// is why local testing has to run and browse the same loopback origin:
/// Spotify rejects `localhost`, and `localhost` and `127.0.0.1` are different
/// origins to the browser, to the session cookie, and to the CSRF check.
fn redirect_uri(s: &AppState) -> String {
    format!("{}/spotify/callback", s.origin)
}

#[utoipa::path(get,path="/users/me/spotify",responses((status=200,body=SpotifyAccount)))]
pub(crate) async fn account(State(s): State<AppState>, a: Auth) -> Result<Json<SpotifyAccount>> {
    Ok(Json(view(&s, &a.user.id).await?))
}

#[utoipa::path(post,path="/users/me/spotify/authorize",responses((status=200,body=SpotifyAuthorization)))]
pub(crate) async fn authorize(
    State(s): State<AppState>,
    a: Auth,
) -> Result<Json<SpotifyAuthorization>> {
    credentials(&s)?;
    // Starting a new authorization supersedes every older attempt for this
    // account, including a callback already waiting on Spotify.
    let attempt_epoch = advance_epoch(&s, &a.user.id).await;
    let state = auth::secret();
    {
        let at = Instant::now();
        let mut states = s.spotify.states.lock().await;
        // One pending authorization per user; this also bounds the map.
        states.retain(|_, (user, expiry, _)| *expiry > at && user != &a.user.id);
        states.insert(
            auth::hash(&state),
            (a.user.id.clone(), at + STATE_TTL, attempt_epoch),
        );
    }
    let redirect = redirect_uri(&s);
    let url = reqwest::Url::parse_with_params(
        AUTHORIZE,
        &[
            ("client_id", CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", redirect.as_str()),
            ("scope", SCOPES),
            ("state", state.as_str()),
        ],
    )
    .map_err(|_| opaque("authorize"))?;
    Ok(Json(SpotifyAuthorization {
        url: url.into(),
        expires_in: STATE_TTL.as_secs() as u32,
    }))
}

#[utoipa::path(post,path="/users/me/spotify/callback",request_body=CompleteSpotifyAuth,responses((status=200,body=SpotifyAccount)))]
pub(crate) async fn callback(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<CompleteSpotifyAuth>,
) -> Result<Json<SpotifyAccount>> {
    let c = credentials(&s)?;
    let owner = s
        .spotify
        .states
        .lock()
        .await
        .remove(&auth::hash(&v.state))
        .filter(|(_, expiry, _)| *expiry > Instant::now())
        .map(|(user, _, attempt)| (user, attempt));
    // Single use, and bound to the user who started it.
    let Some((owner, attempt_epoch)) = owner else {
        return Err(Error::bad(
            "That Spotify link expired. Start again from Settings.",
        ));
    };
    if owner != a.user.id || epoch(&s, &a.user.id).await != attempt_epoch {
        return Err(Error::bad(
            "That Spotify link expired. Start again from Settings.",
        ));
    }
    let redirect = redirect_uri(&s);
    let granted = grant(
        c,
        &[
            ("grant_type", "authorization_code"),
            ("code", v.code.as_str()),
            ("redirect_uri", redirect.as_str()),
        ],
    )
    .await?
    .ok_or_else(|| Error::bad("Spotify rejected that sign-in. Try connecting again."))?;
    if !has_required_scopes(&granted) {
        return Err(Error::bad(
            "Spotify did not grant both read-only playback permissions. Connect again.",
        ));
    }
    let refresh = granted.refresh_token.as_deref().ok_or_else(|| {
        Error::bad("Spotify did not return a refresh token. Try connecting again.")
    })?;
    let name = display_name(c, &granted.access_token).await;
    let (nonce, ciphertext) = seal(c, &a.user.id, refresh)?;
    let _refresh = s.spotify.refresh.lock().await;
    // Disconnect or a newer authorization may have happened while Spotify was
    // answering. Check again under the refresh lock before writing the grant.
    if epoch(&s, &a.user.id).await != attempt_epoch {
        return Err(Error::bad(
            "That Spotify link expired. Start again from Settings.",
        ));
    }
    let time = now();
    sqlx::query("INSERT INTO spotify_accounts(user_id,refresh_nonce,refresh_ciphertext,key_version,account_name,scopes,connected_at,expires_at,needs_reauth) VALUES(?,?,?,1,?,?,?,?,0) ON CONFLICT(user_id) DO UPDATE SET refresh_nonce=excluded.refresh_nonce,refresh_ciphertext=excluded.refresh_ciphertext,key_version=1,account_name=excluded.account_name,scopes=excluded.scopes,connected_at=excluded.connected_at,expires_at=excluded.expires_at,needs_reauth=0")
        .bind(&a.user.id)
        .bind(&nonce)
        .bind(&ciphertext)
        .bind(&name)
        .bind(granted.scope.as_deref().unwrap_or(SCOPES))
        .bind(time)
        .bind(time + REFRESH_LIFETIME)
        .execute(&s.db)
        .await?;
    remember(&s, &a.user.id, &granted).await;
    s.spotify.playback.lock().await.remove(&a.user.id);
    let account = view(&s, &a.user.id).await?;
    let _ = s.events.send(Event::SpotifyAccountUpdated {
        user_id: a.user.id.clone(),
        account: account.clone(),
    });
    Ok(Json(account))
}

#[utoipa::path(delete,path="/users/me/spotify",responses((status=204,description="Disconnected")))]
pub(crate) async fn disconnect(State(s): State<AppState>, a: Auth) -> Result<StatusCode> {
    // Actually delete the grant. There is no disabled-but-retained state.
    // Advance first so a provider response already in flight cannot repopulate
    // either token cache after this request begins.
    advance_epoch(&s, &a.user.id).await;
    s.spotify
        .states
        .lock()
        .await
        .retain(|_, (user, _, _)| user != &a.user.id);
    s.spotify.access.lock().await.remove(&a.user.id);
    s.spotify.playback.lock().await.remove(&a.user.id);
    let _refresh = s.spotify.refresh.lock().await;
    sqlx::query("DELETE FROM spotify_accounts WHERE user_id=?")
        .bind(&a.user.id)
        .execute(&s.db)
        .await?;
    // A refresh that won the lock before disconnect may have filled these after
    // the first clear. The account epoch prevents playback from using it, and
    // this second clear removes the stale material for good.
    s.spotify.access.lock().await.remove(&a.user.id);
    s.spotify.playback.lock().await.remove(&a.user.id);
    announce(&s, &a.user.id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remember(s: &AppState, user: &str, granted: &Granted) {
    let ttl = Duration::from_secs(granted.expires_in.unwrap_or(3600).clamp(60, 86_400));
    s.spotify.access.lock().await.insert(
        user.into(),
        (
            granted.access_token.clone(),
            Instant::now() + ttl.saturating_sub(ACCESS_MARGIN),
        ),
    );
}

/// A usable access token, or `None` when this user has no working connection.
/// Never an error: a dead grant is a UI state, and every caller degrades to the
/// Jam card's no-connection form.
async fn access_token(s: &AppState, user: &str) -> Option<String> {
    let c = s.spotify.credentials.as_ref()?;
    if let Some((token, expiry)) = s.spotify.access.lock().await.get(user) {
        if *expiry > Instant::now() {
            return Some(token.clone());
        }
    }
    let _refresh = s.spotify.refresh.lock().await;
    let grant_epoch = epoch(s, user).await;
    // Another request may have renewed it while this one waited.
    if let Some((token, expiry)) = s.spotify.access.lock().await.get(user) {
        if *expiry > Instant::now() {
            return Some(token.clone());
        }
    }
    let row = stored(s, user).await.ok()??;
    if row.needs_reauth || row.key_version != 1 || row.expires_at <= now() {
        let _ = reauthorize(s, user).await;
        return None;
    }
    let refresh = unseal(c, user, &row.nonce, &row.ciphertext);
    let Some(refresh) = refresh else {
        // Undecryptable is indistinguishable from revoked, from the user's side.
        let _ = reauthorize(s, user).await;
        return None;
    };
    let granted = grant(
        c,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
        ],
    )
    .await;
    let granted = match granted {
        Ok(Some(granted)) => granted,
        // Spotify said the refresh token is dead: the 180-day cliff, or a revoke.
        Ok(None) => {
            let _ = reauthorize(s, user).await;
            return None;
        }
        // Transient. Leave the stored grant alone and try again next time.
        Err(_) => return None,
    };
    if !has_required_scopes(&granted) {
        let _ = reauthorize(s, user).await;
        return None;
    }
    // Disconnect/reconnect may have happened during the provider request. Its
    // response belongs to the old grant and must not rotate or cache anything.
    if epoch(s, user).await != grant_epoch {
        return None;
    }
    // Spotify may hand back a rotated refresh token; the old one stops working.
    // Rotation does not restart Spotify's six-month lifetime: refreshing an
    // access token never extends the original authorization grant.
    if let Some(rotated) = granted.refresh_token.as_deref() {
        if let Ok((nonce, ciphertext)) = seal(c, user, rotated) {
            let _ = sqlx::query("UPDATE spotify_accounts SET refresh_nonce=?,refresh_ciphertext=?,key_version=1 WHERE user_id=?")
                .bind(&nonce).bind(&ciphertext).bind(user)
                .execute(&s.db).await;
        }
    }
    remember(s, user, &granted).await;
    Some(granted.access_token)
}
async fn reauthorize(s: &AppState, user: &str) -> Result<()> {
    advance_epoch(s, user).await;
    sqlx::query("UPDATE spotify_accounts SET needs_reauth=1 WHERE user_id=?")
        .bind(user)
        .execute(&s.db)
        .await?;
    s.spotify.access.lock().await.remove(user);
    s.spotify.playback.lock().await.remove(user);
    announce(s, user).await;
    Ok(())
}

async fn display_name(c: &Credentials, access: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Profile {
        display_name: Option<String>,
        id: Option<String>,
    }
    let response = c
        .client
        .get(&c.profile_url)
        .bearer_auth(access)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let profile: Profile = response.json().await.ok()?;
    profile.display_name.or(profile.id)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_millis() as i64
}

/// The last sample, without touching the network. Used when broadcasting a Jam
/// change so the card does not blink its track off and back on.
pub(crate) async fn cached(s: &AppState, user: &str) -> Option<SpotifyNowPlaying> {
    match s.spotify.playback.lock().await.get(user) {
        Some((value, expiry)) if *expiry > Instant::now() => value.clone(),
        _ => None,
    }
}

/// What the host is playing right now, or `None` for "we cannot see". A host
/// with no connection, a dead grant, private-session mode and a paused-nothing
/// account are all the same answer to the card.
pub(crate) async fn now_playing(s: &AppState, user: &str) -> Option<SpotifyNowPlaying> {
    s.spotify.credentials.as_ref()?;
    {
        let _epochs = s.spotify.epochs.lock().await;
        let playback = s.spotify.playback.lock().await;
        if let Some((value, expiry)) = playback.get(user) {
            if *expiry > Instant::now() {
                return value.clone();
            }
        }
    }
    let _sample = s.spotify.sample.lock().await;
    // A concurrent room read may have filled the cache while this one waited.
    let sample_epoch = {
        let epochs = s.spotify.epochs.lock().await;
        let sample_epoch = epochs.get(user).copied().unwrap_or(0);
        let playback = s.spotify.playback.lock().await;
        if let Some((value, expiry)) = playback.get(user) {
            if *expiry > Instant::now() {
                return value.clone();
            }
        }
        sample_epoch
    };
    let value = match access_token(s, user).await {
        Some(token) => fetch_playing(s, user, &token).await,
        None => PlaybackFetch::default(),
    };
    // Compare and publish under the account-generation lock. A disconnect that
    // already advanced the epoch wins; one that follows clears this cache.
    let epochs = s.spotify.epochs.lock().await;
    if epochs.get(user).copied().unwrap_or(0) != sample_epoch {
        return None;
    }
    s.spotify.playback.lock().await.insert(
        user.into(),
        (value.value.clone(), Instant::now() + value.ttl),
    );
    value.value
}
struct PlaybackFetch {
    value: Option<SpotifyNowPlaying>,
    ttl: Duration,
}
impl Default for PlaybackFetch {
    fn default() -> Self {
        Self {
            value: None,
            ttl: PLAYBACK_TTL,
        }
    }
}

fn playback_retry_after(headers: &reqwest::header::HeaderMap) -> Duration {
    let seconds = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(PLAYBACK_TTL.as_secs())
        // A nonsensical value must not overflow `Instant`, but there is no
        // reason to retry before this grant's entire remaining lifetime.
        .clamp(PLAYBACK_TTL.as_secs(), REFRESH_LIFETIME as u64);
    Duration::from_secs(seconds)
}

async fn fetch_playing(s: &AppState, user: &str, access: &str) -> PlaybackFetch {
    #[derive(Deserialize)]
    struct Playing {
        #[serde(default)]
        is_playing: bool,
        progress_ms: Option<i64>,
        item: Option<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        name: String,
        duration_ms: Option<i64>,
        #[serde(default)]
        artists: Vec<Named>,
        album: Option<Album>,
    }
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    #[derive(Deserialize)]
    struct Album {
        #[serde(default)]
        images: Vec<Image>,
    }
    #[derive(Deserialize)]
    struct Image {
        url: String,
        width: Option<i64>,
    }
    let Some(c) = s.spotify.credentials.as_ref() else {
        return PlaybackFetch::default();
    };
    let response = match c
        .client
        .get(&c.playing_url)
        .bearer_auth(access)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return PlaybackFetch::default(),
    };
    // 204 is Spotify for "nothing is playing", not an error.
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return PlaybackFetch::default();
    }
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        s.spotify.access.lock().await.remove(user);
        return PlaybackFetch::default();
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return PlaybackFetch {
            value: None,
            ttl: playback_retry_after(response.headers()),
        };
    }
    if !response.status().is_success() {
        return PlaybackFetch::default();
    }
    let Ok(playing) = response.json::<Playing>().await else {
        return PlaybackFetch::default();
    };
    let Some(item) = playing.item else {
        return PlaybackFetch::default();
    };
    let mut images = item.album.map(|a| a.images).unwrap_or_default();
    // Smallest art at least 200px wide; the card is a thumbnail, not a poster.
    images.sort_by_key(|i| i.width.unwrap_or(i64::MAX));
    let art = images
        .iter()
        .find(|i| i.width.is_none_or(|w| w >= 200))
        .or(images.first())
        .map(|i| i.url.clone());
    PlaybackFetch {
        value: Some(SpotifyNowPlaying {
            track: item.name,
            artists: item
                .artists
                .into_iter()
                .map(|a| a.name)
                .collect::<Vec<_>>()
                .join(", "),
            album_art: art,
            duration_ms: item.duration_ms,
            progress_ms: playing.progress_ms,
            is_playing: playing.is_playing,
            sampled_at: now_ms(),
        }),
        ttl: PLAYBACK_TTL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key() -> Credentials {
        Credentials {
            secret: "not-a-real-secret".into(),
            key: aead::LessSafeKey::new(
                aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &[7u8; 32]).unwrap(),
            ),
            client: reqwest::Client::new(),
            token_url: String::new(),
            profile_url: String::new(),
            playing_url: String::new(),
        }
    }
    #[test]
    fn refresh_tokens_round_trip_and_are_bound_to_one_user() {
        let c = key();
        let (nonce, ciphertext) = seal(&c, "user-a", "AQC-refresh").ok().expect("sealed");
        assert!(!ciphertext.windows(3).any(|w| w == b"AQC"));
        assert_eq!(
            unseal(&c, "user-a", &nonce, &ciphertext).as_deref(),
            Some("AQC-refresh")
        );
        // A row copied onto another user, or a flipped byte, does not open.
        assert!(unseal(&c, "user-b", &nonce, &ciphertext).is_none());
        let mut tampered = ciphertext.clone();
        tampered[0] ^= 1;
        assert!(unseal(&c, "user-a", &nonce, &tampered).is_none());
        let other = Credentials {
            secret: c.secret.clone(),
            key: aead::LessSafeKey::new(
                aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &[8u8; 32]).unwrap(),
            ),
            client: reqwest::Client::new(),
            token_url: String::new(),
            profile_url: String::new(),
            playing_url: String::new(),
        };
        assert!(unseal(&other, "user-a", &nonce, &ciphertext).is_none());
    }
    #[test]
    fn each_seal_uses_a_fresh_nonce() {
        let c = key();
        let (first, _) = seal(&c, "user-a", "AQC-refresh").ok().expect("sealed");
        let (second, _) = seal(&c, "user-a", "AQC-refresh").ok().expect("sealed");
        assert_ne!(first, second);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn data_key_is_created_private_and_rejects_an_exposed_file() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("den-spotify-key-{}", ulid::Ulid::new()));
        let key = data_key(&path).await.expect("create key");
        assert_ne!(key, [0u8; 32]);
        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(data_key(&path).await.is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn playback_backoff_keeps_the_providers_full_retry_after() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
        assert_eq!(playback_retry_after(&headers), Duration::from_secs(120));
    }
}
