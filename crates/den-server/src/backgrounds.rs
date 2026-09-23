// Account wallpapers. Each person keeps a small library of their uploads, so a
// picture used last month is still one click away. Files live at
// `uploads/backgrounds/<user>/<sha256>` with a JPEG preview beside each; the
// content hash is the ID, so uploading the same picture twice stores it once.
use crate::{auth::Auth, *};
use axum::{
    body::to_bytes,
    extract::Path,
    http::{header, HeaderMap},
};
use sha2::{Digest, Sha256};
use std::path::{Path as FsPath, PathBuf};
use tokio::io::AsyncWriteExt;

/// Clients shrink photos before uploading; this bounds what arrives anyway.
const MAX_IMAGE: usize = 16 * 1024 * 1024;
/// Uploads kept per person. The oldest go first, never the one in use.
const LIBRARY: usize = 24;
const BOUNDS: thumbnails::Bounds = thumbnails::Bounds {
    side: 12_000,
    pixels: 40_000_000,
    alloc: 256 * 1024 * 1024,
};
const PREVIEW: u32 = 480;

fn content_id(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn is_id(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn folder(s: &AppState, user: &str) -> PathBuf {
    s.uploads.join("backgrounds").join(user)
}

async fn write_atomic(dest: &FsPath, bytes: &[u8]) -> std::io::Result<()> {
    let temp = dest.with_extension("part");
    let written = async {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temp).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temp, dest).await
    }
    .await;
    if written.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }
    written
}

fn mime(format: image::ImageFormat) -> &'static str {
    match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => "image/gif",
    }
}

/// Check an upload and render its preview. Every refusal says what to do next.
fn process(bytes: &[u8], format: image::ImageFormat) -> Result<(u32, u32, Vec<u8>)> {
    if image::guess_format(bytes).ok() != Some(format) {
        return Err(Error::bad(
            "That file isn't a readable PNG, JPEG, WebP or GIF image. Try exporting it again.",
        ));
    }
    let open = || image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let (width, height) = open().into_dimensions().map_err(|_| {
        Error::bad("That image looks damaged and couldn't be opened. Try exporting it again.")
    })?;
    if width > BOUNDS.side
        || height > BOUNDS.side
        || u64::from(width) * u64::from(height) > BOUNDS.pixels
    {
        return Err(Error::bad(format!(
            "That image is {width}×{height} pixels. Wallpapers can be up to 40 megapixels and 12,000 pixels on a side, such as 7680×5120."
        )));
    }
    let decoded = thumbnails::decode_within(open(), BOUNDS).map_err(|_| {
        Error::bad("That image needs too much memory to open, usually because it uses 16-bit color. Save it as a JPEG or an 8-bit PNG.")
    })?;
    let mut preview = Vec::new();
    image::DynamicImage::ImageRgb8(decoded.thumbnail(PREVIEW, PREVIEW).to_rgb8())
        .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut preview,
            80,
        ))
        .map_err(|_| Error::conflict("Could not make a preview"))?;
    Ok((decoded.width(), decoded.height(), preview))
}

async fn describe(path: &FsPath, id: &str) -> Option<BackgroundImage> {
    let meta = tokio::fs::metadata(path).await.ok()?;
    let path = path.to_path_buf();
    let (format, (width, height)) = tokio::task::spawn_blocking(move || {
        let reader = image::ImageReader::open(&path)
            .ok()?
            .with_guessed_format()
            .ok()?;
        let format = reader.format()?;
        Some((format, reader.into_dimensions().ok()?))
    })
    .await
    .ok()??;
    let uploaded_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    Some(BackgroundImage {
        id: id.to_string(),
        content_type: mime(format).into(),
        size: meta.len(),
        width,
        height,
        uploaded_at,
    })
}

/// Everything someone has uploaded, newest first.
async fn library(s: &AppState, user: &str) -> Result<Vec<BackgroundImage>> {
    let dir = folder(s, user);
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut images = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_id(&name) {
            if let Some(image) = describe(&entry.path(), &name).await {
                images.push(image);
            }
        }
    }
    images.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at).then(a.id.cmp(&b.id)));
    Ok(images)
}

fn upload_id(background: &Option<Background>) -> Option<&str> {
    match background {
        Some(Background {
            source: BackgroundSource::Upload { id },
            ..
        }) => Some(id),
        _ => None,
    }
}
fn selected(value: &Appearance) -> Option<&str> {
    upload_id(&value.background)
}
/// Library images in use by either wallpaper, which pruning must keep.
fn in_use(value: &Appearance) -> Vec<String> {
    [
        upload_id(&value.background),
        upload_id(&value.sidebar_background),
    ]
    .into_iter()
    .flatten()
    .map(str::to_string)
    .collect()
}

// IDs are checked against the owner's own library, never used as paths unvalidated.
pub(crate) async fn resolve(s: &AppState, user: &str, value: &mut Appearance) -> Result<bool> {
    let mut missing = false;
    for slot in [&mut value.background, &mut value.sidebar_background] {
        if let Some(id) = upload_id(slot) {
            if !is_id(id) || tokio::fs::metadata(folder(s, user).join(id)).await.is_err() {
                *slot = None;
                missing = true;
            }
        }
    }
    Ok(missing)
}

async fn remove_image(s: &AppState, user: &str, id: &str) -> Result<()> {
    let dir = folder(s, user);
    for name in [id.to_string(), format!("{id}.jpg")] {
        match tokio::fs::remove_file(dir.join(name)).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[utoipa::path(put, path="/users/me/background/image",
    request_body(content((Vec<u8> = "image/png"), (Vec<u8> = "image/jpeg"), (Vec<u8> = "image/webp"), (Vec<u8> = "image/gif"))),
    responses((status=200,body=BackgroundImage),(status=400,body=ApiError),(status=413,body=ApiError),(status=415,body=ApiError)))]
pub(crate) async fn put(
    State(s): State<AppState>,
    a: Auth,
    req: Request,
) -> Result<Json<BackgroundImage>> {
    let format = match req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
    {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        "image/gif" => image::ImageFormat::Gif,
        _ => {
            return Err(Error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "invalid_image",
                "Use a PNG, JPEG, WebP or GIF image.".into(),
            ))
        }
    };
    let bytes = to_bytes(req.into_body(), MAX_IMAGE).await.map_err(|_| {
        Error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "image_too_large",
            "Wallpapers can be up to 16 MB. Save a smaller copy, or pick a JPEG instead.".into(),
        )
    })?;
    let permit = s
        .thumbnails
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::conflict("Image worker unavailable"))?;
    let (bytes, width, height, preview) = tokio::task::spawn_blocking(move || -> Result<_> {
        let _permit = permit;
        let (width, height, preview) = process(&bytes, format)?;
        Ok((bytes, width, height, preview))
    })
    .await
    .map_err(|_| Error::conflict("Image processing interrupted"))??;
    let id = content_id(&bytes);
    let _guard = s.writes.lock().await;
    let dir = folder(&s, &a.user.id);
    tokio::fs::create_dir_all(&dir).await?;
    write_atomic(&dir.join(format!("{id}.jpg")), &preview).await?;
    write_atomic(&dir.join(&id), &bytes).await?;
    // An older client replaces "the" image: keep that meaning by moving an
    // already-selected upload to this one.
    let mut value = appearance::load(&s, &a.user.id).await?;
    if let Some(Background {
        source: BackgroundSource::Upload { id: current },
        ..
    }) = &mut value.background
    {
        *current = id.clone();
        appearance::save(&s, &a.user.id, &value).await?;
    }
    let keep = in_use(&value);
    for old in library(&s, &a.user.id).await?.into_iter().skip(LIBRARY) {
        if !keep.contains(&old.id) && old.id != id {
            remove_image(&s, &a.user.id, &old.id).await?;
        }
    }
    Ok(Json(BackgroundImage {
        id,
        content_type: mime(format).into(),
        size: bytes.len() as u64,
        width,
        height,
        uploaded_at: now(),
    }))
}

async fn serve(path: PathBuf, headers: &HeaderMap) -> Result<Response> {
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::missing()),
        Err(e) => return Err(e.into()),
    };
    let format = image::guess_format(&bytes).map_err(|_| Error::missing())?;
    let etag = format!("\"{}\"", content_id(&bytes));
    let unchanged = headers
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

/// The image the old single-image API means: the selected upload, else the newest.
async fn current(s: &AppState, user: &str) -> Result<Option<String>> {
    let value = appearance::load(s, user).await?;
    if let Some(id) = selected(&value).filter(|id| is_id(id)) {
        return Ok(Some(id.to_string()));
    }
    Ok(library(s, user).await?.into_iter().next().map(|i| i.id))
}

#[utoipa::path(get,path="/users/me/background/image",responses(
    (status=200,description="The selected upload, or the newest; ETag and Cache-Control: private, max-age=3600",content((Vec<u8> = "image/png"), (Vec<u8> = "image/jpeg"), (Vec<u8> = "image/webp"), (Vec<u8> = "image/gif"))),
    (status=304,description="Matching ETag"),(status=404,body=ApiError)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    headers: HeaderMap,
) -> Result<Response> {
    let id = current(&s, &a.user.id).await?.ok_or_else(Error::missing)?;
    serve(folder(&s, &a.user.id).join(id), &headers).await
}

#[utoipa::path(delete,path="/users/me/background/image",responses((status=204,description="Image removed")))]
pub(crate) async fn remove(State(s): State<AppState>, a: Auth) -> Result<StatusCode> {
    let _guard = s.writes.lock().await;
    if let Some(id) = current(&s, &a.user.id).await? {
        forget(&s, &a.user.id, &id).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn forget(s: &AppState, user: &str, id: &str) -> Result<()> {
    remove_image(s, user, id).await?;
    let mut value = appearance::load(s, user).await?;
    let mut changed = false;
    for slot in [&mut value.background, &mut value.sidebar_background] {
        if upload_id(slot) == Some(id) {
            *slot = None;
            changed = true;
        }
    }
    if changed {
        appearance::save(s, user, &value).await?;
    }
    Ok(())
}

#[utoipa::path(get,path="/users/me/backgrounds",responses((status=200,body=[BackgroundImage],description="Your uploads, newest first")))]
pub(crate) async fn list(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<BackgroundImage>>> {
    Ok(Json(library(&s, &a.user.id).await?))
}

#[utoipa::path(get,path="/users/me/backgrounds/{id}",params(("id"=String,Path)),responses((status=200,description="The original image"),(status=404,body=ApiError)))]
pub(crate) async fn image(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    if !is_id(&id) {
        return Err(Error::missing());
    }
    serve(folder(&s, &a.user.id).join(id), &headers).await
}

#[utoipa::path(get,path="/users/me/backgrounds/{id}/preview",params(("id"=String,Path)),responses((status=200,description="A small JPEG for the gallery"),(status=404,body=ApiError)))]
pub(crate) async fn preview(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    if !is_id(&id) {
        return Err(Error::missing());
    }
    serve(folder(&s, &a.user.id).join(format!("{id}.jpg")), &headers).await
}

#[utoipa::path(delete,path="/users/me/backgrounds/{id}",params(("id"=String,Path)),responses((status=204,description="Removed; unselected if it was in use"),(status=404,body=ApiError)))]
pub(crate) async fn delete(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    if !is_id(&id) {
        return Err(Error::missing());
    }
    let _guard = s.writes.lock().await;
    if tokio::fs::metadata(folder(&s, &a.user.id).join(&id))
        .await
        .is_err()
    {
        return Err(Error::missing());
    }
    forget(&s, &a.user.id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Before the library, each person had one file at `backgrounds/<user>`. Move it
/// into their folder so it shows up in the gallery and stays selected.
pub(crate) async fn migrate(s: &AppState) -> anyhow::Result<()> {
    s.migrate_backgrounds().await
}
impl AppState {
    #[doc(hidden)]
    pub async fn migrate_backgrounds(&self) -> anyhow::Result<()> {
        let s = self;
        let root = s.uploads.join("backgrounds");
        let mut entries = match tokio::fs::read_dir(&root).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let user = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type().await?.is_file() || user.parse::<ulid::Ulid>().is_err() {
                continue;
            }
            let bytes = tokio::fs::read(entry.path()).await?;
            let id = content_id(&bytes);
            let legacy = root.join(format!("{user}.legacy"));
            tokio::fs::rename(entry.path(), &legacy).await?;
            let dir = root.join(&user);
            tokio::fs::create_dir_all(&dir).await?;
            let format = image::guess_format(&bytes).ok();
            let preview = tokio::task::spawn_blocking(move || {
                format
                    .and_then(|f| process(&bytes, f).ok())
                    .map(|p| (p.2, bytes))
            })
            .await?;
            match preview {
                Some((preview, bytes)) => {
                    write_atomic(&dir.join(format!("{id}.jpg")), &preview).await?;
                    write_atomic(&dir.join(&id), &bytes).await?;
                    tokio::fs::remove_file(&legacy).await?;
                }
                // Unreadable leftovers stay aside rather than being deleted.
                None => tracing::warn!(%user, "Legacy background could not be migrated"),
            }
        }
        Ok(())
    }
}
