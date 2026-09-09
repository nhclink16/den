use den_server::{router, AppState};
use std::{net::SocketAddr, path::PathBuf};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("den_server=info".parse()?)).init();
    let addr: SocketAddr = std::env::var("DEN_BIND").unwrap_or_else(|_| "127.0.0.1:7000".into()).parse()?;
    let state = AppState::open(
        PathBuf::from(std::env::var("DEN_DB").unwrap_or_else(|_| "data/den.db".into())),
        PathBuf::from(std::env::var("DEN_UPLOADS").unwrap_or_else(|_| "data/uploads".into())),
        PathBuf::from(std::env::var("DEN_BOOTSTRAP_FILE").unwrap_or_else(|_| "data/bootstrap.key".into())),
        std::env::var("DEN_ORIGIN").unwrap_or_else(|_| format!("http://{addr}")),
        std::env::var("DEN_MAX_UPLOAD_BYTES").unwrap_or_else(|_| "1073741824".into()).parse()?,
    ).await?;
    state.cleanup().await?;
    let cleanup = state.clone();
    tokio::spawn(async move { let mut timer = tokio::time::interval(std::time::Duration::from_secs(3600)); loop { timer.tick().await; if let Err(e) = cleanup.cleanup().await { tracing::error!(error=%e, "upload cleanup failed"); } } });
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "den-server listening");
    axum::serve(listener, router(state).into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await?;
    Ok(())
}
