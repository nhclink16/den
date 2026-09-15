use crate::{auth::Auth, chat::visible, *};
use axum::extract::{Path, Query};

pub(crate) struct DbMessage {
    id: String,
    channel_id: String,
    author_id: String,
    content: String,
    reply_to: Option<String>,
    created_at: String,
    edited_at: Option<String>,
}
pub(crate) async fn with_uploads(s: &AppState, v: DbMessage) -> Result<Message> {
    let attachments=sqlx::query_as!(Upload,"SELECT id,channel_id,filename,content_type,size,offset,complete as \"complete: bool\", CASE WHEN thumbnail_ready=1 THEN '/uploads/'||id||'/thumbnail' END as \"thumbnail_url?: String\" FROM uploads WHERE message_id=? ORDER BY id",v.id).fetch_all(&s.db).await?;
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
        thread_id: None,
        thread: None,
    })
}
pub(crate) async fn get_message(s: &AppState, id: &str) -> Result<Message> {
    with_uploads(s,sqlx::query_as!(DbMessage,"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at FROM messages WHERE id=?",id).fetch_one(&s.db).await?).await
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
    let rows = if q.after.is_some() {
        sqlx::query_as!(DbMessage,"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at FROM messages WHERE channel_id=? AND id>? ORDER BY id ASC LIMIT ?",id,q.after,limit).fetch_all(&s.db).await?
    } else {
        let mut rows=sqlx::query_as!(DbMessage,"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at FROM messages WHERE channel_id=? AND (? IS NULL OR id<?) ORDER BY id DESC LIMIT ?",id,q.before,q.before,limit).fetch_all(&s.db).await?;
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
    if let Some(reply) = &v.reply_to {
        if sqlx::query_scalar!(
            "SELECT count(*) FROM messages WHERE id=? AND channel_id=?",
            reply,
            channel
        )
        .fetch_one(&mut *tx)
        .await?
            != 1
        {
            return Err(Error::bad("Reply must belong to this channel"));
        }
    }
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
    tx.commit().await?;
    let msg = get_message(&s, &id).await?;
    let _ = s.events.send(Event::MessageCreated(msg.clone()));
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
    if let Err(e) = inbox::changed(&s, &old.channel_id, None).await {
        tracing::error!(code = e.1, "inbox update failed");
    }
    Ok(StatusCode::NO_CONTENT)
}
