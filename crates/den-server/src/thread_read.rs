use crate::{auth::Auth, chat::visible, *};
use axum::extract::{Path, Query};

/// One user's position and counts in one thread. A thread that is not relevant to
/// this user reports zero counts while keeping the position it has saved, so
/// unfollowing hides activity without losing where somebody had read to.
pub(crate) async fn state(s: &AppState, user: &str, thread: &str) -> Result<ThreadReadState> {
    let row = sqlx::query!(
        r#"WITH pos AS (
            SELECT t.id AS thread_id, t.channel_id AS channel_id, p.last_read_id AS last_read_id,
                coalesce(p.following,0) AS following,
                (coalesce(p.following,0)=1 OR c.kind='dm'
                    OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=t.channel_id AND cs.user_id=?)) AS relevant
            FROM threads t JOIN channels c ON c.id=t.channel_id
            LEFT JOIN thread_read_state p ON p.thread_id=t.id AND p.user_id=?
            WHERE t.id=?
        ), unread AS (
            SELECT m.id AS id,
                EXISTS(SELECT 1 FROM message_mentions mm WHERE mm.message_id=m.id AND mm.user_id=?) AS mentioned
            FROM messages m JOIN pos p ON p.thread_id=m.thread_id
            WHERE m.author_id<>? AND p.relevant
                AND (p.last_read_id IS NULL OR m.id>p.last_read_id)
        )
        SELECT p.channel_id as "channel_id!", p.last_read_id as "last_read_id?",
            p.following as "following!: bool",
            (SELECT count(*) FROM unread) as "unread_count!: i64",
            (SELECT count(*) FROM unread WHERE mentioned) as "mention_count!: i64",
            (SELECT count(*) FROM unread u WHERE
                (coalesce((SELECT mentions FROM notification_preferences WHERE user_id=?),1)=1 AND u.mentioned)
                OR (coalesce((SELECT dms FROM notification_preferences WHERE user_id=?),1)=1
                    AND EXISTS(SELECT 1 FROM channels c WHERE c.id=p.channel_id AND c.kind='dm'))
                OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=p.channel_id AND cs.user_id=?)
            ) as "notification_count!: i64"
        FROM pos p"#,
        user, user, thread, user, user, user, user, user
    )
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(Error::missing)?;
    Ok(ThreadReadState {
        thread_id: thread.into(),
        channel_id: row.channel_id,
        last_read_id: row.last_read_id,
        following: row.following,
        unread_count: row.unread_count,
        mention_count: row.mention_count,
        notification_count: row.notification_count,
    })
}
async fn view(s: &AppState, user: &str, thread: &str) -> Result<ThreadView> {
    Ok(ThreadView {
        thread: threads::summary(&s.db, thread).await?,
        read_state: state(s, user, thread).await?,
    })
}
/// A thread the caller may see, authorized by its parent channel.
async fn readable(s: &AppState, user: &str, thread: &str) -> Result<ThreadSummary> {
    let summary = threads::summary(&s.db, thread).await?;
    visible(s, user, &summary.channel_id).await?;
    Ok(summary)
}
#[utoipa::path(get,path="/channels/{id}/threads",params(("id"=String,Path),ThreadQuery),responses((status=200,body=Vec<ThreadView>),(status=404,body=ApiError)))]
pub(crate) async fn list(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    query: std::result::Result<Query<ThreadQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<ThreadView>>> {
    let Query(q) = query.map_err(|_| Error::bad("Invalid thread query"))?;
    visible(&s, &a.user.id, &channel).await?;
    let limit = q.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(Error::bad("Limit must be 1-200"));
    }
    let limit = limit as i64;
    // The filters AND: absent resolved means every thread, false the open strip,
    // true resolved history. unread_only asks for exactly the threads contributing
    // unread to this caller's channel total, so it repeats that predicate rather
    // than inventing a second, subtly different one.
    let unread_only = q.unread_only.unwrap_or(false);
    let ids = sqlx::query_scalar!(
        r#"SELECT t.id FROM threads t JOIN channels c ON c.id=t.channel_id
        LEFT JOIN thread_read_state p ON p.thread_id=t.id AND p.user_id=?
        WHERE t.channel_id=?
            AND (? IS NULL OR (t.resolved_at IS NOT NULL)=?)
            AND (? IS NULL OR t.id<?)
            AND (? = 0 OR EXISTS(
                SELECT 1 FROM messages m WHERE m.thread_id=t.id AND m.author_id<>?
                    AND (p.last_read_id IS NULL OR m.id>p.last_read_id)
                    AND (coalesce(p.following,0)=1 OR c.kind='dm'
                        OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=t.channel_id AND cs.user_id=?))))
        ORDER BY t.id DESC LIMIT ?"#,
        a.user.id, channel, q.resolved, q.resolved, q.before, q.before, unread_only, a.user.id, a.user.id, limit
    )
    .fetch_all(&s.db)
    .await?;
    let mut result = Vec::new();
    for id in ids {
        result.push(view(&s, &a.user.id, &id).await?);
    }
    Ok(Json(result))
}

#[utoipa::path(get,path="/threads/{id}",params(("id"=String,Path)),responses((status=200,body=ThreadView),(status=404,body=ApiError)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<ThreadView>> {
    readable(&s, &a.user.id, &id).await?;
    Ok(Json(view(&s, &a.user.id, &id).await?))
}

#[utoipa::path(put,path="/threads/{id}/read",params(("id"=String,Path)),request_body=MarkThreadRead,responses((status=200,body=ThreadReadState),(status=404,body=ApiError)))]
pub(crate) async fn mark_read(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<MarkThreadRead>,
) -> Result<Json<ThreadReadState>> {
    let thread = readable(&s, &a.user.id, &id).await?;
    let _guard = s.writes.lock().await;
    // A reply of this thread, or its root as the watermark for a thread with no
    // replies yet. Anything else is refused without creating a row.
    if v.message_id != thread.root_message_id
        && sqlx::query_scalar!(
            "SELECT count(*) FROM messages WHERE id=? AND thread_id=?",
            v.message_id,
            id
        )
        .fetch_one(&s.db)
        .await?
            != 1
    {
        return Err(Error::bad(
            "Read marker must identify a reply in this thread or its root",
        ));
    }
    // Reading is not joining: a new row is explicitly following=0, and an existing
    // affiliation is left exactly as its owner set it.
    sqlx::query!(
        "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following) VALUES(?,?,?,0)
         ON CONFLICT(user_id,thread_id) DO UPDATE SET last_read_id=CASE
            WHEN thread_read_state.last_read_id IS NULL OR thread_read_state.last_read_id<excluded.last_read_id
            THEN excluded.last_read_id ELSE thread_read_state.last_read_id END",
        a.user.id,
        id,
        v.message_id
    )
    .execute(&s.db)
    .await?;
    let state = state(&s, &a.user.id, &id).await?;
    let _ = s.events.send(Event::ThreadReadStateUpdated {
        user_id: a.user.id.clone(),
        state: state.clone(),
    });
    // The channel total changed too, and it arrives on its own event so the thread
    // response can stay strictly within its own scope.
    let _ = s.events.send(Event::ReadStateUpdated {
        user_id: a.user.id.clone(),
        state: inbox::state(&s, &a.user.id, &thread.channel_id).await?,
    });
    Ok(Json(state))
}

#[utoipa::path(put,path="/threads/{id}/follow",params(("id"=String,Path)),request_body=FollowThread,responses((status=200,body=ThreadReadState),(status=404,body=ApiError)))]
pub(crate) async fn follow(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<FollowThread>,
) -> Result<Json<ThreadReadState>> {
    let thread = readable(&s, &a.user.id, &id).await?;
    let _guard = s.writes.lock().await;
    if v.following {
        // Following joins from now: it is not a request to be told about history.
        let tail = sqlx::query_scalar!(
            r#"SELECT coalesce(max(m.id),?) as "tail!" FROM messages m WHERE m.thread_id=?"#,
            thread.root_message_id,
            id
        )
        .fetch_one(&s.db)
        .await?;
        sqlx::query!(
            "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following) VALUES(?,?,?,1)
             ON CONFLICT(user_id,thread_id) DO UPDATE SET following=1,last_read_id=CASE
                WHEN thread_read_state.last_read_id IS NULL OR thread_read_state.last_read_id<excluded.last_read_id
                THEN excluded.last_read_id ELSE thread_read_state.last_read_id END",
            a.user.id,
            id,
            tail
        )
        .execute(&s.db)
        .await?;
    } else {
        // Unfollowing keeps the position: it stops counting, it does not forget.
        sqlx::query!(
            "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following) VALUES(?,?,NULL,0)
             ON CONFLICT(user_id,thread_id) DO UPDATE SET following=0",
            a.user.id,
            id
        )
        .execute(&s.db)
        .await?;
    }
    let state = state(&s, &a.user.id, &id).await?;
    let _ = s.events.send(Event::ThreadReadStateUpdated {
        user_id: a.user.id.clone(),
        state: state.clone(),
    });
    let _ = s.events.send(Event::ReadStateUpdated {
        user_id: a.user.id.clone(),
        state: inbox::state(&s, &a.user.id, &thread.channel_id).await?,
    });
    Ok(Json(state))
}
