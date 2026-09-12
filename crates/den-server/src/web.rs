use crate::*;
use axum::{body::Body, http::Method};
use tower::ServiceExt;

pub(crate) async fn serve(root: PathBuf, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    // Misspelled API URLs and missing assets must never return index.html.
    let segment = path.split('/').nth(1).unwrap_or("");
    if matches!(
        segment,
        "auth"
            | "users"
            | "invites"
            | "tokens"
            | "bots"
            | "categories"
            | "channels"
            | "dms"
            | "messages"
            | "uploads"
            | "ws"
            | "presence"
            | "calls"
            | "livekit"
            | "search"
            | "health"
            | "openapi.json"
            | "api"
    ) {
        return Error::missing().into_response();
    }
    if !matches!(method, Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let response = tower_http::services::ServeDir::new(&root)
        .oneshot(req)
        .await
        .unwrap_or_else(|never| match never {})
        .map(Body::new);
    if response.status() != StatusCode::NOT_FOUND
        || segment == "assets"
        || path.rsplit('/').next().unwrap_or("").contains('.')
    {
        return response;
    }
    let req = Request::builder()
        .method(method)
        .uri("/index.html")
        .body(Body::empty())
        .expect("static request");
    tower_http::services::ServeFile::new(root.join("index.html"))
        .oneshot(req)
        .await
        .unwrap_or_else(|never| match never {})
        .map(Body::new)
}
