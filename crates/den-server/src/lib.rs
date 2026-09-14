mod access;
mod activity;
mod appearance;
mod auth;
mod calls;
mod chat;
mod credentials;
mod hosts;
mod inbox;
mod messages;
mod objects;
mod openapi;
pub mod portable;
mod terminal;
mod thumbnails;
mod tickets;
mod uploads;
mod web;
mod ws;

use axum::{
    extract::{FromRequest, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use den_core::*;
use serde::de::DeserializeOwned;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, Mutex};

#[derive(Clone)]
pub struct AppState(pub(crate) Arc<Inner>);
type ObjectPresenceConnections = HashMap<String, (String, HashMap<String, usize>)>;
#[doc(hidden)]
pub struct Inner {
    pub db: SqlitePool,
    pub(crate) hosts: hosts::Hosts,
    pub livekit: Option<calls::LiveKit>,
    pub calls: Mutex<HashMap<String, HashMap<String, String>>>,
    pub uploads: PathBuf,
    pub bootstrap: PathBuf,
    pub origin: String,
    pub max_upload: i64,
    pub events: broadcast::Sender<Event>,
    pub writes: Mutex<()>,
    pub(crate) tickets: Mutex<tickets::Tickets>,
    pub attempts: Mutex<HashMap<String, (Instant, u32)>>,
    pub presence: std::sync::Mutex<HashMap<String, usize>>,
    pub object_presence: std::sync::Mutex<ObjectPresenceConnections>,
    pub thumbnails: Arc<tokio::sync::Semaphore>,
    pub ids: std::sync::Mutex<ulid::Generator>,
}
impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}
impl AppState {
    pub async fn open(
        db_path: PathBuf,
        uploads: PathBuf,
        bootstrap: PathBuf,
        origin: String,
        max_upload: i64,
    ) -> anyhow::Result<Self> {
        let uri: axum::http::Uri = origin.parse()?;
        anyhow::ensure!(
            uri.scheme_str() == Some("https")
                || (uri.scheme_str() == Some("http")
                    && matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]"))),
            "DEN_ORIGIN must use HTTPS except on loopback"
        );
        anyhow::ensure!(
            uri.path() == "/" && uri.query().is_none() && !origin.ends_with('/'),
            "DEN_ORIGIN must be an origin without a trailing slash"
        );
        anyhow::ensure!(max_upload > 0, "DEN_MAX_UPLOAD_BYTES must be positive");
        if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::create_dir_all(&uploads).await?;
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let db = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        // SQLite cannot alter a CHECK constraint. Rebuild parent tables without
        // cascading into their children, on one connection before serving requests.
        let mut migration = db.acquire().await?;
        sqlx::query("PRAGMA foreign_keys=OFF")
            .execute(&mut *migration)
            .await?;
        sqlx::migrate!().run(&mut *migration).await?;
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut *migration)
            .await?;
        anyhow::ensure!(violations.is_empty(), "Migration left invalid foreign keys");
        sqlx::query("PRAGMA foreign_keys=ON")
            .execute(&mut *migration)
            .await?;
        drop(migration);
        appearance::complete_theme_pairs(&db).await?;
        if sqlx::query_scalar!("SELECT count(*) FROM users")
            .fetch_one(&db)
            .await?
            == 0
        {
            if let Some(parent) = bootstrap.parent().filter(|p| !p.as_os_str().is_empty()) {
                tokio::fs::create_dir_all(parent).await?;
            }
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&bootstrap) {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(auth::secret().as_bytes())?;
                    file.sync_all()?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        let (events, _) = broadcast::channel(256);
        let state = Self(Arc::new(Inner {
            db,
            hosts: hosts::Hosts::default(),
            livekit: None,
            calls: Mutex::new(HashMap::new()),
            uploads,
            bootstrap,
            origin,
            max_upload,
            events,
            writes: Mutex::new(()),
            attempts: Mutex::new(HashMap::new()),
            tickets: Mutex::new(tickets::Tickets::default()),
            presence: std::sync::Mutex::new(HashMap::new()),
            object_presence: std::sync::Mutex::new(HashMap::new()),
            thumbnails: Arc::new(tokio::sync::Semaphore::new(1)),
            ids: std::sync::Mutex::new(ulid::Generator::new()),
        }));
        activity::backfill(&state)
            .await
            .map_err(|e| anyhow::anyhow!("Mention indexing failed: {}", e.2))?;
        Ok(state)
    }
    pub fn with_livekit(mut self, url: String, key: String, secret: String) -> Self {
        if !url.is_empty() && !key.is_empty() && !secret.is_empty() {
            Arc::get_mut(&mut self.0)
                .expect("configure before sharing")
                .livekit = Some(calls::LiveKit { url, key, secret });
        }
        self
    }
    pub(crate) fn id(&self) -> String {
        self.ids
            .lock()
            .expect("id mutex poisoned")
            .generate()
            .expect("ULID exhausted")
            .to_string()
    }
    pub async fn cleanup(&self) -> anyhow::Result<()> {
        uploads::cleanup(self).await
    }
}

pub fn router(state: AppState) -> Router {
    router_with_web(state, PathBuf::from("apps/web/dist"))
}
pub fn router_with_web(state: AppState, web_dir: PathBuf) -> Router {
    let settings_web = web_dir.clone();
    Router::new()
        .route("/instance", get(objects::instance))
        .route("/auth/ws-ticket", post(tickets::issue))
        .route("/hosts", get(hosts::list))
        .route("/hosts/enroll", post(hosts::enroll))
        .route("/hosts/login", post(hosts::login))
        .route("/hosts/ws", get(hosts::connect))
        .route("/hosts/direct/check", post(hosts::direct_check))
        .route("/hosts/{id}", delete(hosts::remove))
        .route("/hosts/{id}/requests", post(access::request))
        .route("/hosts/{id}/sessions", post(terminal::open))
        .route("/requests/{id}/decide", post(access::decide))
        .route("/grants", get(access::grants))
        .route("/grants/{id}", delete(access::revoke))
        .route("/access/log", get(access::log))
        .route("/sessions/{id}/controller", post(terminal::controller))
        .route(
            "/sessions/{id}/request-control",
            post(terminal::request_control),
        )
        .route("/sessions/{id}/share", post(terminal::share))
        .route("/sessions/{id}/write", post(terminal::write))
        .route("/sessions/{id}/direct-token", post(terminal::direct_token))
        .route("/sessions/{id}", delete(terminal::close))
        .route(
            "/settings",
            get(objects::settings).put(objects::save_settings),
        )
        .route("/channels/{id}/objects", post(objects::create))
        .route("/objects/{id}", get(objects::get).patch(objects::update))
        .route("/objects/{id}/summary", get(objects::summary))
        .route(
            "/objects/{id}/patch",
            post(objects::patch).layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)),
        )
        .route("/health", get(health))
        .route("/openapi.json", get(openapi::serve))
        .route("/auth/init", post(auth::bootstrap))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/users", get(auth::users))
        .route("/users/me", get(auth::me))
        .route(
            "/users/me/appearance",
            get(appearance::get_appearance).put(appearance::put_appearance),
        )
        .route("/invites", post(credentials::invite))
        .route("/invites/{id}", delete(credentials::revoke_invite))
        .route(
            "/tokens",
            get(credentials::tokens).post(credentials::create_token),
        )
        .route("/tokens/{id}", delete(credentials::revoke_token))
        .route("/bots", post(credentials::bot))
        .route(
            "/categories",
            get(chat::categories).post(chat::create_category),
        )
        .route(
            "/categories/{id}",
            axum::routing::put(chat::update_category).delete(chat::delete_category),
        )
        .route("/channels", get(chat::channels).post(chat::create_channel))
        .route(
            "/channels/{id}",
            get(chat::channel)
                .put(chat::update_channel)
                .delete(chat::delete_channel),
        )
        .route("/dms", post(chat::dm))
        .route(
            "/channels/{id}/messages",
            get(messages::messages).post(messages::send),
        )
        .route(
            "/messages/{id}",
            get(activity::message)
                .patch(messages::edit)
                .delete(messages::remove),
        )
        .route(
            "/messages/{id}/reactions",
            axum::routing::put(activity::react).delete(activity::unreact),
        )
        .route("/channels/{id}/read", axum::routing::put(inbox::mark_read))
        .route("/users/me/read-state", get(inbox::read_states))
        .route(
            "/users/me/notification-preferences",
            get(inbox::get_preferences).put(inbox::put_preferences),
        )
        .route("/search/messages", get(activity::search))
        .route("/presence", get(ws::presence))
        .route("/calls", get(calls::list))
        .route("/calls/{channel_id}/token", post(calls::token))
        .route("/livekit/webhook", post(calls::webhook))
        .route(
            "/uploads/{id}/thumbnail",
            get(thumbnails::serve).head(thumbnails::serve),
        )
        .route("/uploads", post(uploads::begin))
        .route("/uploads/{id}", get(uploads::status).patch(uploads::chunk))
        .route("/uploads/{id}/complete", post(uploads::complete))
        .route("/uploads/{id}/file", get(uploads::file).head(uploads::file))
        .route("/ws", get(ws::connect))
        .fallback(move |req: Request| web::serve(web_dir.clone(), req))
        .layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(axum::middleware::map_response(
            |mut response: Response| async move {
                response.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    "no-store".parse().unwrap(),
                );
                response
            },
        ))
        // /settings is both an API resource and an existing SPA route.
        .layer(axum::middleware::from_fn(
            move |req: Request, next: axum::middleware::Next| {
                let root = settings_web.clone();
                async move {
                    let settings = req.uri().path() == "/settings";
                    let mut response = if settings
                        && matches!(
                            *req.method(),
                            axum::http::Method::GET | axum::http::Method::HEAD
                        )
                        && req
                            .headers()
                            .get(axum::http::header::ACCEPT)
                            .and_then(|v| v.to_str().ok())
                            .is_some_and(|v| v.contains("text/html"))
                    {
                        web::serve(root, req).await
                    } else {
                        next.run(req).await
                    };
                    if settings {
                        response
                            .headers_mut()
                            .insert(axum::http::header::VARY, "Accept".parse().unwrap());
                        response.headers_mut().insert(
                            axum::http::header::CACHE_CONTROL,
                            "no-store".parse().unwrap(),
                        );
                    }
                    response
                }
            },
        ))
        .with_state(state)
}
#[utoipa::path(get, path="/health", responses((status=200, body=Health)))]
async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs() as i64
}
pub(crate) type Result<T> = std::result::Result<T, Error>;
pub(crate) struct Error(pub StatusCode, pub &'static str, pub String);
impl Error {
    pub fn bad(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, "invalid_request", message.into())
    }
    pub fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Authentication required".into(),
        )
    }
    pub fn forbidden() -> Self {
        Self(StatusCode::FORBIDDEN, "forbidden", "Access denied".into())
    }
    pub fn missing() -> Self {
        Self(StatusCode::NOT_FOUND, "not_found", "Not found".into())
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self(StatusCode::CONFLICT, "conflict", message.into())
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(ApiError {
                error: self.1.into(),
                message: self.2,
            }),
        )
            .into_response()
    }
}
impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        if matches!(e, sqlx::Error::RowNotFound) {
            return Self::missing();
        }
        if let sqlx::Error::Database(ref db) = e {
            if db.is_unique_violation() {
                return Self::conflict("Already exists");
            }
            if db.is_foreign_key_violation() {
                return Self::bad("Referenced item does not exist");
            }
        }
        tracing::error!(error=%e, "database operation failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Database operation failed".into(),
        )
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        tracing::error!(error=%e, "file operation failed");
        Self(
            StatusCode::INSUFFICIENT_STORAGE,
            "storage",
            "File operation failed; check server disk space".into(),
        )
    }
}
pub(crate) struct ApiJson<T>(pub T);
impl<T: DeserializeOwned + Send> FromRequest<AppState> for ApiJson<T> {
    type Rejection = Error;
    async fn from_request(req: Request, state: &AppState) -> Result<Self> {
        let patch =
            req.uri().path().starts_with("/objects/") && req.uri().path().ends_with("/patch");
        Json::<T>::from_request(req, state)
            .await
            .map(|v| Self(v.0))
            .map_err(|e| {
                if patch && e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    Error(
                        e.status(),
                        "patch_too_large",
                        "Object patch exceeds 1 MiB".into(),
                    )
                } else {
                    Error(e.status(), "invalid_request", "Invalid JSON request".into())
                }
            })
    }
}
pub(crate) fn name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 100 {
        Err(Error::bad("Name must contain 1 to 100 bytes"))
    } else {
        Ok(())
    }
}
