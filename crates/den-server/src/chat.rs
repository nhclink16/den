use crate::{auth::Auth, *};
use axum::extract::Path;

struct DbChannel {
    id: String,
    name: String,
    category_id: Option<String>,
    kind: String,
    position: i64,
}
impl From<DbChannel> for Channel {
    fn from(v: DbChannel) -> Self {
        Self {
            id: v.id,
            name: v.name,
            category_id: v.category_id,
            kind: if v.kind == "dm" {
                ChannelKind::Dm
            } else if v.kind == "voice" {
                ChannelKind::Voice
            } else {
                ChannelKind::Text
            },
            position: v.position,
            member_ids: Vec::new(),
        }
    }
}
pub(crate) async fn visible(s: &AppState, user_id: &str, id: &str) -> Result<Channel> {
    let channel = sqlx::query_as!(DbChannel,"SELECT id,name,category_id,kind,position FROM channels WHERE id=? AND (kind IN ('text','voice') OR EXISTS(SELECT 1 FROM channel_members WHERE channel_id=channels.id AND user_id=?))",id,user_id).fetch_optional(&s.db).await?.ok_or_else(Error::missing)?.into();
    members(s, channel).await
}
#[utoipa::path(get,path="/categories",responses((status=200,body=Vec<Category>)))]
pub(crate) async fn categories(State(s): State<AppState>, _a: Auth) -> Result<Json<Vec<Category>>> {
    Ok(Json(
        sqlx::query_as!(
            Category,
            "SELECT id,name,position FROM categories ORDER BY position,id"
        )
        .fetch_all(&s.db)
        .await?,
    ))
}
#[utoipa::path(post,path="/categories",request_body=SaveCategory,responses((status=200,body=Category)))]
pub(crate) async fn create_category(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<SaveCategory>,
) -> Result<Json<Category>> {
    a.admin()?;
    name(&v.name)?;
    let id = s.id();
    sqlx::query!(
        "INSERT INTO categories(id,name,position) VALUES(?,?,?)",
        id,
        v.name,
        v.position
    )
    .execute(&s.db)
    .await?;
    Ok(Json(Category {
        id,
        name: v.name,
        position: v.position,
    }))
}
#[utoipa::path(put,path="/categories/{id}",params(("id"=String,Path)),request_body=SaveCategory,responses((status=200,body=Category)))]
pub(crate) async fn update_category(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<SaveCategory>,
) -> Result<Json<Category>> {
    a.admin()?;
    name(&v.name)?;
    if sqlx::query!(
        "UPDATE categories SET name=?,position=? WHERE id=?",
        v.name,
        v.position,
        id
    )
    .execute(&s.db)
    .await?
    .rows_affected()
        == 0
    {
        return Err(Error::missing());
    }
    Ok(Json(Category {
        id,
        name: v.name,
        position: v.position,
    }))
}
#[utoipa::path(delete,path="/categories/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn delete_category(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    a.admin()?;
    sqlx::query!("DELETE FROM categories WHERE id=?", id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/channels",responses((status=200,body=Vec<Channel>)))]
pub(crate) async fn channels(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<Channel>>> {
    let rows = sqlx::query_as!(DbChannel,"SELECT id,name,category_id,kind,position FROM channels WHERE kind IN ('text','voice') OR EXISTS(SELECT 1 FROM channel_members WHERE channel_id=channels.id AND user_id=?) ORDER BY position,id",a.user.id).fetch_all(&s.db).await?;
    let mut result = Vec::new();
    for row in rows {
        result.push(members(&s, row.into()).await?);
    }
    Ok(Json(result))
}
async fn members(s: &AppState, mut channel: Channel) -> Result<Channel> {
    if channel.kind == ChannelKind::Dm {
        channel.member_ids = sqlx::query_scalar!(
            "SELECT user_id FROM channel_members WHERE channel_id=? ORDER BY user_id",
            channel.id
        )
        .fetch_all(&s.db)
        .await?;
    }
    Ok(channel)
}
#[utoipa::path(get,path="/channels/{id}",params(("id"=String,Path)),responses((status=200,body=Channel)))]
pub(crate) async fn channel(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<Channel>> {
    Ok(Json(visible(&s, &a.user.id, &id).await?))
}
#[utoipa::path(post,path="/channels",request_body=SaveChannel,responses((status=200,body=Channel)))]
pub(crate) async fn create_channel(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<SaveChannel>,
) -> Result<Json<Channel>> {
    a.admin()?;
    name(&v.name)?;
    let id = s.id();
    sqlx::query!(
        "INSERT INTO channels(id,name,category_id,kind,position) VALUES(?,?,?,'text',?)",
        id,
        v.name,
        v.category_id,
        v.position
    )
    .execute(&s.db)
    .await?;
    Ok(Json(visible(&s, &a.user.id, &id).await?))
}
#[utoipa::path(put,path="/channels/{id}",params(("id"=String,Path)),request_body=SaveChannel,responses((status=200,body=Channel)))]
pub(crate) async fn update_channel(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<SaveChannel>,
) -> Result<Json<Channel>> {
    a.admin()?;
    name(&v.name)?;
    if sqlx::query!(
        "UPDATE channels SET name=?,category_id=?,position=? WHERE id=? AND kind IN ('text','voice')",
        v.name,
        v.category_id,
        v.position,
        id
    )
    .execute(&s.db)
    .await?
    .rows_affected()
        == 0
    {
        return Err(Error::missing());
    }
    Ok(Json(visible(&s, &a.user.id, &id).await?))
}
#[utoipa::path(delete,path="/channels/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn delete_channel(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    a.admin()?;
    sqlx::query!(
        "DELETE FROM channels WHERE id=? AND kind IN ('text','voice')",
        id
    )
    .execute(&s.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/dms",request_body=CreateDm,responses((status=200,body=Channel)))]
pub(crate) async fn dm(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(mut v): ApiJson<CreateDm>,
) -> Result<Json<Channel>> {
    if v.member_ids.is_empty() || v.member_ids.len() > 20 {
        return Err(Error::bad("Supply 1-20 DM members"));
    }
    v.member_ids.push(a.user.id.clone());
    v.member_ids.sort();
    v.member_ids.dedup();
    for member in &v.member_ids {
        super::auth::user(&s, member).await?;
    }
    let key = v.member_ids.join(",");
    let id = s.id();
    let mut tx = s.db.begin().await?;
    sqlx::query!("INSERT INTO channels(id,name,kind,dm_key) VALUES(?,'DM','dm',?) ON CONFLICT(dm_key) DO NOTHING",id,key).execute(&mut *tx).await?;
    let actual = sqlx::query_scalar!("SELECT id FROM channels WHERE dm_key=?", key)
        .fetch_one(&mut *tx)
        .await?;
    for member in &v.member_ids {
        sqlx::query!(
            "INSERT OR IGNORE INTO channel_members(channel_id,user_id) VALUES(?,?)",
            actual,
            member
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(visible(&s, &a.user.id, &actual).await?))
}
