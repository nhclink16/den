use crate::{auth::Auth, *};
use axum::extract::{Path, Query};

struct DbChannel { id:String,name:String,category_id:Option<String>,kind:String,position:i64 }
impl From<DbChannel> for Channel { fn from(v:DbChannel)->Self { Self{id:v.id,name:v.name,category_id:v.category_id,kind:if v.kind=="dm" {ChannelKind::Dm}else{ChannelKind::Text},position:v.position} } }
pub(crate) async fn visible(s:&AppState,user_id:&str,id:&str)->Result<Channel> {
    Ok(sqlx::query_as!(DbChannel,"SELECT id,name,category_id,kind,position FROM channels WHERE id=? AND (kind='text' OR EXISTS(SELECT 1 FROM channel_members WHERE channel_id=channels.id AND user_id=?))",id,user_id).fetch_optional(&s.db).await?.ok_or_else(Error::missing)?.into())
}
#[utoipa::path(get,path="/categories",responses((status=200,body=Vec<Category>)))]
pub(crate) async fn categories(State(s):State<AppState>,_a:Auth)->Result<Json<Vec<Category>>> { Ok(Json(sqlx::query_as!(Category,"SELECT id,name,position FROM categories ORDER BY position,id").fetch_all(&s.db).await?)) }
#[utoipa::path(post,path="/categories",request_body=SaveCategory,responses((status=200,body=Category)))]
pub(crate) async fn create_category(State(s):State<AppState>,a:Auth,ApiJson(v):ApiJson<SaveCategory>)->Result<Json<Category>> {
    a.admin()?;name(&v.name)?;let id=s.id();sqlx::query!("INSERT INTO categories(id,name,position) VALUES(?,?,?)",id,v.name,v.position).execute(&s.db).await?;Ok(Json(Category{id,name:v.name,position:v.position}))
}
#[utoipa::path(put,path="/categories/{id}",params(("id"=String,Path)),request_body=SaveCategory,responses((status=200,body=Category)))]
pub(crate) async fn update_category(State(s):State<AppState>,a:Auth,Path(id):Path<String>,ApiJson(v):ApiJson<SaveCategory>)->Result<Json<Category>> {
    a.admin()?;name(&v.name)?;if sqlx::query!("UPDATE categories SET name=?,position=? WHERE id=?",v.name,v.position,id).execute(&s.db).await?.rows_affected()==0 { return Err(Error::missing()); }Ok(Json(Category{id,name:v.name,position:v.position}))
}
#[utoipa::path(delete,path="/categories/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn delete_category(State(s):State<AppState>,a:Auth,Path(id):Path<String>)->Result<StatusCode> { a.admin()?;sqlx::query!("DELETE FROM categories WHERE id=?",id).execute(&s.db).await?;Ok(StatusCode::NO_CONTENT) }
#[utoipa::path(get,path="/channels",responses((status=200,body=Vec<Channel>)))]
pub(crate) async fn channels(State(s):State<AppState>,a:Auth)->Result<Json<Vec<Channel>>> {
    Ok(Json(sqlx::query_as!(DbChannel,"SELECT id,name,category_id,kind,position FROM channels WHERE kind='text' OR EXISTS(SELECT 1 FROM channel_members WHERE channel_id=channels.id AND user_id=?) ORDER BY position,id",a.user.id).fetch_all(&s.db).await?.into_iter().map(Into::into).collect()))
}
#[utoipa::path(get,path="/channels/{id}",params(("id"=String,Path)),responses((status=200,body=Channel)))]
pub(crate) async fn channel(State(s):State<AppState>,a:Auth,Path(id):Path<String>)->Result<Json<Channel>> { Ok(Json(visible(&s,&a.user.id,&id).await?)) }
#[utoipa::path(post,path="/channels",request_body=SaveChannel,responses((status=200,body=Channel)))]
pub(crate) async fn create_channel(State(s):State<AppState>,a:Auth,ApiJson(v):ApiJson<SaveChannel>)->Result<Json<Channel>> {
    a.admin()?;name(&v.name)?;let id=s.id();sqlx::query!("INSERT INTO channels(id,name,category_id,kind,position) VALUES(?,?,?,'text',?)",id,v.name,v.category_id,v.position).execute(&s.db).await?;Ok(Json(visible(&s,&a.user.id,&id).await?))
}
#[utoipa::path(put,path="/channels/{id}",params(("id"=String,Path)),request_body=SaveChannel,responses((status=200,body=Channel)))]
pub(crate) async fn update_channel(State(s):State<AppState>,a:Auth,Path(id):Path<String>,ApiJson(v):ApiJson<SaveChannel>)->Result<Json<Channel>> {
    a.admin()?;name(&v.name)?;if sqlx::query!("UPDATE channels SET name=?,category_id=?,position=? WHERE id=? AND kind='text'",v.name,v.category_id,v.position,id).execute(&s.db).await?.rows_affected()==0 {return Err(Error::missing());} Ok(Json(visible(&s,&a.user.id,&id).await?))
}
#[utoipa::path(delete,path="/channels/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn delete_channel(State(s):State<AppState>,a:Auth,Path(id):Path<String>)->Result<StatusCode> {
    a.admin()?;sqlx::query!("DELETE FROM channels WHERE id=? AND kind='text'",id).execute(&s.db).await?;Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/dms",request_body=CreateDm,responses((status=200,body=Channel)))]
pub(crate) async fn dm(State(s):State<AppState>,a:Auth,ApiJson(mut v):ApiJson<CreateDm>)->Result<Json<Channel>> {
    if v.member_ids.is_empty() || v.member_ids.len()>20 {return Err(Error::bad("Supply 1-20 DM members"));}
    v.member_ids.push(a.user.id.clone());v.member_ids.sort();v.member_ids.dedup();
    for member in &v.member_ids { super::auth::user(&s,member).await?; }
    let key=v.member_ids.join(",");let id=s.id();let mut tx=s.db.begin().await?;
    sqlx::query!("INSERT INTO channels(id,name,kind,dm_key) VALUES(?,'DM','dm',?) ON CONFLICT(dm_key) DO NOTHING",id,key).execute(&mut *tx).await?;
    let actual=sqlx::query_scalar!("SELECT id FROM channels WHERE dm_key=?",key).fetch_one(&mut *tx).await?;
    for member in &v.member_ids { sqlx::query!("INSERT OR IGNORE INTO channel_members(channel_id,user_id) VALUES(?,?)",actual,member).execute(&mut *tx).await?; }
    tx.commit().await?;Ok(Json(visible(&s,&a.user.id,&actual).await?))
}
struct DbMessage { id:String,channel_id:String,author_id:String,content:String,reply_to:Option<String>,created_at:String,edited_at:Option<String> }
async fn with_uploads(s:&AppState,v:DbMessage)->Result<Message> {
    let attachments=sqlx::query_as!(Upload,"SELECT id,channel_id,filename,content_type,size,offset,complete as \"complete: bool\" FROM uploads WHERE message_id=? ORDER BY id",v.id).fetch_all(&s.db).await?;
    Ok(Message{id:v.id,channel_id:v.channel_id,author_id:v.author_id,content:v.content,reply_to:v.reply_to,created_at:v.created_at,edited_at:v.edited_at,attachments})
}
async fn get_message(s:&AppState,id:&str)->Result<Message> { with_uploads(s,sqlx::query_as!(DbMessage,"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at FROM messages WHERE id=?",id).fetch_one(&s.db).await?).await }
#[utoipa::path(get,path="/channels/{id}/messages",params(("id"=String,Path),MessageQuery),responses((status=200,body=Vec<Message>)))]
pub(crate) async fn messages(State(s):State<AppState>,a:Auth,Path(id):Path<String>,Query(q):Query<MessageQuery>)->Result<Json<Vec<Message>>> {
    visible(&s,&a.user.id,&id).await?;
    if q.before.is_some() && q.after.is_some() {return Err(Error::bad("Use before or after, not both"));}
    let limit=q.limit.unwrap_or(50);if !(1..=200).contains(&limit) {return Err(Error::bad("Limit must be 1-200"));}
    let limit=limit as i64;
    let rows=if q.after.is_some() {
        sqlx::query_as!(DbMessage,"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at FROM messages WHERE channel_id=? AND id>? ORDER BY id ASC LIMIT ?",id,q.after,limit).fetch_all(&s.db).await?
    }else{
        let mut rows=sqlx::query_as!(DbMessage,"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at FROM messages WHERE channel_id=? AND (? IS NULL OR id<?) ORDER BY id DESC LIMIT ?",id,q.before,q.before,limit).fetch_all(&s.db).await?;rows.reverse();rows
    };
    let mut result=Vec::new();for row in rows {result.push(with_uploads(&s,row).await?);}Ok(Json(result))
}
#[utoipa::path(post,path="/channels/{id}/messages",params(("id"=String,Path)),request_body=CreateMessage,responses((status=200,body=Message)))]
pub(crate) async fn send(State(s):State<AppState>,a:Auth,Path(channel):Path<String>,ApiJson(v):ApiJson<CreateMessage>)->Result<Json<Message>> {
    visible(&s,&a.user.id,&channel).await?;
    if v.content.len()>32000 || (v.content.trim().is_empty() && v.upload_ids.is_empty()) || v.upload_ids.len()>10 {return Err(Error::bad("Message needs text or uploads; maximum 32000 bytes and 10 uploads"));}
    let _guard=s.writes.lock().await;let id=s.id();let mut tx=s.db.begin().await?;
    if let Some(reply)=&v.reply_to {
        if sqlx::query_scalar!("SELECT count(*) FROM messages WHERE id=? AND channel_id=?",reply,channel).fetch_one(&mut *tx).await?!=1 {return Err(Error::bad("Reply must belong to this channel"));}
    }
    sqlx::query!("INSERT INTO messages(id,channel_id,author_id,content,reply_to) VALUES(?,?,?,?,?)",id,channel,a.user.id,v.content,v.reply_to).execute(&mut *tx).await?;
    for upload in &v.upload_ids {
        if sqlx::query!("UPDATE uploads SET message_id=? WHERE id=? AND channel_id=? AND owner_id=? AND complete=1 AND message_id IS NULL",id,upload,channel,a.user.id).execute(&mut *tx).await?.rows_affected()!=1 {return Err(Error::bad("Upload must be complete, unattached, owned by you and in this channel"));}
    }
    tx.commit().await?;let msg=get_message(&s,&id).await?;let _=s.events.send(Event::MessageCreated(msg.clone()));Ok(Json(msg))
}
#[utoipa::path(patch,path="/messages/{id}",params(("id"=String,Path)),request_body=EditMessage,responses((status=200,body=Message)))]
pub(crate) async fn edit(State(s):State<AppState>,a:Auth,Path(id):Path<String>,ApiJson(v):ApiJson<EditMessage>)->Result<Json<Message>> {
    let _guard=s.writes.lock().await;let old=get_message(&s,&id).await?;visible(&s,&a.user.id,&old.channel_id).await?;
    if old.author_id!=a.user.id {return Err(Error::forbidden());}if v.content.len()>32000 || (v.content.trim().is_empty()&&old.attachments.is_empty()) {return Err(Error::bad("Invalid message length"));}
    sqlx::query!("UPDATE messages SET content=?,edited_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?",v.content,id).execute(&s.db).await?;
    let msg=get_message(&s,&id).await?;let _=s.events.send(Event::MessageEdited(msg.clone()));Ok(Json(msg))
}
#[utoipa::path(delete,path="/messages/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn remove(State(s):State<AppState>,a:Auth,Path(id):Path<String>)->Result<StatusCode> {
    let _guard=s.writes.lock().await;let old=get_message(&s,&id).await?;visible(&s,&a.user.id,&old.channel_id).await?;
    if old.author_id!=a.user.id && a.user.role!=Role::Admin {return Err(Error::forbidden());}
    sqlx::query!("DELETE FROM messages WHERE id=?",id).execute(&s.db).await?;let _=s.events.send(Event::MessageDeleted{id,channel_id:old.channel_id});Ok(StatusCode::NO_CONTENT)
}
