use crate::{auth::Auth, *};

pub(crate) async fn load(s: &AppState, user: &str) -> Result<Appearance> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT appearance FROM user_appearance WHERE user_id=?")
            .bind(user)
            .fetch_optional(&s.db)
            .await?;
    match value {
        Some(v) => serde_json::from_str(&v).map_err(|_| Error::bad("Invalid stored appearance")),
        None => Ok(Appearance::default()),
    }
}

fn response(value: Appearance, missing: bool) -> Response {
    let mut response = Json(value).into_response();
    if missing {
        response
            .headers_mut()
            .insert("x-den-background-status", "missing".parse().unwrap());
    }
    response
}

#[utoipa::path(get,path="/users/me/appearance",responses((status=200,body=Appearance,
    description="Missing upload resolves to background:null and X-Den-Background-Status: missing")))]
pub(crate) async fn get_appearance(State(s): State<AppState>, a: Auth) -> Result<Response> {
    let _guard = s.writes.lock().await;
    let mut value = load(&s, &a.user.id).await?;
    let missing = backgrounds::resolve(&s, &a.user.id, &mut value).await?;
    Ok(response(value, missing))
}
#[utoipa::path(put,path="/users/me/appearance",request_body=Appearance,responses((status=200,body=Appearance,
    description="Returns clamped values; missing upload resolves to background:null and X-Den-Background-Status: missing")))]
pub(crate) async fn put_appearance(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(raw): ApiJson<serde_json::Value>,
) -> Result<Response> {
    // Same answer the JSON extractor gives a body that does not fit the type.
    let mut v: Appearance = serde_json::from_value(raw.clone()).map_err(|_| {
        Error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            "Invalid JSON request".into(),
        )
    })?;
    v.validate().map_err(Error::bad)?;
    let _guard = s.writes.lock().await;
    // Clients that predate the sidebar wallpaper send the whole object without it.
    // Absent means "leave it"; only an explicit null clears it.
    if raw.get("sidebar_background").is_none() {
        v.sidebar_background = load(&s, &a.user.id).await?.sidebar_background;
    }
    let missing = backgrounds::resolve(&s, &a.user.id, &mut v).await?;
    save(&s, &a.user.id, &v).await?;
    Ok(response(v, missing))
}

// Call with the write lock held so file replacement and appearance events agree.
pub(crate) async fn save(s: &AppState, user: &str, value: &Appearance) -> Result<()> {
    let json = serde_json::to_string(value).map_err(|_| Error::bad("Invalid appearance"))?;
    sqlx::query("INSERT INTO user_appearance(user_id,appearance) VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET appearance=excluded.appearance")
        .bind(user).bind(json).execute(&s.db).await?;
    let _ = s.events.send(Event::AppearanceUpdated {
        user_id: user.into(),
        appearance: value.clone(),
    });
    Ok(())
}

// Complete 0008 before opening HTTP. This is deliberately repeatable after a
// process crash, and all rows are committed together, with no partially filled API.
pub(crate) async fn complete_theme_pairs(db: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT user_id, appearance FROM user_appearance WHERE EXISTS (SELECT 1 FROM json_each(appearance, '$.custom_themes') WHERE json_type(value, '$.light') = 'null' OR json_type(value, '$.dark') = 'null')").fetch_all(&mut *tx).await?;
    for (user, raw) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&raw)?;
        for theme in value["custom_themes"].as_array_mut().unwrap() {
            for (missing, source) in [("light", "dark"), ("dark", "light")] {
                if theme[missing].is_null() {
                    let colors: ThemeColors = serde_json::from_value(theme[source].clone())?;
                    theme[missing] =
                        serde_json::to_value(den_core::theme_color::derive_half(&colors))?;
                }
            }
        }
        sqlx::query("UPDATE user_appearance SET appearance=? WHERE user_id=?")
            .bind(serde_json::to_string(&value)?)
            .bind(user)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn migration_preserves_custom_colors_and_completes_only_missing_halves() {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE users (id TEXT PRIMARY KEY)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0007_appearance.sql"))
            .execute(&db)
            .await
            .unwrap();
        let tide = builtin_themes()
            .into_iter()
            .find(|t| t.id == "tide")
            .unwrap();
        for (id, dark, light) in [
            ("a", Some("custom"), Some("den-light")),
            ("b", None, Some("den-light")),
            ("c", None, None),
        ] {
            sqlx::query("INSERT INTO users VALUES(?)")
                .bind(id)
                .execute(&db)
                .await
                .unwrap();
            let raw = serde_json::json!({"mode":"system","dark_theme":dark,"light_theme":light,"custom_themes":[
                {"id":"custom","name":"Old dark","appearance":"dark","colors":tide.dark,"fonts":tide.fonts,"radius":"soft","density":"compact"},
                {"id":"old-light","name":"Old light","appearance":"light","colors":tide.light,"fonts":tide.fonts,"radius":"round","density":"comfortable"}
            ]});
            sqlx::query("INSERT INTO user_appearance VALUES(?,?)")
                .bind(id)
                .bind(raw.to_string())
                .execute(&db)
                .await
                .unwrap();
        }
        sqlx::raw_sql(include_str!("../migrations/0008_theme_pairs.sql"))
            .execute(&db)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0013_split_theme_choice.sql"))
            .execute(&db)
            .await
            .unwrap();
        // This models a restart after SQL committed and before completion began.
        complete_theme_pairs(&db).await.unwrap();
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT user_id,appearance FROM user_appearance ORDER BY user_id")
                .fetch_all(&db)
                .await
                .unwrap();
        for (id, raw) in &rows {
            let a: Appearance = serde_json::from_str(raw).unwrap();
            a.validate().unwrap();
            assert_eq!(a.light_theme, if id == "a" { "custom" } else { "den" });
            assert_eq!(a.custom_themes[0].dark, tide.dark);
            assert_eq!(
                a.custom_themes[0].light,
                den_core::theme_color::derive_half(&tide.dark)
            );
            assert_eq!(a.custom_themes[1].light, tide.light);
            assert!(a.custom_themes[1].dark.generated);
        }
        complete_theme_pairs(&db).await.unwrap();
        let again: Vec<(String, String)> =
            sqlx::query_as("SELECT user_id,appearance FROM user_appearance ORDER BY user_id")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(again, rows);
    }
}
