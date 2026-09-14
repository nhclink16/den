use crate::{auth::Auth, *};

#[utoipa::path(get,path="/users/me/appearance",responses((status=200,body=Appearance)))]
pub(crate) async fn get_appearance(State(s): State<AppState>, a: Auth) -> Result<Json<Appearance>> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT appearance FROM user_appearance WHERE user_id=?")
            .bind(&a.user.id)
            .fetch_optional(&s.db)
            .await?;
    Ok(Json(match value {
        Some(v) => serde_json::from_str(&v).map_err(|_| Error::bad("Invalid stored appearance"))?,
        None => Appearance::default(),
    }))
}
#[utoipa::path(put,path="/users/me/appearance",request_body=Appearance,responses((status=200,body=Appearance)))]
pub(crate) async fn put_appearance(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<Appearance>,
) -> Result<Json<Appearance>> {
    v.validate().map_err(Error::bad)?;
    let _guard = s.writes.lock().await;
    let json = serde_json::to_string(&v).map_err(|_| Error::bad("Invalid appearance"))?;
    sqlx::query("INSERT INTO user_appearance(user_id,appearance) VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET appearance=excluded.appearance")
        .bind(&a.user.id).bind(json).execute(&s.db).await?;
    let _ = s.events.send(Event::AppearanceUpdated {
        user_id: a.user.id,
        appearance: v.clone(),
    });
    Ok(Json(v))
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
            assert_eq!(a.theme, if id == "a" { "custom" } else { "den" });
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
