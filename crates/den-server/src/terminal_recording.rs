use crate::{auth::Auth, *};
use axum::extract::Path;

pub(crate) async fn preference(s: &AppState, user: &str, host: &str) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT enabled FROM terminal_recording_preferences WHERE user_id=? AND host_id=?",
    )
    .bind(user)
    .bind(host)
    .fetch_optional(&s.db)
    .await?
    .unwrap_or(false))
}
async fn remember(s: &AppState, user: &str, host: &str, enabled: bool) -> Result<()> {
    sqlx::query("INSERT INTO terminal_recording_preferences(user_id,host_id,enabled) VALUES(?,?,?) ON CONFLICT(user_id,host_id) DO UPDATE SET enabled=excluded.enabled")
        .bind(user).bind(host).bind(enabled).execute(&s.db).await?;
    Ok(())
}
async fn owner(s: &AppState, user: &str, host: &str) -> Result<()> {
    if hosts::load(s, host).await?.owner_id != user {
        return Err(Error::missing());
    }
    Ok(())
}
#[utoipa::path(get,path="/users/me/hosts/{id}/recording",params(("id"=String,Path)),responses((status=200,body=TerminalRecording)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<TerminalRecording>> {
    owner(&s, &a.user.id, &id).await?;
    Ok(Json(TerminalRecording {
        enabled: preference(&s, &a.user.id, &id).await?,
    }))
}
#[utoipa::path(put,path="/users/me/hosts/{id}/recording",params(("id"=String,Path)),request_body=TerminalRecording,responses((status=200,body=TerminalRecording)))]
pub(crate) async fn put(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<TerminalRecording>,
) -> Result<Json<TerminalRecording>> {
    let _g = s.writes.lock().await;
    owner(&s, &a.user.id, &id).await?;
    remember(&s, &a.user.id, &id, v.enabled).await?;
    Ok(Json(v))
}
#[utoipa::path(post,path="/sessions/{id}/recording",params(("id"=String,Path)),request_body=TerminalRecording,responses((status=200,body=TerminalState)))]
pub(crate) async fn set(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<TerminalRecording>,
) -> Result<Json<TerminalState>> {
    // Output, toggles, and session end share this lock so off cannot race a write.
    let _g = s.writes.lock().await;
    let mut t = terminal::load(&s, &id).await?;
    if t.owner_id != a.user.id {
        return Err(Error::missing());
    }
    if t.ended_at.is_some() {
        return Err(Error::conflict("Session ended"));
    }
    if !v.enabled {
        discard(&s, &id).await?;
        t.recording_capped = false;
    }
    t.recording_enabled = v.enabled;
    terminal::save(&s, &t).await?;
    remember(&s, &a.user.id, &t.host_id, v.enabled).await?;
    Ok(Json(t))
}
pub(crate) async fn discard(s: &AppState, id: &str) -> Result<()> {
    match tokio::fs::remove_file(s.uploads.join(format!("{id}.recording"))).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
