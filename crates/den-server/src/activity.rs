use crate::{auth::Auth, chat::visible, *};
use axum::extract::{Path, Query};
use std::collections::{BTreeMap, BTreeSet};

#[utoipa::path(get,path="/messages/{id}",params(("id"=String,Path)),responses((status=200,body=Message)))]
pub(crate) async fn message(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<Message>> {
    let message = messages::get_message(&s, &id).await?;
    visible(&s, &a.user.id, &message.channel_id).await?;
    Ok(Json(message))
}
pub(crate) async fn reactions(s: &AppState, id: &str) -> Result<Vec<Reaction>> {
    let rows = sqlx::query!(
        "SELECT emoji,user_id FROM reactions WHERE message_id=? ORDER BY emoji,user_id",
        id
    )
    .fetch_all(&s.db)
    .await?;
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for row in rows {
        groups.entry(row.emoji).or_default().push(row.user_id);
    }
    Ok(groups
        .into_iter()
        .map(|(emoji, user_ids)| Reaction { emoji, user_ids })
        .collect())
}
#[utoipa::path(put,path="/messages/{id}/reactions",params(("id"=String,Path)),request_body=SetReaction,responses((status=200,body=Vec<Reaction>)))]
pub(crate) async fn react(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<SetReaction>,
) -> Result<Json<Vec<Reaction>>> {
    change(&s, &a, &id, v, true).await
}
#[utoipa::path(delete,path="/messages/{id}/reactions",params(("id"=String,Path)),request_body=SetReaction,responses((status=200,body=Vec<Reaction>)))]
pub(crate) async fn unreact(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<SetReaction>,
) -> Result<Json<Vec<Reaction>>> {
    change(&s, &a, &id, v, false).await
}
async fn change(
    s: &AppState,
    a: &Auth,
    id: &str,
    v: SetReaction,
    add: bool,
) -> Result<Json<Vec<Reaction>>> {
    if v.emoji.is_empty()
        || v.emoji.len() > 64
        || v.emoji.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(Error::bad(
            "Reaction must contain 1-64 bytes without whitespace",
        ));
    }
    let _guard = s.writes.lock().await;
    let message = messages::get_message(s, id).await?;
    visible(s, &a.user.id, &message.channel_id).await?;
    if add {
        sqlx::query!(
            "INSERT OR IGNORE INTO reactions(message_id,user_id,emoji) VALUES(?,?,?)",
            id,
            a.user.id,
            v.emoji
        )
        .execute(&s.db)
        .await?;
    } else {
        sqlx::query!(
            "DELETE FROM reactions WHERE message_id=? AND user_id=? AND emoji=?",
            id,
            a.user.id,
            v.emoji
        )
        .execute(&s.db)
        .await?;
    }
    let reactions = reactions(s, id).await?;
    let _ = s.events.send(Event::ReactionsUpdated {
        message_id: id.into(),
        channel_id: message.channel_id,
        reactions: reactions.clone(),
    });
    Ok(Json(reactions))
}

#[utoipa::path(get,path="/search/messages",params(SearchMessages),responses((status=200,body=Vec<Message>)))]
pub(crate) async fn search(
    State(s): State<AppState>,
    a: Auth,
    query: std::result::Result<Query<SearchMessages>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<Message>>> {
    let Query(q) = query.map_err(|_| Error::bad("Invalid search query"))?;
    if let Some(channel) = &q.channel_id {
        visible(&s, &a.user.id, channel).await?;
    }
    let limit = q.limit.unwrap_or(50);
    let terms = q.q.split_whitespace().collect::<Vec<_>>();
    if q.q.len() > 200 || terms.is_empty() || terms.len() > 20 || !(1..=100).contains(&limit) {
        return Err(Error::bad(
            "Search needs 1-200 bytes, at most 20 terms, and limit 1-100",
        ));
    }
    // Literal terms joined with AND. Never interpret user input as FTS operators.
    let expression = terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");
    let limit = limit as i64;
    let ids=sqlx::query_scalar!(r#"SELECT m.id FROM messages m JOIN channels c ON c.id=m.channel_id
        WHERE m.rowid IN (SELECT rowid FROM message_search WHERE message_search MATCH ?)
        AND (c.kind='text' OR EXISTS(SELECT 1 FROM channel_members cm WHERE cm.channel_id=c.id AND cm.user_id=?))
        AND (? IS NULL OR c.id=?) AND (? IS NULL OR m.id<?) ORDER BY m.id DESC LIMIT ?"#,
        expression,a.user.id,q.channel_id,q.channel_id,q.before,q.before,limit).fetch_all(&s.db).await?;
    let mut messages = Vec::new();
    for id in ids {
        messages.push(messages::get_message(&s, &id).await?);
    }
    Ok(Json(messages))
}
fn mention_names(content: &str) -> BTreeSet<String> {
    use pulldown_cmark::{Event as Markdown, Parser, Tag, TagEnd};
    let mut names = BTreeSet::new();
    let mut in_code = false;
    for event in Parser::new(content) {
        match event {
            Markdown::Start(Tag::CodeBlock(_)) => in_code = true,
            Markdown::End(TagEnd::CodeBlock) => in_code = false,
            Markdown::Text(text) if !in_code => {
                let bytes = text.as_bytes();
                for (i, b) in bytes.iter().enumerate() {
                    if *b != b'@'
                        || (i > 0
                            && (bytes[i - 1].is_ascii_alphanumeric()
                                || matches!(bytes[i - 1], b'_' | b'@')))
                    {
                        continue;
                    }
                    let name = text[i + 1..]
                        .bytes()
                        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                        .collect::<Vec<_>>();
                    if (3..=32).contains(&name.len()) {
                        names.insert(
                            String::from_utf8(name)
                                .expect("ASCII username")
                                .to_ascii_lowercase(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    names
}
pub(crate) async fn index_mentions(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
    channel: &str,
    content: &str,
) -> Result<()> {
    sqlx::query!("DELETE FROM message_mentions WHERE message_id=?", id)
        .execute(&mut **tx)
        .await?;
    for name in mention_names(content) {
        sqlx::query!(r#"INSERT INTO message_mentions(message_id,user_id)
            SELECT ?,u.id FROM users u JOIN channels c ON c.id=? WHERE u.username=?
            AND (c.kind='text' OR EXISTS(SELECT 1 FROM channel_members cm WHERE cm.channel_id=c.id AND cm.user_id=u.id))"#,id,channel,name).execute(&mut **tx).await?;
    }
    sqlx::query!("UPDATE messages SET mentions_indexed=1 WHERE id=?", id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(crate) async fn backfill(s: &AppState) -> Result<()> {
    let rows = sqlx::query!("SELECT id,channel_id,content FROM messages WHERE mentions_indexed=0")
        .fetch_all(&s.db)
        .await?;
    for row in rows {
        let mut tx = s.db.begin().await?;
        index_mentions(&mut tx, &row.id, &row.channel_id, &row.content).await?;
        tx.commit().await?;
    }
    Ok(())
}
