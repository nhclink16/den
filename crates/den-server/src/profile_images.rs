use crate::{auth::Auth, *};
use axum::{body::to_bytes, extract::Path, http::header};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy)]
enum Kind {
    Avatar,
    Banner,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Avatar => "avatar",
            Self::Banner => "banner",
        }
    }
    fn limit(self) -> usize {
        match self {
            Self::Avatar => 4 * 1024 * 1024,
            Self::Banner => 8 * 1024 * 1024,
        }
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
async fn read(path: &std::path::Path, limit: usize) -> Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::missing()
        } else {
            e.into()
        }
    })?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).await?;
    if bytes.len() > limit {
        return Err(Error::missing());
    }
    Ok(bytes)
}
async fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|v| v.to_str())
            .unwrap_or("original")
    ));
    let result = async {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temp).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temp, path).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(temp).await;
    }
    Ok(result?)
}
async fn set_url(s: &AppState, user: &str, kind: Kind, url: Option<String>) -> Result<User> {
    let sql = match kind {
        Kind::Avatar => "UPDATE users SET avatar_url=? WHERE id=?",
        Kind::Banner => "UPDATE users SET banner_url=? WHERE id=?",
    };
    sqlx::query(sql).bind(url).bind(user).execute(&s.db).await?;
    let user = auth::user(s, user).await?;
    profiles::broadcast(s, &user);
    Ok(user)
}
async fn put(s: AppState, user: String, kind: Kind, req: Request) -> Result<Json<User>> {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let format = match content_type {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        "image/gif" => image::ImageFormat::Gif,
        _ => {
            return Err(Error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "invalid_image",
                "Use PNG, JPEG, WebP or GIF".into(),
            ))
        }
    };
    let bytes = to_bytes(req.into_body(), kind.limit()).await.map_err(|_| {
        Error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "image_too_large",
            format!("{} image is too large", kind.name()),
        )
    })?;
    let permit = s
        .thumbnails
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::conflict("Image worker unavailable"))?;
    let (original, derivative) = tokio::task::spawn_blocking(move || -> Result<_> {
        let _permit = permit;
        if image::guess_format(&bytes).ok() != Some(format) {
            return Err(Error::bad("Image does not match its content type"));
        }
        let decoded = thumbnails::decode(image::ImageReader::with_format(
            std::io::Cursor::new(&bytes),
            format,
        ))
        .map_err(|_| Error::bad("Invalid image or image exceeds decoding limits"))?;
        if matches!(kind, Kind::Avatar)
            && (decoded.width() != decoded.height() || decoded.width() > 4096)
        {
            return Err(Error::bad(
                "Avatar must be square and at most 4096 pixels on a side",
            ));
        }
        // Keep GIFs byte-for-byte, including every animation frame and its timing.
        let derivative = if format == image::ImageFormat::Gif {
            None
        } else {
            let resized = match kind {
                Kind::Avatar => {
                    decoded.resize_exact(256, 256, image::imageops::FilterType::Triangle)
                }
                Kind::Banner => decoded.resize(1200, 8192, image::imageops::FilterType::Triangle),
            };
            let mut output = std::io::Cursor::new(Vec::new());
            resized
                .write_to(&mut output, image::ImageFormat::Png)
                .map_err(|_| Error::bad("Cannot create profile image preview"))?;
            Some(output.into_inner())
        };
        Ok((bytes, derivative))
    })
    .await
    .map_err(|_| Error::conflict("Image processing interrupted"))??;
    let _guard = s.writes.lock().await;
    let root = s.uploads.join("profiles").join(&user);
    tokio::fs::create_dir_all(&root).await?;
    let original_path = root.join(kind.name());
    let derivative_path = original_path.with_extension("png");
    if let Some(bytes) = &derivative {
        atomic_write(&derivative_path, bytes).await?;
    }
    atomic_write(&original_path, &original).await?;
    if derivative.is_none() {
        match tokio::fs::remove_file(&derivative_path).await {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    let id = hash(derivative.as_deref().unwrap_or(&original));
    let url = format!("/users/{user}/{}?v={id}", kind.name());
    Ok(Json(set_url(&s, &user, kind, Some(url)).await?))
}
async fn get(s: AppState, user: String, kind: Kind, req: Request) -> Result<Response> {
    let _guard = s.writes.lock().await;
    let record = auth::user(&s, &user).await?;
    if match kind {
        Kind::Avatar => record.avatar_url,
        Kind::Banner => record.banner_url,
    }
    .is_none()
    {
        return Err(Error::missing());
    }
    let original = s
        .uploads
        .join("profiles")
        .join(&record.id)
        .join(kind.name());
    let bytes = read(&original, kind.limit()).await?;
    let gif = image::guess_format(&bytes).ok() == Some(image::ImageFormat::Gif);
    let bytes = if gif {
        bytes
    } else {
        read(&original.with_extension("png"), 64 * 1024 * 1024).await?
    };
    let etag = format!("\"{}\"", hash(&bytes));
    let unchanged = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| {
            h.split(',')
                .map(str::trim)
                .any(|tag| tag == "*" || tag.trim_start_matches("W/") == etag)
        });
    let mut response = if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        bytes.into_response()
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        if gif { "image/gif" } else { "image/png" }.parse().unwrap(),
    );
    headers.insert(header::ETAG, etag.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "private, max-age=86400".parse().unwrap(),
    );
    headers.insert(header::VARY, "Authorization, Cookie".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    Ok(response)
}
async fn remove(s: AppState, user: String, kind: Kind) -> Result<Json<User>> {
    let _guard = s.writes.lock().await;
    let path = s.uploads.join("profiles").join(&user).join(kind.name());
    for file in [&path, &path.with_extension("png")] {
        match tokio::fs::remove_file(file).await {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(Json(set_url(&s, &user, kind, None).await?))
}

#[utoipa::path(put,path="/users/me/avatar",request_body(content((Vec<u8>="image/png"),(Vec<u8>="image/jpeg"),(Vec<u8>="image/webp"),(Vec<u8>="image/gif"))),responses((status=200,body=User),(status=400,body=ApiError),(status=413,body=ApiError)))]
pub(crate) async fn put_avatar(
    State(s): State<AppState>,
    a: Auth,
    req: Request,
) -> Result<Json<User>> {
    put(s, a.user.id, Kind::Avatar, req).await
}
#[utoipa::path(put,path="/users/me/banner",request_body(content((Vec<u8>="image/png"),(Vec<u8>="image/jpeg"),(Vec<u8>="image/webp"),(Vec<u8>="image/gif"))),responses((status=200,body=User),(status=400,body=ApiError),(status=413,body=ApiError)))]
pub(crate) async fn put_banner(
    State(s): State<AppState>,
    a: Auth,
    req: Request,
) -> Result<Json<User>> {
    put(s, a.user.id, Kind::Banner, req).await
}
/// Someone else's pictures: an admin tidying profiles, or the owner of an agent.
async fn editable(s: &AppState, a: &Auth, id: &str) -> Result<String> {
    let owner: Option<Option<String>> = sqlx::query_scalar("SELECT owner_id FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?;
    let Some(owner) = owner else {
        return Err(Error::missing());
    };
    if a.user.role == Role::Admin || owner.as_deref() == Some(&a.user.id) {
        Ok(id.to_string())
    } else {
        Err(Error::forbidden())
    }
}
#[utoipa::path(put,path="/users/{id}/avatar",params(("id"=String,Path)),request_body(content((Vec<u8>="image/png"),(Vec<u8>="image/jpeg"),(Vec<u8>="image/webp"),(Vec<u8>="image/gif"))),responses((status=200,body=User),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn put_user_avatar(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    req: Request,
) -> Result<Json<User>> {
    let id = editable(&s, &a, &id).await?;
    put(s, id, Kind::Avatar, req).await
}
#[utoipa::path(put,path="/users/{id}/banner",params(("id"=String,Path)),request_body(content((Vec<u8>="image/png"),(Vec<u8>="image/jpeg"),(Vec<u8>="image/webp"),(Vec<u8>="image/gif"))),responses((status=200,body=User),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn put_user_banner(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    req: Request,
) -> Result<Json<User>> {
    let id = editable(&s, &a, &id).await?;
    put(s, id, Kind::Banner, req).await
}
#[utoipa::path(delete,path="/users/{id}/avatar",params(("id"=String,Path)),responses((status=200,body=User),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn delete_user_avatar(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<User>> {
    let id = editable(&s, &a, &id).await?;
    remove(s, id, Kind::Avatar).await
}
#[utoipa::path(delete,path="/users/{id}/banner",params(("id"=String,Path)),responses((status=200,body=User),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn delete_user_banner(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<User>> {
    let id = editable(&s, &a, &id).await?;
    remove(s, id, Kind::Banner).await
}
#[utoipa::path(get,path="/users/{id}/avatar",params(("id"=String,Path)),responses((status=200,description="PNG derivative or original GIF; strong ETag and private, max-age=86400",content((Vec<u8>="image/png"),(Vec<u8>="image/gif"))),(status=304,description="Matching ETag"),(status=404,body=ApiError)))]
pub(crate) async fn get_avatar(
    State(s): State<AppState>,
    _a: Auth,
    Path(id): Path<String>,
    req: Request,
) -> Result<Response> {
    get(s, id, Kind::Avatar, req).await
}
#[utoipa::path(get,path="/users/{id}/banner",params(("id"=String,Path)),responses((status=200,description="PNG derivative or original GIF; strong ETag and private, max-age=86400",content((Vec<u8>="image/png"),(Vec<u8>="image/gif"))),(status=304,description="Matching ETag"),(status=404,body=ApiError)))]
pub(crate) async fn get_banner(
    State(s): State<AppState>,
    _a: Auth,
    Path(id): Path<String>,
    req: Request,
) -> Result<Response> {
    get(s, id, Kind::Banner, req).await
}
#[utoipa::path(delete,path="/users/me/avatar",responses((status=200,body=User)))]
pub(crate) async fn delete_avatar(State(s): State<AppState>, a: Auth) -> Result<Json<User>> {
    remove(s, a.user.id, Kind::Avatar).await
}
#[utoipa::path(delete,path="/users/me/banner",responses((status=200,body=User)))]
pub(crate) async fn delete_banner(State(s): State<AppState>, a: Auth) -> Result<Json<User>> {
    remove(s, a.user.id, Kind::Banner).await
}
