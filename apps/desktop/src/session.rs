use reqwest::{Client, Url};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

type Result<T> = std::result::Result<T, String>;
pub fn origin(value: &str) -> Result<Url> {
    let u = Url::parse(value).map_err(|_| "Invalid server URL")?;
    if !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || u.path() != "/"
        || !(u.scheme() == "https"
            || (u.scheme() == "http"
                && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
    {
        return Err("Use an HTTPS origin, or localhost for development".into());
    }
    Ok(u)
}
fn entry(server: &str) -> Result<keyring::Entry> {
    let u = origin(server)?;
    keyring::Entry::new("app.denchat.desktop", &u.origin().ascii_serialization())
        .map_err(|_| "OS keychain is unavailable".into())
}
#[tauri::command]
pub fn session_get(origin: String) -> Result<Option<String>> {
    match entry(&origin)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => {
            Err("Cannot read the OS keychain. Unlock your login keychain and try again.".into())
        }
    }
}
#[tauri::command]
pub fn session_set(origin: String, token: String) -> Result<()> {
    entry(&origin)?
        .set_password(&token)
        .map_err(|_| "Cannot save your session in the OS keychain".into())
}
#[tauri::command]
pub fn session_clear(origin: String) -> Result<()> {
    match entry(&origin)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Cannot remove your session from the OS keychain".into()),
    }
}
#[tauri::command]
pub fn instances_get(app: AppHandle) -> Result<Vec<String>> {
    let store = app.store("instances.json").map_err(|e| e.to_string())?;
    Ok(store
        .get("origins")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}
#[tauri::command]
pub fn instances_set(app: AppHandle, origins: Vec<String>) -> Result<()> {
    for o in &origins {
        origin(o)?;
    }
    let store = app.store("instances.json").map_err(|e| e.to_string())?;
    store.set("origins", serde_json::json!(origins));
    store.save().map_err(|e| e.to_string())
}
#[derive(Serialize)]
pub struct ApiResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

pub async fn request(
    client: &Client,
    server: &str,
    method: &str,
    path: &str,
    body: Option<Vec<u8>>,
    headers: HashMap<String, String>,
) -> Result<reqwest::Response> {
    let base = origin(server)?;
    if !path.starts_with('/') || path.starts_with("//") {
        return Err("Invalid API path".into());
    }
    let url = base.join(path).map_err(|_| "Invalid API path")?;
    if url.origin() != base.origin() {
        return Err("API request changed origin".into());
    }
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| "Invalid method")?;
    let mut req = client.request(method, url);
    if !matches!(path, "/auth/login" | "/auth/register" | "/instance") {
        if let Some(token) = session_get(server.into())? {
            req = req.bearer_auth(token);
        }
    }
    for (key, value) in headers {
        if matches!(
            key.to_ascii_lowercase().as_str(),
            "content-type" | "range" | "upload-offset" | "if-range"
        ) {
            req = req.header(key, value);
        }
    }
    if let Some(body) = body {
        req = req.body(body);
    }
    // Do not return reqwest errors containing credential-bearing query strings.
    req.send()
        .await
        .map_err(|_| "Cannot reach this server".into())
}
#[tauri::command]
pub async fn api_request(
    app: AppHandle,
    origin: String,
    method: String,
    path: String,
    body: Option<Vec<u8>>,
    headers: HashMap<String, String>,
) -> Result<ApiResponse> {
    let response = request(
        &app.state::<Client>(),
        &origin,
        &method,
        &path,
        body,
        headers,
    )
    .await?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "set-cookie" | "authorization"))
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
        .collect();
    let body = response
        .bytes()
        .await
        .map_err(|_| "Cannot read server response")?
        .to_vec();
    Ok(ApiResponse {
        status,
        headers,
        body,
    })
}

pub async fn media(
    app: AppHandle,
    req: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    async fn fetch(
        app: AppHandle,
        req: tauri::http::Request<Vec<u8>>,
    ) -> Result<tauri::http::Response<Vec<u8>>> {
        let uri = Url::parse(&req.uri().to_string()).map_err(|_| "Invalid media URL")?;
        let server = uri
            .query_pairs()
            .find(|(k, _)| k == "origin")
            .map(|(_, v)| v.into_owned())
            .ok_or("Missing origin")?;
        let path = uri.path();
        if !path.starts_with("/uploads/")
            || !(path.ends_with("/file") || path.ends_with("/thumbnail"))
        {
            return Err("Invalid media path".into());
        }
        let mut headers = HashMap::new();
        if let Some(range) = req.headers().get("range").and_then(|v| v.to_str().ok()) {
            headers.insert("range".into(), range.into());
        }
        let response = request(
            &app.state::<Client>(),
            &server,
            req.method().as_str(),
            path,
            None,
            headers,
        )
        .await?;
        let mut builder = tauri::http::Response::builder().status(response.status());
        for key in [
            "content-type",
            "content-length",
            "content-range",
            "accept-ranges",
            "content-disposition",
        ] {
            if let Some(v) = response.headers().get(key) {
                builder = builder.header(key, v);
            }
        }
        builder
            .header("access-control-allow-origin", "*")
            .header("cache-control", "no-store")
            .body(
                response
                    .bytes()
                    .await
                    .map_err(|_| "Cannot load attachment")?
                    .to_vec(),
            )
            .map_err(|_| "Invalid attachment response".into())
    }
    fetch(app, req).await.unwrap_or_else(|_| {
        tauri::http::Response::builder()
            .status(502)
            .body(Vec::new())
            .unwrap()
    })
}
