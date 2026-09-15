use crate::{auth::Auth, chat::visible, *};
use axum::{body::Body, extract::Path, http::header};
use tower::ServiceExt;

pub(crate) async fn generate(s: &AppState, row: &Upload) -> Result<bool> {
    if !row.content_type.starts_with("image/") || row.size > 32 * 1024 * 1024 {
        return Ok(false);
    }
    let _permit = s
        .thumbnails
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::conflict("Thumbnail worker unavailable"))?;
    let source = s.uploads.join(&row.id);
    let dest = s.uploads.join(format!("{}.thumb.png", row.id));
    if !tokio::fs::try_exists(&dest).await? {
        let output = dest.clone();
        let built = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let _permit = _permit;
            let reader = image::ImageReader::open(source)?.with_guessed_format()?;
            let decoded = decode(reader)?;
            let thumbnail = decoded.thumbnail(512, 512);
            let temp = output.with_extension("png.part");
            thumbnail.save_with_format(&temp, image::ImageFormat::Png)?;
            std::fs::File::open(&temp)?.sync_all()?;
            std::fs::rename(temp, output)?;
            Ok(())
        })
        .await
        .map_err(|_| Error::conflict("Thumbnail processing interrupted"))?;
        // Corrupt or oversized images remain downloadable, without a preview.
        if built.is_err() {
            return Ok(false);
        }
    }
    sqlx::query!("UPDATE uploads SET thumbnail_ready=1 WHERE id=?", row.id)
        .execute(&s.db)
        .await?;
    Ok(true)
}
#[utoipa::path(get,path="/uploads/{id}/thumbnail",params(("id"=String,Path)),responses((status=200,description="PNG thumbnail, at most 512 by 512"),(status=404,body=ApiError)))]
pub(crate) async fn serve(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    req: Request,
) -> Result<Response> {
    let row = uploads::load(&s, &id).await?;
    visible(&s, &a.user.id, &row.channel_id).await?;
    if !row.complete || !generate(&s, &row).await? {
        return Err(Error::missing());
    }
    let mut response =
        tower_http::services::ServeFile::new(s.uploads.join(format!("{}.thumb.png", row.id)))
            .oneshot(req)
            .await
            .unwrap_or_else(|never| match never {})
            .map(Body::new);
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    Ok(response)
}

// Shared by thumbnails and account backgrounds, including the decode allocation cap.
pub(crate) fn decode<R: std::io::BufRead + std::io::Seek>(
    mut reader: image::ImageReader<R>,
) -> anyhow::Result<image::DynamicImage> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    use image::ImageDecoder;
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    anyhow::ensure!(
        u64::from(width) * u64::from(height) <= 16_000_000,
        "Image exceeds pixel limit"
    );
    let orientation = decoder.orientation()?;
    let mut decoded = image::DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    anyhow::ensure!(
        u64::from(decoded.width()) * u64::from(decoded.height()) <= 16_000_000,
        "Image exceeds pixel limit"
    );
    Ok(decoded)
}
