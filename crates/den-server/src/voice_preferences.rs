use crate::{auth::Auth, *};

async fn load(s: &AppState, user: &str) -> Result<VoicePreferences> {
    let json: Option<String> =
        sqlx::query_scalar("SELECT preferences FROM user_voice_preferences WHERE user_id=?")
            .bind(user)
            .fetch_optional(&s.db)
            .await?;
    json.map(|v| {
        serde_json::from_str(&v).map_err(|_| Error::bad("Invalid stored voice preferences"))
    })
    .unwrap_or_else(|| Ok(VoicePreferences::default()))
}
#[utoipa::path(get,path="/users/me/voice",responses((status=200,body=VoicePreferences)))]
pub(crate) async fn get(State(s): State<AppState>, a: Auth) -> Result<Json<VoicePreferences>> {
    Ok(Json(load(&s, &a.user.id).await?))
}
#[utoipa::path(put,path="/users/me/voice",request_body=VoicePreferences,responses((status=200,body=VoicePreferences,description="Merges device entries with saved preferences")))]
pub(crate) async fn put(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(patch): ApiJson<VoicePreferences>,
) -> Result<Json<VoicePreferences>> {
    patch.validate().map_err(Error::bad)?;
    let _guard = s.writes.lock().await;
    let mut value = load(&s, &a.user.id).await?;
    value.microphones.extend(patch.microphones);
    value.cameras.extend(patch.cameras);
    value.validate().map_err(Error::bad)?;
    let json =
        serde_json::to_string(&value).map_err(|_| Error::bad("Invalid voice preferences"))?;
    sqlx::query("INSERT INTO user_voice_preferences(user_id,preferences) VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET preferences=excluded.preferences")
        .bind(&a.user.id).bind(json).execute(&s.db).await?;
    let _ = s.events.send(Event::VoicePreferencesUpdated {
        user_id: a.user.id,
        preferences: value.clone(),
    });
    Ok(Json(value))
}
