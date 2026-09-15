use crate::{auth::Auth, chat::visible, *};
use axum::extract::{Path, Query};

pub(crate) struct DbMessage {
    pub(crate) id: String,
    pub(crate) channel_id: String,
    pub(crate) author_id: String,
    pub(crate) content: String,
    pub(crate) reply_to: Option<String>,
    pub(crate) created_at: String,
    pub(crate) edited_at: Option<String>,
    pub(crate) thread_id: Option<String>,
}
pub(crate) async fn with_uploads(s: &AppState, v: DbMessage) -> Result<Message> {
    let attachments=sqlx::query_as!(Upload,"SELECT id,channel_id,filename,content_type,size,offset,complete as \"complete: bool\", CASE WHEN thumbnail_ready=1 THEN '/uploads/'||id||'/thumbnail' END as \"thumbnail_url?: String\" FROM uploads WHERE message_id=? ORDER BY id",v.id).fetch_all(&s.db).await?;
    // A root carries its conversation's summary so a client can place it without a
    // second request; a reply carries only the thread it belongs to.
    let thread = threads::for_root(s, &v.id).await?;
    Ok(Message {
        id: v.id.clone(),
        channel_id: v.channel_id,
        author_id: v.author_id,
        content: v.content,
        reply_to: v.reply_to,
        created_at: v.created_at,
        edited_at: v.edited_at,
        attachments,
        objects: objects::for_message(s, &v.id).await?,
        reactions: activity::reactions(s, &v.id).await?,
        mention_ids: sqlx::query_scalar!(
            "SELECT user_id FROM message_mentions WHERE message_id=? ORDER BY user_id",
            v.id
        )
        .fetch_all(&s.db)
        .await?,
        thread_id: v.thread_id,
        thread,
    })
}
pub(crate) async fn get_message(s: &AppState, id: &str) -> Result<Message> {
    with_uploads(s,sqlx::query_as!(DbMessage,r#"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at,thread_id as "thread_id?" FROM messages WHERE id=?"#,id).fetch_one(&s.db).await?).await
}
#[utoipa::path(get,path="/channels/{id}/messages",params(("id"=String,Path),MessageQuery),responses((status=200,body=Vec<Message>)))]
pub(crate) async fn messages(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    query: std::result::Result<Query<MessageQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<Message>>> {
    let Query(q) = query.map_err(|_| Error::bad("Invalid message query"))?;
    visible(&s, &a.user.id, &id).await?;
    if q.before.is_some() && q.after.is_some() {
        return Err(Error::bad("Use before or after, not both"));
    }
    let limit = q.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(Error::bad("Limit must be 1-200"));
    }
    let limit = limit as i64;
    // Absent or false is the legacy flat list, which keeps old clients and their
    // pre-migration quoted replies exactly as they were.
    let roots_only = q.roots_only.unwrap_or(false);
    let rows = if q.after.is_some() {
        sqlx::query_as!(DbMessage,r#"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at,thread_id as "thread_id?" FROM messages WHERE channel_id=? AND id>? AND (? = 0 OR thread_id IS NULL) ORDER BY id ASC LIMIT ?"#,id,q.after,roots_only,limit).fetch_all(&s.db).await?
    } else {
        let mut rows=sqlx::query_as!(DbMessage,r#"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at,thread_id as "thread_id?" FROM messages WHERE channel_id=? AND (? IS NULL OR id<?) AND (? = 0 OR thread_id IS NULL) ORDER BY id DESC LIMIT ?"#,id,q.before,q.before,roots_only,limit).fetch_all(&s.db).await?;
        rows.reverse();
        rows
    };
    let mut result = Vec::new();
    for row in rows {
        result.push(with_uploads(&s, row).await?);
    }
    Ok(Json(result))
}
#[utoipa::path(post,path="/channels/{id}/messages",params(("id"=String,Path)),request_body=CreateMessage,responses((status=200,body=Message)))]
pub(crate) async fn send(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<CreateMessage>,
) -> Result<Json<Message>> {
    if visible(&s, &a.user.id, &channel).await?.kind == ChannelKind::Voice {
        return Err(Error::bad("Voice rooms do not accept messages"));
    }
    if v.content.len() > 32000
        || (v.content.trim().is_empty() && v.upload_ids.is_empty())
        || v.upload_ids.len() > 10
    {
        return Err(Error::bad(
            "Message needs text or uploads; maximum 32000 bytes and 10 uploads",
        ));
    }
    let _guard = s.writes.lock().await;
    let id = s.id();
    let mut tx = s.db.begin().await?;
    let ctx = threads::Context {
        thread_id: v.thread_id.as_deref(),
        task_id: v.task_id.as_deref(),
        reply_to: v.reply_to.as_deref(),
        hint: None,
    };
    // Resolve first: a conflicting destination must fail before a thread is created
    // or an upload is attached to anything.
    let target = threads::resolve(&mut tx, &channel, &a.user.id, &ctx).await?;
    sqlx::query!(
        "INSERT INTO messages(id,channel_id,author_id,content,reply_to) VALUES(?,?,?,?,?)",
        id,
        channel,
        a.user.id,
        v.content,
        v.reply_to
    )
    .execute(&mut *tx)
    .await?;
    for upload in &v.upload_ids {
        if sqlx::query!("UPDATE uploads SET message_id=? WHERE id=? AND channel_id=? AND owner_id=? AND complete=1 AND message_id IS NULL",id,upload,channel,a.user.id).execute(&mut *tx).await?.rows_affected()!=1 {return Err(Error::bad("Upload must be complete, unattached, owned by you and in this channel"));}
    }
    activity::index_mentions(&mut tx, &id, &channel, &v.content).await?;
    // Placement runs after mention indexing so a mentioned user joins the thread.
    let thread = threads::apply(&mut tx, &s, &channel, &a.user.id, &id, target, &ctx).await?;
    tx.commit().await?;
    let msg = get_message(&s, &id).await?;
    let _ = s.events.send(Event::MessageCreated(msg.clone()));
    if let Some(thread) = &thread {
        if let Err(e) = threads::announce(&s, thread).await {
            tracing::error!(code = e.1, "thread metadata update failed");
        }
    }
    if let Err(e) = inbox::changed(&s, &msg.channel_id, Some(&msg)).await {
        tracing::error!(code = e.1, "inbox update failed");
    }
    Ok(Json(msg))
}
#[utoipa::path(patch,path="/messages/{id}",params(("id"=String,Path)),request_body=EditMessage,responses((status=200,body=Message)))]
pub(crate) async fn edit(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<EditMessage>,
) -> Result<Json<Message>> {
    let _guard = s.writes.lock().await;
    let old = get_message(&s, &id).await?;
    visible(&s, &a.user.id, &old.channel_id).await?;
    if old.author_id != a.user.id {
        return Err(Error::forbidden());
    }
    if v.content.len() > 32000
        || (v.content.trim().is_empty() && old.attachments.is_empty() && old.objects.is_empty())
    {
        return Err(Error::bad("Invalid message length"));
    }
    let mut tx = s.db.begin().await?;
    sqlx::query!(
        "UPDATE messages SET content=?,edited_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?",
        v.content,
        id
    )
    .execute(&mut *tx)
    .await?;
    activity::index_mentions(&mut tx, &id, &old.channel_id, &v.content).await?;
    tx.commit().await?;
    let msg = get_message(&s, &id).await?;
    let _ = s.events.send(Event::MessageEdited(msg.clone()));
    // Editing a root never rewrites the snapshot title, but the summary a client
    // holds should still reflect the conversation after any change to its messages.
    if let Some(thread) = threads::touched(&msg) {
        if let Err(e) = threads::announce(&s, thread).await {
            tracing::error!(code = e.1, "thread metadata update failed");
        }
    }
    if let Err(e) = inbox::changed(&s, &old.channel_id, None).await {
        tracing::error!(code = e.1, "inbox update failed");
    }
    Ok(Json(msg))
}
#[utoipa::path(delete,path="/messages/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn remove(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let _guard = s.writes.lock().await;
    let old = get_message(&s, &id).await?;
    visible(&s, &a.user.id, &old.channel_id).await?;
    if old.author_id != a.user.id && a.user.role != Role::Admin {
        return Err(Error::forbidden());
    }
    // Deleting a root would take its whole conversation with it, including other
    // people's replies and live objects. Resolve the thread instead.
    if old.thread.is_some() {
        return Err(Error::conflict(
            "This message starts a thread; resolve the thread instead of deleting it",
        ));
    }
    for object in &old.objects {
        if object.kind == "terminal" {
            if let Ok(t) = terminal::load(&s, &object.id).await {
                if t.id == object.id && t.ended_at.is_none() {
                    return Err(Error::conflict(
                        "End the terminal session before deleting its original card",
                    ));
                }
            }
        }
    }
    sqlx::query!("DELETE FROM messages WHERE id=?", id)
        .execute(&s.db)
        .await?;
    let _ = s.events.send(Event::MessageDeleted {
        id,
        channel_id: old.channel_id.clone(),
    });
    // Counts come from the reply rows, so the summary is already correct; clients
    // still need telling that it changed.
    if let Some(thread) = &old.thread_id {
        if let Err(e) = threads::announce(&s, thread).await {
            tracing::error!(code = e.1, "thread metadata update failed");
        }
    }
    if let Err(e) = inbox::changed(&s, &old.channel_id, None).await {
        tracing::error!(code = e.1, "inbox update failed");
    }
    Ok(StatusCode::NO_CONTENT)
}
