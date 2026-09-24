// Admin care of the member list: renaming someone and removing them. Pictures
// live in profile_images, which admins may also set for anyone.
use crate::{auth::Auth, *};
use axum::extract::Path;

async fn member(s: &AppState, id: &str) -> Result<User> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?;
    auth::user(s, &exists.ok_or_else(Error::missing)?).await
}

#[utoipa::path(patch,path="/users/{id}",params(("id"=String,Path)),request_body=MemberPatch,responses((status=200,body=User),(status=400,body=ApiError),(status=403,body=ApiError),(status=404,body=ApiError),(status=409,body=ApiError)))]
pub(crate) async fn patch(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<MemberPatch>,
) -> Result<Json<User>> {
    a.admin()?;
    let current = member(&s, &id).await?;
    let username = v.username.as_deref().map(str::trim);
    if let Some(name) = username {
        auth::username(name)?;
    }
    let username = username.unwrap_or(&current.username).to_string();
    // A blank display name means "same as the username", as at sign-up.
    let display = match v.display_name.as_deref().map(str::trim) {
        Some("") => username.clone(),
        Some(name) => {
            crate::name(name)?;
            name.to_string()
        }
        None if current.display_name == current.username => username.clone(),
        None => current.display_name.clone(),
    };
    sqlx::query("UPDATE users SET username=?, display_name=? WHERE id=?")
        .bind(&username)
        .bind(&display)
        .bind(&id)
        .execute(&s.db)
        .await
        .map_err(|e| match Error::from(e) {
            Error(StatusCode::CONFLICT, ..) => Error::conflict("That username is taken"),
            other => other,
        })?;
    let user = auth::user(&s, &id).await?;
    profiles::broadcast(&s, &user);
    Ok(Json(user))
}

/// Removes a member without deleting their history: they are signed out
/// everywhere and cannot log in again, and their messages keep their name.
#[utoipa::path(delete,path="/users/{id}",params(("id"=String,Path)),responses((status=200,body=User),(status=400,body=ApiError),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn remove(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<User>> {
    a.admin()?;
    if id == a.user.id {
        return Err(Error::bad("You can't remove yourself"));
    }
    member(&s, &id).await?;
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE users SET removed_at=coalesce(removed_at, ?) WHERE id=?")
        .bind(now())
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    for sql in [
        "DELETE FROM sessions WHERE user_id=?",
        "DELETE FROM tokens WHERE user_id=?",
        "DELETE FROM devices WHERE user_id=?",
    ] {
        sqlx::query(sql).bind(&id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    // Open sockets notice the missing session on their next check and close.
    activities::clear_all(&s, &id);
    let user = auth::user(&s, &id).await?;
    profiles::broadcast(&s, &user);
    Ok(Json(user))
}
