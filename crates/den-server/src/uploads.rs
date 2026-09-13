use crate::{auth::Auth, chat::visible, *};
use axum::{
    body::{Body, Bytes},
    extract::Path,
    http::{header, HeaderMap},
    response::Response,
};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tower::ServiceExt;

pub(crate) async fn load(s: &AppState, id: &str) -> Result<Upload> {
    Ok(sqlx::query_as!(Upload,"SELECT id,channel_id,filename,content_type,size,offset,complete as \"complete: bool\", CASE WHEN thumbnail_ready=1 THEN '/uploads/'||id||'/thumbnail' END as \"thumbnail_url?: String\" FROM uploads WHERE id=?",id).fetch_one(&s.db).await?)
}
async fn owned(s: &AppState, a: &Auth, id: &str) -> Result<Upload> {
    let row = load(s, id).await?;
    visible(s, &a.user.id, &row.channel_id).await?;
    let owner = sqlx::query_scalar!("SELECT owner_id FROM uploads WHERE id=?", id)
        .fetch_one(&s.db)
        .await?;
    if owner != a.user.id {
        return Err(Error::forbidden());
    }
    Ok(row)
}
fn media_type(value: &str) -> &str {
    match value {
        "video/mp4" | "video/webm" | "video/quicktime" | "audio/mpeg" | "audio/mp4"
        | "audio/ogg" | "audio/wav" | "image/png" | "image/jpeg" | "image/gif" | "image/webp" => {
            value
        }
        _ => "application/octet-stream",
    }
}
#[utoipa::path(post,path="/uploads",request_body=BeginUpload,responses((status=200,body=Upload),(status=413,body=ApiError)))]
pub(crate) async fn begin(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<BeginUpload>,
) -> Result<Json<Upload>> {
    visible(&s, &a.user.id, &v.channel_id).await?;
    if v.size <= 0 || v.size > s.max_upload {
        return Err(Error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "upload_size",
            format!("Size must be 1-{} bytes", s.max_upload),
        ));
    }
    if v.filename.is_empty()
        || v.filename.len() > 255
        || v.filename
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(Error::bad("Invalid filename"));
    }
    let _guard = s.writes.lock().await;
    let pending = sqlx::query_scalar!(
        "SELECT count(*) FROM uploads WHERE owner_id=? AND complete=0",
        a.user.id
    )
    .fetch_one(&s.db)
    .await?;
    if pending >= 5 {
        return Err(Error::conflict(
            "Finish pending uploads first; maximum five",
        ));
    }
    let reserved = sqlx::query_scalar!(
        "SELECT coalesce(sum(size-offset),0) as \"reserved!: i64\" FROM uploads WHERE complete=0"
    )
    .fetch_one(&s.db)
    .await?;
    space(&s, reserved.saturating_add(v.size) as u64)?;
    let id = s.id();
    let mime = media_type(&v.content_type);
    let time = now();
    let path = s.uploads.join(format!("{id}.part"));
    tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await?;
    if let Err(e)=sqlx::query!("INSERT INTO uploads(id,channel_id,owner_id,filename,content_type,size,touched_at) VALUES(?,?,?,?,?,?,?)",id,v.channel_id,a.user.id,v.filename,mime,v.size,time).execute(&s.db).await {let _=tokio::fs::remove_file(path).await;return Err(e.into());}
    Ok(Json(load(&s, &id).await?))
}
#[utoipa::path(get,path="/uploads/{id}",params(("id"=String,Path)),responses((status=200,body=Upload)))]
pub(crate) async fn status(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<Upload>> {
    Ok(Json(owned(&s, &a, &id).await?))
}
#[utoipa::path(patch,path="/uploads/{id}",params(("id"=String,Path),("Upload-Offset"=i64,Header,description="Current server offset; fetch status after interrupted requests")),request_body(content=String,content_type="application/octet-stream"),responses((status=200,body=Upload),(status=409,body=ApiError)))]
pub(crate) async fn chunk(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: std::result::Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Result<Json<Upload>> {
    let body = body.map_err(|e| {
        Error(
            e.status(),
            "invalid_request",
            "Chunk exceeds request body limit".into(),
        )
    })?;
    let offset = headers
        .get("upload-offset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Error::bad("Upload-Offset header required"))?;
    let _guard = s.writes.lock().await;
    let row = owned(&s, &a, &id).await?;
    if row.complete || row.offset != offset {
        return Err(Error::conflict(
            "Offset changed or upload complete; fetch upload status",
        ));
    }
    if body.is_empty() || body.len() > 8 * 1024 * 1024 || body.len() as i64 > row.size - row.offset
    {
        return Err(Error::bad(
            "Chunk must be 1-8388608 bytes and fit remaining size",
        ));
    }
    space(&s, body.len() as u64)?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(s.uploads.join(format!("{id}.part")))
        .await?;
    // A crash may leave bytes beyond the committed offset. Retries replace them.
    file.set_len(offset as u64).await?;
    file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
    file.write_all(&body).await?;
    file.sync_data().await?;
    let next = offset + body.len() as i64;
    let time = now();
    sqlx::query!(
        "UPDATE uploads SET offset=?,touched_at=? WHERE id=?",
        next,
        time,
        id
    )
    .execute(&s.db)
    .await?;
    Ok(Json(load(&s, &id).await?))
}
#[utoipa::path(post,path="/uploads/{id}/complete",params(("id"=String,Path)),responses((status=200,body=Upload)))]
pub(crate) async fn complete(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<Upload>> {
    let _guard = s.writes.lock().await;
    let row = owned(&s, &a, &id).await?;
    if row.complete {
        drop(_guard);
        thumbnails::generate(&s, &row).await?;
        return Ok(Json(load(&s, &id).await?));
    }
    if row.offset != row.size {
        return Err(Error::conflict("Upload is incomplete"));
    }
    let final_path = s.uploads.join(&id);
    let part = s.uploads.join(format!("{id}.part"));
    // If a crash followed rename, retry only needs to commit the DB state.
    if !tokio::fs::try_exists(&final_path).await? {
        tokio::fs::rename(part, &final_path).await?;
    }
    if tokio::fs::metadata(&final_path).await?.len() != row.size as u64 {
        return Err(Error::conflict("Stored size mismatch"));
    }
    // Persist the directory entry before announcing a complete upload.
    let directory = s.uploads.clone();
    tokio::task::spawn_blocking(move || std::fs::File::open(directory)?.sync_all())
        .await
        .map_err(|_| Error::conflict("Finalize interrupted; retry"))??;
    let time = now();
    sqlx::query!(
        "UPDATE uploads SET complete=1,touched_at=? WHERE id=?",
        time,
        id
    )
    .execute(&s.db)
    .await?;
    drop(_guard);
    let row = load(&s, &id).await?;
    thumbnails::generate(&s, &row).await?;
    Ok(Json(load(&s, &id).await?))
}
#[utoipa::path(get,path="/uploads/{id}/file",params(("id"=String,Path),("Range"=Option<String>,Header)),responses((status=200,description="File bytes"),(status=206,description="Requested byte range"),(status=416,description="Unsatisfiable range")))]
pub(crate) async fn file(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    req: Request,
) -> Result<Response> {
    let row = load(&s, &id).await?;
    let recording = sqlx::query_scalar::<_, String>(
        "SELECT session_id FROM terminal_recordings WHERE upload_id=?",
    )
    .bind(&id)
    .fetch_optional(&s.db)
    .await?;
    if let Some(session) = recording {
        if !terminal::can_view(&s, &a.user.id, &session).await {
            return Err(Error::missing());
        }
    } else {
        visible(&s, &a.user.id, &row.channel_id).await?;
    }
    if !row.complete {
        return Err(Error::missing());
    }
    let response = tower_http::services::ServeFile::new(s.uploads.join(&row.id))
        .oneshot(req)
        .await
        .unwrap_or_else(|never| match never {});
    let mut response = response.map(Body::new);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        row.content_type
            .parse()
            .map_err(|_| Error::bad("Invalid media type"))?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    let disposition = if row.content_type == "application/octet-stream" {
        "attachment"
    } else {
        "inline"
    };
    response
        .headers_mut()
        .insert(header::CONTENT_DISPOSITION, disposition.parse().unwrap());
    Ok(response)
}
pub(crate) async fn cleanup(s: &AppState) -> anyhow::Result<()> {
    let _guard = s.writes.lock().await;
    let cutoff = now() - 86400;
    let stale = sqlx::query!(
        "SELECT id FROM uploads WHERE complete=0 AND touched_at<?",
        cutoff
    )
    .fetch_all(&s.db)
    .await?;
    for row in stale {
        for path in [
            s.uploads.join(&row.id),
            s.uploads.join(format!("{}.part", row.id)),
        ] {
            if let Err(e) = tokio::fs::remove_file(path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e.into());
                }
            }
        }
        sqlx::query!("DELETE FROM uploads WHERE id=?", row.id)
            .execute(&s.db)
            .await?;
    }
    Ok(())
}

// Keep space for SQLite/WAL while accounting for accepted unfinished uploads.
fn space(s: &AppState, bytes: u64) -> Result<()> {
    if fs2::available_space(&s.uploads)? < bytes.saturating_add(64 * 1024 * 1024) {
        return Err(Error(
            StatusCode::INSUFFICIENT_STORAGE,
            "storage",
            "Not enough free disk space for this upload".into(),
        ));
    }
    Ok(())
}
