use crate::{auth::Auth, chat::visible, *};
use axum::extract::{Path, Query};

/// Shared metadata for one thread. Counts come from the reply rows themselves, so
/// deleting a reply cannot leave a cached total behind.
pub(crate) async fn summary<'a>(
    db: impl sqlx::SqliteExecutor<'a>,
    id: &str,
) -> Result<ThreadSummary> {
    let r = sqlx::query!(
        r#"SELECT t.id,t.channel_id,t.root_message_id,t.title,t.created_by,t.created_at,
            t.resolved_at as "resolved_at?",t.resolved_by as "resolved_by?",
            (SELECT count(*) FROM messages m WHERE m.thread_id=t.id) as "reply_count!: i64",
            (SELECT max(m.id) FROM messages m WHERE m.thread_id=t.id) as "last_reply_id?",
            coalesce((SELECT max(m.created_at) FROM messages m WHERE m.thread_id=t.id),t.created_at) as "last_activity_at!"
            FROM threads t WHERE t.id=?"#,
        id
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(Error::missing)?;
    Ok(ThreadSummary {
        id: r.id,
        channel_id: r.channel_id,
        root_message_id: r.root_message_id,
        title: r.title,
        created_by: r.created_by,
        created_at: r.created_at,
        resolved_at: r.resolved_at,
        resolved_by: r.resolved_by,
        reply_count: r.reply_count,
        last_reply_id: r.last_reply_id,
        last_activity_at: r.last_activity_at,
    })
}
/// The summary a root message carries, or None when the message roots nothing.
pub(crate) async fn for_root(s: &AppState, message: &str) -> Result<Option<ThreadSummary>> {
    let Some(id) = sqlx::query_scalar!("SELECT id FROM threads WHERE root_message_id=?", message)
        .fetch_optional(&s.db)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(summary(&s.db, &id).await?))
}
/// A thread the caller may see, authorized by its parent channel. Following is not
/// membership: everyone who can read the channel can read its threads.
async fn readable(s: &AppState, user: &str, id: &str) -> Result<ThreadSummary> {
    let thread = summary(&s.db, id).await?;
    visible(s, user, &thread.channel_id).await?;
    Ok(thread)
}
/// First nonempty line of the root, whitespace collapsed, at most 80 characters.
pub(crate) fn title_from(content: &str, hint: Option<&str>) -> String {
    let line = content
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|l| !l.is_empty());
    let chosen = match (line, hint) {
        (Some(line), _) => line,
        (None, Some(hint)) if !hint.trim().is_empty() => {
            hint.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        _ => "Conversation".into(),
    };
    // Truncate by characters: an 80-byte cut can split a multi-byte character.
    chosen.chars().take(80).collect()
}
fn check_title(value: &str) -> Result<String> {
    let title = value.trim();
    if !(1..=80).contains(&title.chars().count()) || title.chars().any(char::is_control) {
        return Err(Error::bad(
            "Title must be 1 to 80 characters without control characters",
        ));
    }
    Ok(title.into())
}
/// Refuse conversation context on a route that cannot honour it yet. Placing object
/// cards is a later PR; until it lands, accepting these fields and dropping them
/// would silently put a task's card somewhere its runner did not ask for.
pub(crate) fn unsupported_context(
    thread_id: Option<&str>,
    task_id: Option<&str>,
    reply_to: Option<&str>,
) -> Result<()> {
    if thread_id.is_some() || task_id.is_some() || reply_to.is_some() {
        return Err(Error::bad(
            "Thread context for this card is not supported yet",
        ));
    }
    Ok(())
}
/// A runner's job identity. Opaque: never lowercased, trimmed into a different
/// value, or inferred from the bot, token or elapsed time.
fn check_task(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(Error::bad(
            "Task ID must be 1 to 128 bytes without control characters",
        ));
    }
    Ok(())
}

/// Destination context supplied with a new message or object card.
pub(crate) struct Context<'a> {
    pub thread_id: Option<&'a str>,
    pub task_id: Option<&'a str>,
    pub reply_to: Option<&'a str>,
    /// Name to title a thread this message itself roots when it has no text of its
    /// own, such as an object-only card.
    pub hint: Option<&'a str>,
}
/// Where a new message belongs, decided before anything is written.
pub(crate) enum Target {
    Room,
    Join(String),
    /// Open a thread. `root` is an existing message, or None when the new message
    /// is itself the root.
    Open {
        root: Option<String>,
    },
}
/// A resolved destination together with what is already stored about it.
pub(crate) struct Placement {
    target: Target,
    /// A persisted task mapping already pointed here. It has been validated against
    /// every other supplied destination, so it must be preserved, not written again.
    mapped: bool,
}

/// Resolve the destination. Reads only: a conflict fails here, before a thread is
/// created or an upload is attached.
pub(crate) async fn resolve(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    channel: &str,
    author: &str,
    ctx: &Context<'_>,
) -> Result<Placement> {
    let mut found: Option<String> = None;
    if let Some(task) = ctx.task_id {
        check_task(task)?;
        // A persisted mapping is authoritative. A read error must abort rather than
        // look like a missing mapping and start a second thread for the same job.
        found = sqlx::query_scalar!(
            "SELECT thread_id FROM thread_tasks WHERE user_id=? AND channel_id=? AND task_id=?",
            author,
            channel,
            task
        )
        .fetch_optional(&mut **tx)
        .await?;
    }
    let mapped = found.is_some();
    if let Some(explicit) = ctx.thread_id {
        let owned = sqlx::query_scalar!(
            "SELECT id FROM threads WHERE id=? AND channel_id=?",
            explicit,
            channel
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| Error::bad("Thread must belong to this channel"))?;
        agree(&mut found, owned)?;
    }
    if let Some(reply) = ctx.reply_to {
        let row = sqlx::query!(
            r#"SELECT m.thread_id as "thread_id?",
                (SELECT t.id FROM threads t WHERE t.root_message_id=m.id) as "roots?"
                FROM messages m WHERE m.id=? AND m.channel_id=?"#,
            reply,
            channel
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| Error::bad("Reply must belong to this channel"))?;
        // A reply to a reply stays in the same thread; there is no nesting. An old
        // quoted reply is history, so its own reply_to chain is never walked.
        match row.thread_id.or(row.roots) {
            Some(thread) => agree(&mut found, thread)?,
            None if found.is_some() => {
                return Err(Error::conflict(
                    "Reply target is not part of the destination thread",
                ))
            }
            None => {
                return Ok(Placement {
                    target: Target::Open {
                        root: Some(reply.into()),
                    },
                    mapped,
                })
            }
        }
    }
    match found {
        Some(thread) => {
            if sqlx::query_scalar!(
                "SELECT count(*) FROM threads WHERE id=? AND resolved_at IS NOT NULL",
                thread
            )
            .fetch_one(&mut **tx)
            .await?
                != 0
            {
                return Err(Error::conflict(
                    "This thread is resolved; reopen it before posting",
                ));
            }
            Ok(Placement {
                target: Target::Join(thread),
                mapped,
            })
        }
        // A task's first post becomes a room root and opens its own thread. An
        // ordinary post with no context at all stays in the room.
        None if ctx.task_id.is_some() => Ok(Placement {
            target: Target::Open { root: None },
            mapped,
        }),
        None => Ok(Placement {
            target: Target::Room,
            mapped,
        }),
    }
}
fn agree(found: &mut Option<String>, candidate: String) -> Result<()> {
    match found {
        Some(existing) if *existing != candidate => Err(Error::conflict(
            "Conflicting thread destinations for this message",
        )),
        _ => {
            *found = Some(candidate);
            Ok(())
        }
    }
}

/// Apply a resolved destination once the message row exists. Returns the thread the
/// message joined or opened. The message must already be inserted: a thread rooted
/// at it references its ID.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    s: &AppState,
    channel: &str,
    author: &str,
    message: &str,
    placement: Placement,
    ctx: &Context<'_>,
) -> Result<Option<String>> {
    let Placement { target, mapped } = placement;
    let (thread, opened) = match target {
        Target::Room => return Ok(None),
        Target::Join(thread) => (thread, false),
        Target::Open { root } => {
            let id = s.id();
            let root = root.unwrap_or_else(|| message.into());
            let content = sqlx::query_scalar!("SELECT content FROM messages WHERE id=?", root)
                .fetch_one(&mut **tx)
                .await?;
            // The title describes the root. An existing object-only root is named by
            // its own card, never by whatever the new message happens to carry. A
            // brand new card root falls back to its supplied name, because its object
            // row may not be written until later in this same transaction.
            let card = sqlx::query_scalar!("SELECT name FROM objects WHERE message_id=?", root)
                .fetch_optional(&mut **tx)
                .await?;
            let hint = card
                .as_deref()
                .or_else(|| (root == message).then_some(ctx.hint).flatten());
            let title = title_from(&content, hint);
            sqlx::query!(
                "INSERT INTO threads(id,channel_id,root_message_id,title,created_by) VALUES(?,?,?,?,?)",
                id,
                channel,
                root,
                title,
                author
            )
            .execute(&mut **tx)
            .await?;
            (id, true)
        }
    };
    // The root stays in the main conversation, so only a reply carries thread_id.
    let root = sqlx::query_scalar!("SELECT root_message_id FROM threads WHERE id=?", thread)
        .fetch_one(&mut **tx)
        .await?;
    if root != message {
        sqlx::query!(
            "UPDATE messages SET thread_id=? WHERE id=?",
            thread,
            message
        )
        .execute(&mut **tx)
        .await?;
    }
    // A mapping resolve already loaded is the one being continued: rewriting it
    // would collide on its own key and could only ever store the same value.
    if let (Some(task), false) = (ctx.task_id, mapped) {
        sqlx::query!(
            "INSERT INTO thread_tasks(user_id,channel_id,task_id,thread_id) VALUES(?,?,?,?)",
            author,
            channel,
            task,
            thread
        )
        .execute(&mut **tx)
        .await?;
    }
    affiliate(tx, &thread, author, message, opened).await?;
    Ok(Some(thread))
}

/// Record who has joined this conversation, for the read model the next PR builds.
/// Call it after mention indexing, never from the indexer itself: that one also runs
/// for edits and for startup backfill, neither of which is anyone joining anything.
async fn affiliate(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    thread: &str,
    author: &str,
    message: &str,
    opened: bool,
) -> Result<()> {
    let mut users = vec![author.to_string()];
    if opened {
        users.push(
            sqlx::query_scalar!(
                "SELECT m.author_id FROM messages m JOIN threads t ON t.root_message_id=m.id WHERE t.id=?",
                thread
            )
            .fetch_one(&mut **tx)
            .await?,
        );
    }
    users.extend(
        sqlx::query_scalar!(
            "SELECT user_id FROM message_mentions WHERE message_id=? ORDER BY user_id",
            message
        )
        .fetch_all(&mut **tx)
        .await?,
    );
    users.sort();
    users.dedup();
    // Someone joining starts at the last reply before the message that brought them
    // in, so that message is unread without the backlog becoming new activity.
    let start = sqlx::query_scalar!(
        r#"SELECT coalesce((SELECT max(m.id) FROM messages m WHERE m.thread_id=t.id AND m.id<?),
            t.root_message_id) as "start!" FROM threads t WHERE t.id=?"#,
        message,
        thread
    )
    .fetch_one(&mut **tx)
    .await?;
    for user in users {
        // Deliberately no max(last_read_id) clause, unlike every read action: posting
        // through the CLI does not prove the author read the intervening messages, so
        // an existing position stays exactly where its owner left it. Replying or a
        // new-message mention does re-follow a thread somebody had unfollowed.
        sqlx::query!(
            "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following) VALUES(?,?,?,1)
             ON CONFLICT(user_id,thread_id) DO UPDATE SET following=1",
            user,
            thread,
            start
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Whether a thread belongs to this channel. Used where a caller has already been
/// authorized for the channel and must not learn anything about other channels.
pub(crate) async fn in_channel(s: &AppState, thread: &str, channel: &str) -> bool {
    sqlx::query_scalar!(
        "SELECT count(*) FROM threads WHERE id=? AND channel_id=?",
        thread,
        channel
    )
    .fetch_one(&s.db)
    .await
    .is_ok_and(|n| n == 1)
}
/// Broadcast committed metadata. Callers hold the write lock, so events stay in the
/// same order as the mutations that produced them.
pub(crate) async fn announce(s: &AppState, thread: &str) -> Result<()> {
    let _ = s.events.send(Event::ThreadUpdated {
        thread: summary(&s.db, thread).await?,
    });
    Ok(())
}
/// The thread a message belongs to or roots, for refreshing metadata after an edit
/// or a delete changes the reply rows.
pub(crate) fn touched(message: &Message) -> Option<&str> {
    match &message.thread_id {
        Some(id) => Some(id),
        None => message.thread.as_ref().map(|t| t.id.as_str()),
    }
}

#[utoipa::path(get,path="/threads/{id}/messages",params(("id"=String,Path),MessageQuery),responses((status=200,body=Vec<Message>),(status=404,body=ApiError)))]
pub(crate) async fn replies(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    query: std::result::Result<Query<MessageQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<Message>>> {
    let Query(q) = query.map_err(|_| Error::bad("Invalid message query"))?;
    // Every message here is a reply, so a main-conversation filter is not a
    // narrower view of this endpoint: it is a request for the wrong one.
    if q.roots_only == Some(true) {
        return Err(Error::bad("roots_only does not apply to a thread"));
    }
    readable(&s, &a.user.id, &id).await?;
    if q.before.is_some() && q.after.is_some() {
        return Err(Error::bad("Use before or after, not both"));
    }
    let limit = q.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(Error::bad("Limit must be 1-200"));
    }
    let limit = limit as i64;
    let rows = if q.after.is_some() {
        sqlx::query_as!(messages::DbMessage,r#"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at,thread_id as "thread_id?" FROM messages WHERE thread_id=? AND id>? ORDER BY id ASC LIMIT ?"#,id,q.after,limit).fetch_all(&s.db).await?
    } else {
        let mut rows=sqlx::query_as!(messages::DbMessage,r#"SELECT id,channel_id,author_id,content,reply_to,created_at,edited_at,thread_id as "thread_id?" FROM messages WHERE thread_id=? AND (? IS NULL OR id<?) ORDER BY id DESC LIMIT ?"#,id,q.before,q.before,limit).fetch_all(&s.db).await?;
        rows.reverse();
        rows
    };
    let mut result = Vec::new();
    for row in rows {
        result.push(messages::with_uploads(&s, row).await?);
    }
    Ok(Json(result))
}

#[utoipa::path(patch,path="/threads/{id}",params(("id"=String,Path)),request_body=UpdateThread,responses((status=200,body=ThreadSummary),(status=404,body=ApiError)))]
pub(crate) async fn update(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<UpdateThread>,
) -> Result<Json<ThreadSummary>> {
    let _guard = s.writes.lock().await;
    // Any member who can see the channel may rename, resolve or reopen its threads.
    let thread = readable(&s, &a.user.id, &id).await?;
    if let Some(title) = &v.title {
        let title = check_title(title)?;
        sqlx::query!("UPDATE threads SET title=? WHERE id=?", title, id)
            .execute(&s.db)
            .await?;
    }
    match v.resolved {
        // Resolving an already resolved thread keeps the original actor and time:
        // only an actual reopen clears them.
        Some(true) if thread.resolved_at.is_none() => {
            sqlx::query!("UPDATE threads SET resolved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),resolved_by=? WHERE id=?",a.user.id,id).execute(&s.db).await?;
        }
        Some(false) => {
            sqlx::query!(
                "UPDATE threads SET resolved_at=NULL,resolved_by=NULL WHERE id=?",
                id
            )
            .execute(&s.db)
            .await?;
        }
        _ => {}
    }
    let thread = summary(&s.db, &id).await?;
    let _ = s.events.send(Event::ThreadUpdated {
        thread: thread.clone(),
    });
    Ok(Json(thread))
}
