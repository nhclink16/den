//! Den server entry point. Milestone 0: boots, serves /health.

use axum::{routing::get, Json, Router};
use den_core::Health;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("den_server=info".parse()?))
        .init();

    let app = Router::new().route("/health", get(health));

    let addr: SocketAddr = std::env::var("DEN_BIND")
        .unwrap_or_else(|_| "127.0.0.1:7000".into())
        .parse()?;
    tracing::info!(%addr, "den-server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<Health> {
    Json(Health { ok: true, version: env!("CARGO_PKG_VERSION").into() })
}
