use crate::{auth::Auth, *};
use unicode_segmentation::UnicodeSegmentation;

// Lazy expiry happens on user reads, including authentication. The SQL predicate
// checks the current status, so a replacement status cannot be cleared by a stale read.
pub(crate) async fn expire(s: &AppState, user: Option<&str>) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("UPDATE users SET status=NULL WHERE (? IS NULL OR id=?) AND json_extract(status,'$.expires_at')<=? RETURNING id")
        .bind(user).bind(user).bind(now()).fetch_all(&s.db).await?)
}

pub(crate) fn broadcast(s: &AppState, user: &User) {
    let _ = s.events.send(Event::UserUpdated { user: user.clone() });
}

#[utoipa::path(patch,path="/users/me/profile",request_body=ProfilePatch,responses((status=200,body=User),(status=400,body=ApiError)))]
pub(crate) async fn patch(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<ProfilePatch>,
) -> Result<Json<User>> {
    if let Some(name) = &v.display_name {
        crate::name(name)?;
    }
    if let Some(Some(bio)) = &v.bio {
        if bio.chars().count() > 190 {
            return Err(Error::bad("Bio must be at most 190 characters"));
        }
    }
    if let Some(Some(accent)) = &v.accent {
        if accent.len() != 7
            || !accent.starts_with('#')
            || !accent.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(Error::bad("Accent must be a six-digit hex color"));
        }
    }
    if let Some(Some(status)) = &v.status {
        if status
            .text
            .as_ref()
            .is_some_and(|text| text.chars().count() > 60)
        {
            return Err(Error::bad("Status text must be at most 60 characters"));
        }
        if status
            .emoji
            .as_ref()
            .is_some_and(|emoji| emoji.graphemes(true).count() != 1)
        {
            return Err(Error::bad("Status emoji must be a single grapheme"));
        }
    }
    let _guard = s.writes.lock().await;
    // Load after acquiring the lock so simultaneous subset patches preserve one another.
    let mut user = auth::user(&s, &a.user.id).await?;
    if let Some(name) = v.display_name {
        user.display_name = name;
    }
    if let Some(bio) = v.bio {
        user.bio = bio;
    }
    if let Some(accent) = v.accent {
        user.accent = accent.map(|v| v.to_ascii_lowercase());
    }
    if let Some(status) = v.status {
        user.status = status.filter(|v| v.expires_at.is_none_or(|at| at > now()));
    }
    let status = user
        .status
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| Error::bad("Invalid status"))?;
    sqlx::query("UPDATE users SET display_name=?,bio=?,accent=?,status=? WHERE id=?")
        .bind(&user.display_name)
        .bind(&user.bio)
        .bind(&user.accent)
        .bind(status)
        .bind(&user.id)
        .execute(&s.db)
        .await?;
    broadcast(&s, &user);
    Ok(Json(user))
}
