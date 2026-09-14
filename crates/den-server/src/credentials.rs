use crate::{
    auth::{hash, secret, user, username, Auth},
    *,
};
use axum::extract::Path;

#[utoipa::path(post,path="/invites",request_body=CreateInvite,responses((status=200,body=Invite)))]
pub(crate) async fn invite(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<CreateInvite>,
) -> Result<Json<Invite>> {
    a.admin()?;
    if !(1..=100).contains(&v.uses) || !(1..=720).contains(&v.expires_in_hours) {
        return Err(Error::bad(
            "Invite uses must be 1-100 and expiry 1-720 hours",
        ));
    }
    let code = secret();
    let digest = hash(&code);
    let id = s.id();
    let expiry = now() + v.expires_in_hours as i64 * 3600;
    sqlx::query!(
        "INSERT INTO invites(id,secret_hash,uses_left,expires_at) VALUES(?,?,?,?)",
        id,
        digest,
        v.uses,
        expiry
    )
    .execute(&s.db)
    .await?;
    Ok(Json(Invite {
        id,
        code,
        uses_left: v.uses,
        expires_at: expiry,
    }))
}
#[utoipa::path(delete,path="/invites/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn revoke_invite(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    a.admin()?;
    sqlx::query!("DELETE FROM invites WHERE id=?", id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn mint(s: &AppState, user_id: &str, name: &str) -> Result<TokenSecret> {
    crate::name(name)?;
    let token = secret();
    let digest = hash(&token);
    let id = s.id();
    sqlx::query!(
        "INSERT INTO tokens(id,user_id,name,secret_hash) VALUES(?,?,?,?)",
        id,
        user_id,
        name,
        digest
    )
    .execute(&s.db)
    .await?;
    Ok(TokenSecret {
        token,
        credential: Token {
            id,
            name: name.into(),
            user_id: user_id.into(),
        },
    })
}
#[utoipa::path(post,path="/tokens",request_body=CreateToken,responses((status=200,body=TokenSecret)))]
pub(crate) async fn create_token(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<CreateToken>,
) -> Result<Json<TokenSecret>> {
    let target = v.user_id.as_deref().unwrap_or(&a.user.id);
    if target != a.user.id {
        let owner = sqlx::query_scalar!("SELECT owner_id FROM users WHERE id=? AND bot=1", target)
            .fetch_optional(&s.db)
            .await?
            .flatten();
        if owner.as_deref() != Some(&a.user.id) {
            return Err(Error::forbidden());
        }
    }
    Ok(Json(mint(&s, target, &v.name).await?))
}
#[utoipa::path(get,path="/tokens",responses((status=200,body=Vec<Token>)))]
pub(crate) async fn tokens(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<Token>>> {
    Ok(Json(sqlx::query_as!(Token,"SELECT t.id,t.name,t.user_id FROM tokens t JOIN users u ON u.id=t.user_id WHERE u.id=? OR u.owner_id=? ORDER BY t.id",a.user.id,a.user.id).fetch_all(&s.db).await?))
}
#[utoipa::path(delete,path="/tokens/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn revoke_token(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let changed=sqlx::query!("DELETE FROM tokens WHERE id=? AND user_id IN (SELECT id FROM users WHERE id=? OR owner_id=?)",id,a.user.id,a.user.id).execute(&s.db).await?;
    if changed.rows_affected() == 0 {
        return Err(Error::missing());
    }
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/bots",request_body=CreateBot,responses((status=200,body=BotCreated)))]
pub(crate) async fn bot(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<CreateBot>,
) -> Result<Json<BotCreated>> {
    if a.user.bot {
        return Err(Error::forbidden());
    }
    username(&v.username)?;
    crate::name(&v.display_name)?;
    let id = s.id();
    sqlx::query!(
        "INSERT INTO users(id,username,display_name,bot,owner_id,role) VALUES(?,?,?,1,?,'member')",
        id,
        v.username,
        v.display_name,
        a.user.id
    )
    .execute(&s.db)
    .await?;
    Ok(Json(BotCreated {
        user: user(&s, &id).await?,
        credential: mint(&s, &id, "initial").await?,
    }))
}
