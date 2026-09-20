use crate::*;
use axum::{body::Body, http::Method};
use tower::ServiceExt;

pub(crate) async fn serve(root: PathBuf, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    // MediaPipe is large enough that re-downloading it for every preview is a
    // visible regression. Only cache URLs whose version changes with their bytes;
    // the global response layer keeps every unversioned static URL at no-store.
    let immutable_blur = path.starts_with("/blur/wasm-0.10.14/")
        || (path == "/blur/selfie_segmenter.tflite" && req.uri().query() == Some("v=191ac952"));
    if matches!(path.as_str(), "/install-host.sh" | "/install-host.ps1") {
        if !matches!(method, Method::GET | Method::HEAD) {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        let deploy = std::env::var_os("DEN_DEPLOY_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("deploy"));
        return tower_http::services::ServeFile::new_with_mime(
            deploy.join(&path[1..]),
            &"text/plain; charset=utf-8".parse().expect("static MIME"),
        )
        .oneshot(req)
        .await
        .unwrap_or_else(|never| match never {})
        .map(Body::new);
    }
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
            | "objects"
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
    let mut response = tower_http::services::ServeDir::new(&root)
        .oneshot(req)
        .await
        .unwrap_or_else(|never| match never {})
        .map(Body::new);
    if immutable_blur && response.status().is_success() {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            "public, max-age=31536000, immutable".parse().unwrap(),
        );
    }
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
