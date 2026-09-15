use crate::{auth::Auth, *};
use axum::{body::to_bytes, http::header};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_IMAGE: usize = 8 * 1024 * 1024;

fn content_id(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn read(s: &AppState, user: &str) -> Result<Option<Vec<u8>>> {
    let file = match tokio::fs::File::open(s.uploads.join("backgrounds").join(user)).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_IMAGE + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > MAX_IMAGE {
        return Ok(None);
    }
    Ok(Some(bytes))
}

// IDs are compared only against the owner's file, never used as filesystem paths.
pub(crate) async fn resolve(s: &AppState, user: &str, value: &mut Appearance) -> Result<bool> {
    if let Some(Background {
        source: BackgroundSource::Upload { id },
        ..
    }) = &value.background
    {
        if read(s, user)
            .await?
            .is_none_or(|bytes| content_id(&bytes) != *id)
        {
            value.background = None;
            return Ok(true);
        }
    }
    Ok(false)
}

#[utoipa::path(put, path="/users/me/background/image",
    request_body(content((Vec<u8> = "image/png"), (Vec<u8> = "image/jpeg"), (Vec<u8> = "image/webp"), (Vec<u8> = "image/gif"))),
    responses((status=200,body=BackgroundImage),(status=400,body=ApiError),(status=413,body=ApiError),(status=415,body=ApiError)))]
pub(crate) async fn put(
    State(s): State<AppState>,
    a: Auth,
    req: Request,
) -> Result<Json<BackgroundImage>> {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let format = match content_type.as_str() {
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
    let bytes = to_bytes(req.into_body(), MAX_IMAGE).await.map_err(|_| {
        Error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "image_too_large",
            "Background images must be at most 8 MiB".into(),
        )
    })?;
    let permit = s
        .thumbnails
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::conflict("Image worker unavailable"))?;
    let (bytes, metadata) = tokio::task::spawn_blocking(move || -> Result<_> {
        let _permit = permit;
        if image::guess_format(&bytes).ok() != Some(format) {
            return Err(Error::bad("Image does not match its content type"));
        }
        let decoded = thumbnails::decode(image::ImageReader::with_format(
            std::io::Cursor::new(&bytes),
            format,
        ))
        .map_err(|_| Error::bad("Invalid image or image exceeds decoding limits"))?;
        let metadata = BackgroundImage {
            id: content_id(&bytes),
            content_type,
            size: bytes.len() as u64,
            width: decoded.width(),
            height: decoded.height(),
        };
        Ok((bytes, metadata))
    })
    .await
    .map_err(|_| Error::conflict("Image processing interrupted"))??;
    let _guard = s.writes.lock().await;
    let root = s.uploads.join("backgrounds");
    tokio::fs::create_dir_all(&root).await?;
    let dest = root.join(&a.user.id);
    let temp = dest.with_extension("part");
    let written = async {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temp).await?;
        file.write_all(&bytes).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temp, &dest).await
    }
    .await;
    if written.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }
    written?;
    let mut value = appearance::load(&s, &a.user.id).await?;
    if let Some(Background {
        source: BackgroundSource::Upload { id },
        ..
    }) = &mut value.background
    {
        *id = metadata.id.clone();
        appearance::save(&s, &a.user.id, &value).await?;
    }
    Ok(Json(metadata))
}

#[utoipa::path(get,path="/users/me/background/image",responses(
    (status=200,description="Owner's image; ETag and Cache-Control: private, max-age=3600",content((Vec<u8> = "image/png"), (Vec<u8> = "image/jpeg"), (Vec<u8> = "image/webp"), (Vec<u8> = "image/gif"))),
    (status=304,description="Matching ETag"),(status=404,body=ApiError)))]
pub(crate) async fn get(State(s): State<AppState>, a: Auth, req: Request) -> Result<Response> {
    let _guard = s.writes.lock().await;
    let bytes = read(&s, &a.user.id).await?.ok_or_else(Error::missing)?;
    let format = image::guess_format(&bytes).map_err(|_| Error::missing())?;
    let etag = format!("\"{}\"", content_id(&bytes));
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
    headers.insert(header::CONTENT_TYPE, format.to_mime_type().parse().unwrap());
    headers.insert(header::ETAG, etag.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "private, max-age=3600".parse().unwrap(),
    );
    // Prevent a shared URL from reusing a different account's cached response.
    headers.insert(header::VARY, "Authorization, Cookie".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    Ok(response)
}

#[utoipa::path(delete,path="/users/me/background/image",responses((status=204,description="Image removed")))]
pub(crate) async fn remove(State(s): State<AppState>, a: Auth) -> Result<StatusCode> {
    let _guard = s.writes.lock().await;
    match tokio::fs::remove_file(s.uploads.join("backgrounds").join(&a.user.id)).await {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    let mut value = appearance::load(&s, &a.user.id).await?;
    if matches!(
        value.background,
        Some(Background {
            source: BackgroundSource::Upload { .. },
            ..
        })
    ) {
        value.background = None;
        appearance::save(&s, &a.user.id, &value).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
