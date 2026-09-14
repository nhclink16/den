use crate::{auth::Auth, *};

#[utoipa::path(get,path="/users/me/appearance",responses((status=200,body=Appearance)))]
pub(crate) async fn get(State(s): State<AppState>, a: Auth) -> Result<Json<Appearance>> {
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
pub(crate) async fn put(
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
