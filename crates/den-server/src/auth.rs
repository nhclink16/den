use crate::*;
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts, HeaderMap, Method},
};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;

pub(crate) fn secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub(crate) struct DbUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bot: bool,
    pub role: String,
}
impl From<DbUser> for User {
    fn from(v: DbUser) -> Self {
        Self {
            id: v.id,
            username: v.username,
            display_name: v.display_name,
            avatar_url: v.avatar_url,
            bot: v.bot,
            role: if v.role == "admin" {
                Role::Admin
            } else {
                Role::Member
            },
        }
    }
}
pub(crate) async fn user(state: &AppState, id: &str) -> Result<User> {
    Ok(sqlx::query_as!(DbUser, "SELECT id,username,display_name,avatar_url,bot as \"bot: bool\",role FROM users WHERE id=?", id).fetch_one(&state.db).await?.into())
}
#[derive(Clone)]
pub(crate) struct Auth {
    pub user: User,
    pub credential: String,
    pub session: bool,
}
impl Auth {
    pub fn admin(&self) -> Result<()> {
        if self.user.role == Role::Admin {
            Ok(())
        } else {
            Err(Error::forbidden())
        }
    }
    pub async fn valid(&self, s: &AppState) -> bool {
        if self.session {
            let time = now();
            sqlx::query_scalar!(
                "SELECT count(*) FROM sessions WHERE id=? AND expires_at>?",
                self.credential,
                time
            )
            .fetch_one(&s.db)
            .await
            .unwrap_or(0)
                == 1
        } else {
            sqlx::query_scalar!("SELECT count(*) FROM tokens WHERE id=?", self.credential)
                .fetch_one(&s.db)
                .await
                .unwrap_or(0)
                == 1
        }
    }
}
impl FromRequestParts<AppState> for Auth {
    type Rejection = Error;
    async fn from_request_parts(parts: &mut Parts, s: &AppState) -> Result<Self> {
        if parts.method == Method::GET && parts.uri.path() == "/ws" {
            let query = axum::extract::Query::<HashMap<String, String>>::try_from_uri(&parts.uri)
                .map_err(|_| Error::unauthorized())?;
            if let Some(ticket) = query.get("ticket") {
                let auth = s
                    .tickets
                    .lock()
                    .await
                    .take(ticket, Instant::now())
                    .ok_or_else(Error::unauthorized)?;
                if !auth.valid(s).await {
                    return Err(Error::unauthorized());
                }
                return Ok(auth);
            }
        }
        let bearer = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        let cookie = parts
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.split(';')
                    .map(str::trim)
                    .find_map(|p| p.strip_prefix("den_session="))
            });
        let digest = hash(bearer.or(cookie).ok_or_else(Error::unauthorized)?);
        let time = now();
        if let Some(row) = sqlx::query!(
            "SELECT id,user_id,csrf_hash FROM sessions WHERE secret_hash=? AND expires_at>?",
            digest,
            time
        )
        .fetch_optional(&s.db)
        .await?
        {
            if bearer.is_none() {
                let origin = parts
                    .headers
                    .get(header::ORIGIN)
                    .and_then(|v| v.to_str().ok());
                if !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS) {
                    let csrf = parts
                        .headers
                        .get("x-csrf-token")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if origin != Some(&s.origin) && hash(csrf) != row.csrf_hash {
                        return Err(Error::forbidden());
                    }
                }
                if parts.uri.path() == "/ws" && origin != Some(&s.origin) {
                    return Err(Error::forbidden());
                }
            }
            return Ok(Self {
                user: user(s, &row.user_id).await?,
                credential: row.id,
                session: true,
            });
        }
        if bearer.is_some() {
            if let Some(row) =
                sqlx::query!("SELECT id,user_id FROM tokens WHERE secret_hash=?", digest)
                    .fetch_optional(&s.db)
                    .await?
            {
                return Ok(Self {
                    user: user(s, &row.user_id).await?,
                    credential: row.id,
                    session: false,
                });
            }
        }
        Err(Error::unauthorized())
    }
}
async fn throttle(s: &AppState, addr: SocketAddr) -> Result<()> {
    let mut map = s.attempts.lock().await;
    map.retain(|_, (t, _)| t.elapsed() < Duration::from_secs(60));
    if map.len() >= 10000 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Try again in a minute".into(),
        ));
    }
    let entry = map
        .entry(addr.ip().to_string())
        .or_insert((Instant::now(), 0));
    entry.1 += 1;
    if entry.1 > 10 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Try again in a minute".into(),
        ));
    }
    Ok(())
}
pub(crate) fn username(value: &str) -> Result<()> {
    if !(3..=32).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(Error::bad(
            "Username must be 3-32 lowercase letters, digits or underscores",
        ));
    }
    Ok(())
}
async fn password(value: String) -> Result<String> {
    if !(12..=1024).contains(&value.len()) {
        return Err(Error::bad("Password must be 12-1024 bytes"));
    }
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(value.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|h| h.to_string())
    })
    .await
    .map_err(|_| Error::bad("Password processing failed"))?
    .map_err(|_| Error::bad("Password processing failed"))
}
async fn session(s: &AppState, id: &str) -> Result<Response> {
    let token = secret();
    let csrf = secret();
    let digest = hash(&token);
    let csrf_hash = hash(&csrf);
    let sid = s.id();
    let expiry = now() + 90 * 86400;
    sqlx::query!(
        "INSERT INTO sessions(id,user_id,secret_hash,csrf_hash,expires_at) VALUES(?,?,?,?,?)",
        sid,
        id,
        digest,
        csrf_hash,
        expiry
    )
    .execute(&s.db)
    .await?;
    let body = Session {
        token: token.clone(),
        csrf_token: csrf,
        expires_at: expiry,
        user: user(s, id).await?,
    };
    let secure = if s.origin.starts_with("https:") {
        "; Secure"
    } else {
        ""
    };
    let cookie =
        format!("den_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=7776000{secure}");
    Ok((
        [
            (header::SET_COOKIE, cookie),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Json(body),
    )
        .into_response())
}
fn login_origin(s: &AppState, headers: &HeaderMap) -> Result<()> {
    if let Some(origin) = headers.get(header::ORIGIN) {
        if origin.to_str().ok() != Some(&s.origin) {
            return Err(Error::forbidden());
        }
    }
    Ok(())
}
#[utoipa::path(post,path="/auth/init",request_body=Bootstrap,responses((status=200,body=Session),(status=409,body=ApiError)))]
pub(crate) async fn bootstrap(
    State(s): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ApiJson(v): ApiJson<Bootstrap>,
) -> Result<Response> {
    throttle(&s, addr).await?;
    login_origin(&s, &headers)?;
    username(&v.username)?;
    let _guard = s.writes.lock().await;
    if sqlx::query_scalar!("SELECT count(*) FROM users")
        .fetch_one(&s.db)
        .await?
        != 0
    {
        return Err(Error::conflict("Instance is already initialized"));
    }
    let expected = tokio::fs::read_to_string(&s.bootstrap).await?;
    if hash(v.bootstrap_token.trim()) != hash(expected.trim()) {
        return Err(Error::forbidden());
    }
    let pass = password(v.password).await?;
    let id = s.id();
    let channel = s.id();
    let mut tx = s.db.begin().await?;
    sqlx::query!(
        "INSERT INTO users(id,username,display_name,password_hash,role) VALUES(?,?,?,?,'admin')",
        id,
        v.username,
        v.username,
        pass
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO channels(id,name,kind) VALUES(?,'general','text')",
        channel
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let _ = tokio::fs::remove_file(&s.bootstrap).await;
    session(&s, &id).await
}
#[utoipa::path(post,path="/auth/register",request_body=Register,responses((status=200,body=Session),(status=400,body=ApiError)))]
pub(crate) async fn register(
    State(s): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ApiJson(v): ApiJson<Register>,
) -> Result<Response> {
    throttle(&s, addr).await?;
    login_origin(&s, &headers)?;
    username(&v.username)?;
    let pass = password(v.password).await?;
    let digest = hash(&v.invite);
    let time = now();
    let id = s.id();
    let mut tx = s.db.begin().await?;
    let used=sqlx::query!("UPDATE invites SET uses_left=uses_left-1 WHERE secret_hash=? AND uses_left>0 AND expires_at>?",digest,time).execute(&mut *tx).await?;
    if used.rows_affected() != 1 {
        return Err(Error::bad("Invite is invalid, expired or exhausted"));
    }
    sqlx::query!(
        "INSERT INTO users(id,username,display_name,password_hash,role) VALUES(?,?,?,?,'member')",
        id,
        v.username,
        v.username,
        pass
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    session(&s, &id).await
}
#[utoipa::path(post,path="/auth/login",request_body=Login,responses((status=200,body=Session),(status=401,body=ApiError)))]
pub(crate) async fn login(
    State(s): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ApiJson(v): ApiJson<Login>,
) -> Result<Response> {
    throttle(&s, addr).await?;
    login_origin(&s, &headers)?;
    if v.password.len() > 1024 {
        return Err(Error::unauthorized());
    }
    let row = sqlx::query!(
        "SELECT id,password_hash FROM users WHERE username=? AND bot=0",
        v.username
    )
    .fetch_optional(&s.db)
    .await?;
    // Hash even unknown accounts to keep the expensive path comparable.
    let stored = row.as_ref().and_then(|r| r.password_hash.clone());
    let valid = tokio::task::spawn_blocking(move || {
        if let Some(stored) = stored {
            PasswordHash::new(&stored).is_ok_and(|h| {
                Argon2::default()
                    .verify_password(v.password.as_bytes(), &h)
                    .is_ok()
            })
        } else {
            let _ = Argon2::default()
                .hash_password(v.password.as_bytes(), &SaltString::generate(&mut OsRng));
            false
        }
    })
    .await
    .unwrap_or(false);
    if !valid {
        return Err(Error::unauthorized());
    }
    session(&s, &row.ok_or_else(Error::unauthorized)?.id).await
}
#[utoipa::path(post,path="/auth/logout",responses((status=204)))]
pub(crate) async fn logout(State(s): State<AppState>, a: Auth) -> Result<Response> {
    if a.session {
        sqlx::query!("DELETE FROM sessions WHERE id=?", a.credential)
            .execute(&s.db)
            .await?;
    } else {
        sqlx::query!("DELETE FROM tokens WHERE id=?", a.credential)
            .execute(&s.db)
            .await?;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            "den_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
    )
        .into_response())
}
#[utoipa::path(get,path="/users/me",responses((status=200,body=User)))]
pub(crate) async fn me(a: Auth) -> Json<User> {
    Json(a.user)
}
#[utoipa::path(get,path="/users",responses((status=200,body=Vec<User>)))]
pub(crate) async fn users(State(s): State<AppState>, _a: Auth) -> Result<Json<Vec<User>>> {
    Ok(Json(sqlx::query_as!(DbUser,"SELECT id,username,display_name,avatar_url,bot as \"bot: bool\",role FROM users ORDER BY id").fetch_all(&s.db).await?.into_iter().map(Into::into).collect()))
}
