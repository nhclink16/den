mod auth;
mod chat;
mod uploads;
mod ws;
mod openapi;

use axum::{extract::{FromRequest, Request, State}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post, delete}, Json, Router};
use den_core::*;
use serde::de::DeserializeOwned;
use sqlx::{sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions}, SqlitePool};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use tokio::sync::{broadcast, Mutex};

#[derive(Clone)]
pub struct AppState(pub(crate) Arc<Inner>);
#[doc(hidden)]
pub struct Inner {
    pub db: SqlitePool,
    pub uploads: PathBuf,
    pub bootstrap: PathBuf,
    pub origin: String,
    pub max_upload: i64,
    pub events: broadcast::Sender<Event>,
    pub writes: Mutex<()>,
    pub attempts: Mutex<HashMap<String, (Instant, u32)>>,
    pub ids: std::sync::Mutex<ulid::Generator>,
}
impl std::ops::Deref for AppState { type Target = Inner; fn deref(&self) -> &Inner { &self.0 } }
impl AppState {
    pub async fn open(db_path: PathBuf, uploads: PathBuf, bootstrap: PathBuf, origin: String, max_upload: i64) -> anyhow::Result<Self> {
        let uri: axum::http::Uri = origin.parse()?;
        anyhow::ensure!(uri.scheme_str() == Some("https") || (uri.scheme_str() == Some("http") && matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]"))), "DEN_ORIGIN must use HTTPS except on loopback");
        anyhow::ensure!(uri.path() == "/" && uri.query().is_none() && !origin.ends_with('/'), "DEN_ORIGIN must be an origin without a trailing slash");
        anyhow::ensure!(max_upload > 0, "DEN_MAX_UPLOAD_BYTES must be positive");
        if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) { tokio::fs::create_dir_all(parent).await?; }
        tokio::fs::create_dir_all(&uploads).await?;
        let options = SqliteConnectOptions::new().filename(db_path).create_if_missing(true).journal_mode(SqliteJournalMode::Wal).foreign_keys(true).busy_timeout(Duration::from_secs(5));
        let db = SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
        sqlx::migrate!().run(&db).await?;
        if sqlx::query_scalar!("SELECT count(*) FROM users").fetch_one(&db).await? == 0 {
            if let Some(parent) = bootstrap.parent().filter(|p| !p.as_os_str().is_empty()) { tokio::fs::create_dir_all(parent).await?; }
            let mut options = std::fs::OpenOptions::new(); options.write(true).create_new(true);
            #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
            match options.open(&bootstrap) {
                Ok(mut file) => { use std::io::Write; file.write_all(auth::secret().as_bytes())?; file.sync_all()?; },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {},
                Err(e) => return Err(e.into()),
            }
        }
        let (events, _) = broadcast::channel(256);
        Ok(Self(Arc::new(Inner { db, uploads, bootstrap, origin, max_upload, events, writes: Mutex::new(()), attempts: Mutex::new(HashMap::new()), ids: std::sync::Mutex::new(ulid::Generator::new()) })))
    }
    pub(crate) fn id(&self) -> String { self.ids.lock().expect("id mutex poisoned").generate().expect("ULID exhausted").to_string() }
    pub async fn cleanup(&self) -> anyhow::Result<()> { uploads::cleanup(self).await }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(openapi::serve))
        .route("/auth/init", post(auth::bootstrap))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/users", get(auth::users))
        .route("/users/me", get(auth::me))
        .route("/invites", post(auth::invite))
        .route("/invites/{id}", delete(auth::revoke_invite))
        .route("/tokens", get(auth::tokens).post(auth::create_token))
        .route("/tokens/{id}", delete(auth::revoke_token))
        .route("/bots", post(auth::bot))
        .route("/categories", get(chat::categories).post(chat::create_category))
        .route("/categories/{id}", axum::routing::put(chat::update_category).delete(chat::delete_category))
        .route("/channels", get(chat::channels).post(chat::create_channel))
        .route("/channels/{id}", get(chat::channel).put(chat::update_channel).delete(chat::delete_channel))
        .route("/dms", post(chat::dm))
        .route("/channels/{id}/messages", get(chat::messages).post(chat::send))
        .route("/messages/{id}", axum::routing::patch(chat::edit).delete(chat::remove))
        .route("/uploads", post(uploads::begin))
        .route("/uploads/{id}", get(uploads::status).patch(uploads::chunk))
        .route("/uploads/{id}/complete", post(uploads::complete))
        .route("/uploads/{id}/file", get(uploads::file).head(uploads::file))
        .route("/ws", get(ws::connect))
        .layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024))
        .with_state(state)
}
#[utoipa::path(get, path="/health", responses((status=200, body=Health)))]
async fn health() -> Json<Health> { Json(Health { ok: true, version: env!("CARGO_PKG_VERSION").into() }) }

pub(crate) fn now() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).expect("clock before epoch").as_secs() as i64 }
pub(crate) type Result<T> = std::result::Result<T, Error>;
pub(crate) struct Error(pub StatusCode, pub &'static str, pub String);
impl Error {
    pub fn bad(message: impl Into<String>) -> Self { Self(StatusCode::BAD_REQUEST, "invalid_request", message.into()) }
    pub fn unauthorized() -> Self { Self(StatusCode::UNAUTHORIZED, "unauthorized", "Authentication required".into()) }
    pub fn forbidden() -> Self { Self(StatusCode::FORBIDDEN, "forbidden", "Access denied".into()) }
    pub fn missing() -> Self { Self(StatusCode::NOT_FOUND, "not_found", "Not found".into()) }
    pub fn conflict(message: impl Into<String>) -> Self { Self(StatusCode::CONFLICT, "conflict", message.into()) }
}
impl IntoResponse for Error { fn into_response(self) -> Response { (self.0, Json(ApiError { error: self.1.into(), message: self.2 })).into_response() } }
impl From<sqlx::Error> for Error { fn from(e: sqlx::Error) -> Self {
    if matches!(e, sqlx::Error::RowNotFound) { return Self::missing(); }
    if let sqlx::Error::Database(ref db) = e { if db.is_unique_violation() { return Self::conflict("Already exists"); } if db.is_foreign_key_violation() { return Self::bad("Referenced item does not exist"); } }
    tracing::error!(error=%e, "database operation failed"); Self(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Database operation failed".into())
} }
impl From<std::io::Error> for Error { fn from(e: std::io::Error) -> Self { tracing::error!(error=%e, "file operation failed"); Self(StatusCode::INSUFFICIENT_STORAGE, "storage", "File operation failed; check server disk space".into()) } }
pub(crate) struct ApiJson<T>(pub T);
impl<T: DeserializeOwned + Send> FromRequest<AppState> for ApiJson<T> {
    type Rejection = Error;
    async fn from_request(req: Request, state: &AppState) -> Result<Self> {
        Json::<T>::from_request(req, state).await.map(|v| Self(v.0)).map_err(|e| Error(e.status(), "invalid_request", "Invalid JSON request".into()))
    }
}
pub(crate) fn name(value: &str) -> Result<()> { if value.trim().is_empty() || value.len() > 100 { Err(Error::bad("Name must contain 1 to 100 bytes")) } else { Ok(()) } }
