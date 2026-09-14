use crate::{auth::Auth, *};
use axum::extract::Path;

#[utoipa::path(post,path="/devices",request_body=RegisterDevice,responses((status=200,body=Device)),description="Register an iOS alert APNs token for the authenticated credential. DEN_APNS_ENV selects sandbox or production. Re-registration updates ownership; logout/revocation removes registrations bound to that credential.")]
pub(crate) async fn register(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<RegisterDevice>,
) -> Result<Json<Device>> {
    if v.token.is_empty()
        || v.token.len() > 512
        || v.token.len() % 2 != 0
        || !v.token.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(Error::bad(
            "Device token must be 1 to 256 bytes of hexadecimal",
        ));
    }
    if v.app_version.trim().is_empty()
        || v.app_version.len() > 64
        || v.app_version.chars().any(char::is_control)
    {
        return Err(Error::bad("App version must contain 1 to 64 bytes"));
    }
    let token = v.token.to_ascii_lowercase();
    let _guard = s.writes.lock().await;
    // A credential may have been revoked while this request waited for the lock.
    if !a.valid(&s).await {
        return Err(Error::unauthorized());
    }
    let id: String = sqlx::query_scalar(
        "INSERT INTO devices(id,user_id,session_id,token_id,platform,token,environment,app_version,registered_at)
         VALUES(?,?,?,?,'ios',?,?,?,?) ON CONFLICT(environment,token) DO UPDATE SET
         user_id=excluded.user_id,session_id=excluded.session_id,token_id=excluded.token_id,
         app_version=excluded.app_version,registered_at=excluded.registered_at,generation=devices.generation+1
         RETURNING id",
    )
    .bind(s.id())
    .bind(&a.user.id)
    .bind(a.session.then_some(&a.credential))
    .bind((!a.session).then_some(&a.credential))
    .bind(token)
    .bind(s.apns_environment.as_str())
    .bind(&v.app_version)
    .bind(crate::push::now_millis())
    .fetch_one(&s.db)
    .await?;
    Ok(Json(Device {
        id,
        platform: v.platform,
        app_version: v.app_version,
    }))
}

#[utoipa::path(delete,path="/devices/{id}",params(("id"=String,Path)),responses((status=204),(status=404,body=ApiError)),description="Remove your device registration before logging out. Other users cannot remove it, including administrators.")]
pub(crate) async fn remove(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let changed = sqlx::query("DELETE FROM devices WHERE id=? AND user_id=?")
        .bind(id)
        .bind(a.user.id)
        .execute(&s.db)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(Error::missing());
    }
    Ok(StatusCode::NO_CONTENT)
}
