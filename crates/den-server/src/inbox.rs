use crate::{auth::Auth, chat::visible, *};
use axum::extract::Path;

pub(crate) async fn preferences(s: &AppState, user: &str) -> Result<NotificationPreferences> {
    let row=sqlx::query!("SELECT mentions as \"mentions: bool\",dms as \"dms: bool\" FROM notification_preferences WHERE user_id=?",user).fetch_optional(&s.db).await?;
    let subscribed_channel_ids = sqlx::query_scalar!(
        "SELECT channel_id FROM channel_subscriptions WHERE user_id=? ORDER BY channel_id",
        user
    )
    .fetch_all(&s.db)
    .await?;
    Ok(NotificationPreferences {
        mentions: row.as_ref().is_none_or(|r| r.mentions),
        dms: row.as_ref().is_none_or(|r| r.dms),
        subscribed_channel_ids,
    })
}
#[utoipa::path(get,path="/users/me/notification-preferences",responses((status=200,body=NotificationPreferences)))]
pub(crate) async fn get_preferences(
    State(s): State<AppState>,
    a: Auth,
) -> Result<Json<NotificationPreferences>> {
    Ok(Json(preferences(&s, &a.user.id).await?))
}
#[utoipa::path(put,path="/users/me/notification-preferences",request_body=NotificationPreferences,responses((status=200,body=NotificationPreferences)))]
pub(crate) async fn put_preferences(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(mut v): ApiJson<NotificationPreferences>,
) -> Result<Json<NotificationPreferences>> {
    if v.subscribed_channel_ids.len() > 1000 {
        return Err(Error::bad("Too many subscriptions"));
    }
    v.subscribed_channel_ids.sort();
    v.subscribed_channel_ids.dedup();
    for channel in &v.subscribed_channel_ids {
        visible(&s, &a.user.id, channel).await?;
    }
    let _guard = s.writes.lock().await;
    let mut tx = s.db.begin().await?;
    // First statement is a write, so this transaction never begins as a reader and
    // then has to upgrade.
    sqlx::query!("INSERT INTO notification_preferences(user_id,mentions,dms) VALUES(?,?,?) ON CONFLICT(user_id) DO UPDATE SET mentions=excluded.mentions,dms=excluded.dms",a.user.id,v.mentions,v.dms).execute(&mut *tx).await?;
    // Which channels are newly subscribed has to be read before the wholesale
    // rewrite below erases the evidence. Toggling an unrelated switch, reordering
    // the same set or unsubscribing therefore advances nothing.
    let existing = sqlx::query_scalar!(
        "SELECT channel_id FROM channel_subscriptions WHERE user_id=?",
        a.user.id
    )
    .fetch_all(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM channel_subscriptions WHERE user_id=?",
        a.user.id
    )
    .execute(&mut *tx)
    .await?;
    for channel in &v.subscribed_channel_ids {
        sqlx::query!(
            "INSERT INTO channel_subscriptions(user_id,channel_id) VALUES(?,?)",
            a.user.id,
            channel
        )
        .execute(&mut *tx)
        .await?;
    }
    for channel in v
        .subscribed_channel_ids
        .iter()
        .filter(|c| !existing.contains(c))
    {
        // Subscribing is not a request to be told about history, so a thread that
        // was irrelevant until now starts at its current tail. Threads already
        // contributing unread are skipped: a followed thread, and every thread of a
        // DM, would lose real unread if they were advanced. A following=0 row left
        // by an earlier flat Mark all read does advance, with a NULL-safe maximum,
        // because INSERT OR IGNORE would resurrect everything since that marker.
        // Resolved threads are included; resolution changes no read position.
        sqlx::query!(
            r#"INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following)
            SELECT ?,t.id,coalesce((SELECT max(m.id) FROM messages m WHERE m.thread_id=t.id),t.root_message_id),0
            FROM threads t JOIN channels c ON c.id=t.channel_id
            WHERE t.channel_id=? AND c.kind<>'dm'
                AND NOT EXISTS(SELECT 1 FROM thread_read_state p WHERE p.thread_id=t.id AND p.user_id=? AND p.following=1)
            ON CONFLICT(user_id,thread_id) DO UPDATE SET last_read_id=CASE
                WHEN thread_read_state.last_read_id IS NULL OR thread_read_state.last_read_id<excluded.last_read_id
                THEN excluded.last_read_id ELSE thread_read_state.last_read_id END"#,
            a.user.id,
            channel,
            a.user.id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let _ = s.events.send(Event::NotificationPreferencesUpdated {
        user_id: a.user.id.clone(),
        preferences: v.clone(),
    });
    // Every switch here can change what a conversation contributes, not only the
    // subscription set: turning dms off silences a DM's threads whose channel never
    // moved, and turning mentions off silences a followed thread the same way. So
    // this refreshes every conversation the user can see rather than only the
    // channels that were added or removed. Computing a state reads only — it
    // creates no row and advances no position.
    for state in states(&s, &a.user.id).await? {
        for thread in threads_of(&s, &state.channel_id).await? {
            let _ = s.events.send(Event::ThreadReadStateUpdated {
                user_id: a.user.id.clone(),
                state: thread_read::state(&s, &a.user.id, &thread).await?,
            });
        }
        let _ = s.events.send(Event::ReadStateUpdated {
            user_id: a.user.id.clone(),
            state,
        });
    }
    Ok(Json(v))
}
pub(crate) async fn state(s: &AppState, user: &str, channel: &str) -> Result<ChannelReadState> {
    // One statement, so every count in a payload describes the same snapshot. The
    // candidate set names each unread message once: a main-conversation message is
    // judged against the channel position, a reply against its own thread's, and a
    // message is one or the other, never both.
    //
    // A thread with no saved row is a static floor at "nothing read". It must never
    // fall back to the channel cursor, or reading a newer root would silently
    // acknowledge a collapsed conversation.
    //
    // Relevance is following, or belonging to the parent DM, or subscribing to the
    // parent channel. It is not permission, and notification switches decide alerts
    // rather than whether a conversation is unread.
    let row = sqlx::query!(
        r#"WITH unread AS (
            SELECT m.id AS id,
                EXISTS(SELECT 1 FROM message_mentions mm WHERE mm.message_id=m.id AND mm.user_id=?) AS mentioned
            FROM messages m
            LEFT JOIN read_state r ON r.channel_id=m.channel_id AND r.user_id=?
            WHERE m.channel_id=? AND m.thread_id IS NULL AND m.author_id<>?
                AND (r.last_read_id IS NULL OR m.id>r.last_read_id)
            UNION ALL
            SELECT m.id AS id,
                EXISTS(SELECT 1 FROM message_mentions mm WHERE mm.message_id=m.id AND mm.user_id=?) AS mentioned
            FROM messages m JOIN threads t ON t.id=m.thread_id JOIN channels c ON c.id=t.channel_id
            LEFT JOIN thread_read_state s ON s.thread_id=t.id AND s.user_id=?
            WHERE t.channel_id=? AND m.author_id<>?
                AND (s.last_read_id IS NULL OR m.id>s.last_read_id)
                AND (coalesce(s.following,0)=1 OR c.kind='dm'
                    OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=t.channel_id AND cs.user_id=?))
        )
        SELECT (SELECT last_read_id FROM read_state WHERE user_id=? AND channel_id=?) as "last_read_id?",
            (SELECT count(*) FROM unread) as "unread_count!: i64",
            (SELECT count(*) FROM unread WHERE mentioned) as "mention_count!: i64",
            (SELECT count(*) FROM unread u WHERE
                (coalesce((SELECT mentions FROM notification_preferences WHERE user_id=?),1)=1 AND u.mentioned)
                OR (coalesce((SELECT dms FROM notification_preferences WHERE user_id=?),1)=1
                    AND EXISTS(SELECT 1 FROM channels c WHERE c.id=? AND c.kind='dm'))
                OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=? AND cs.user_id=?)
            ) as "notification_count!: i64""#,
        user, user, channel, user,
        user, user, channel, user, user,
        user, channel,
        user, user, channel, channel, user
    )
    .fetch_one(&s.db)
    .await?;
    Ok(ChannelReadState {
        channel_id: channel.into(),
        last_read_id: row.last_read_id,
        unread_count: row.unread_count,
        mention_count: row.mention_count,
        notification_count: row.notification_count,
    })
}
async fn states(s: &AppState, user: &str) -> Result<Vec<ChannelReadState>> {
    let channels=sqlx::query_scalar!("SELECT id FROM channels WHERE kind='text' OR EXISTS(SELECT 1 FROM channel_members WHERE channel_id=channels.id AND user_id=?) ORDER BY id",user).fetch_all(&s.db).await?;
    let mut result = Vec::new();
    for channel in channels {
        result.push(state(s, user, &channel).await?);
    }
    Ok(result)
}
#[utoipa::path(get,path="/users/me/read-state",responses((status=200,body=Vec<ChannelReadState>)))]
pub(crate) async fn read_states(
    State(s): State<AppState>,
    a: Auth,
) -> Result<Json<Vec<ChannelReadState>>> {
    Ok(Json(states(&s, &a.user.id).await?))
}
#[utoipa::path(put,path="/channels/{id}/read",params(("id"=String,Path)),request_body=MarkRead,responses((status=200,body=ChannelReadState)))]
pub(crate) async fn mark_read(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<MarkRead>,
) -> Result<Json<ChannelReadState>> {
    visible(&s, &a.user.id, &channel).await?;
    let _guard = s.writes.lock().await;
    // Validate before opening the transaction, so the transaction's first statement
    // is a write and never has to upgrade from a read it began with.
    let marker = sqlx::query!(
        r#"SELECT (thread_id IS NULL) as "root!: bool" FROM messages WHERE id=? AND channel_id=?"#,
        v.message_id,
        channel
    )
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| Error::bad("Read marker must identify a message in this channel"))?;
    if v.roots_only && !marker.root {
        return Err(Error::bad(
            "A main-conversation read marker cannot be a thread reply",
        ));
    }
    let mut tx = s.db.begin().await?;
    sqlx::query!("INSERT INTO read_state(user_id,channel_id,last_read_id) VALUES(?,?,?) ON CONFLICT(user_id,channel_id) DO UPDATE SET last_read_id=max(read_state.last_read_id,excluded.last_read_id)",a.user.id,channel,v.message_id).execute(&mut *tx).await?;
    if !v.roots_only {
        // The default is a flat acknowledgement, whether it came from an old client
        // showing one timeline or from an explicit room-wide Mark all read. Every
        // thread of the channel advances, but only as far as the marker the caller
        // supplied: a reply that crossed the request stays unread. Affiliations are
        // untouched and rows created here are explicitly following=0.
        sqlx::query!(
            "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following)
             SELECT ?,t.id,?,0 FROM threads t WHERE t.channel_id=?
             ON CONFLICT(user_id,thread_id) DO UPDATE SET last_read_id=CASE
                WHEN thread_read_state.last_read_id IS NULL OR thread_read_state.last_read_id<excluded.last_read_id
                THEN excluded.last_read_id ELSE thread_read_state.last_read_id END",
            a.user.id,
            v.message_id,
            channel
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let state = state(&s, &a.user.id, &channel).await?;
    let _ = s.events.send(Event::ReadStateUpdated {
        user_id: a.user.id.clone(),
        state: state.clone(),
    });
    if !v.roots_only {
        for thread in threads_of(&s, &channel).await? {
            let _ = s.events.send(Event::ThreadReadStateUpdated {
                user_id: a.user.id.clone(),
                state: thread_read::state(&s, &a.user.id, &thread).await?,
            });
        }
    }
    Ok(Json(state))
}
pub(crate) async fn threads_of(s: &AppState, channel: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM threads WHERE channel_id=? ORDER BY id",
        channel
    )
    .fetch_all(&s.db)
    .await?)
}
/// Everyone who can see this channel. Read events fan out over this set rather than
/// over the users who happen to have a saved thread row: DM membership or a channel
/// subscription already makes replies relevant with no row in existence.
pub(crate) async fn audience(s: &AppState, channel: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT u.id FROM users u JOIN channels c ON c.id=? WHERE c.kind='text' OR EXISTS(SELECT 1 FROM channel_members cm WHERE cm.channel_id=c.id AND cm.user_id=u.id)",channel).fetch_all(&s.db).await?)
}
/// Whether this exact message is currently unread AND relevant for this user, by the
/// same rule the counts use. A reply is judged against its own thread's position and
/// relevance; the channel cursor decides nothing about it. Reading the room therefore
/// neither raises nor suppresses an alert for a collapsed conversation.
async fn eligible(s: &AppState, user: &str, message: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM messages m JOIN channels c ON c.id=m.channel_id
            LEFT JOIN read_state r ON r.channel_id=m.channel_id AND r.user_id=?
            LEFT JOIN thread_read_state p ON p.thread_id=m.thread_id AND p.user_id=?
            WHERE m.id=? AND m.author_id<>?
                AND CASE WHEN m.thread_id IS NULL
                    THEN (r.last_read_id IS NULL OR m.id>r.last_read_id)
                    ELSE (p.last_read_id IS NULL OR m.id>p.last_read_id)
                        AND (coalesce(p.following,0)=1 OR c.kind='dm'
                            OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=m.channel_id AND cs.user_id=?))
                END
        ) as "eligible!: bool""#,
        user, user, message, user, user
    )
    .fetch_one(&s.db)
    .await?)
}
// Called after committed message mutations while the message write lock is held.
// `thread` is carried separately from `message` because an edit or a delete passes
// None for the message to suppress alerts while still needing to refresh the
// conversation it touched — including when the deleted reply was the last one and
// there is no message left to look the thread up from.
pub(crate) async fn changed(
    s: &AppState,
    channel: &str,
    thread: Option<&str>,
    message: Option<&Message>,
) -> Result<()> {
    for user in audience(s, channel).await? {
        if let Some(thread) = thread {
            let _ = s.events.send(Event::ThreadReadStateUpdated {
                user_id: user.clone(),
                state: thread_read::state(s, &user, thread).await?,
            });
        }
        let state = state(s, &user, channel).await?;
        if let Some(message) = message {
            if eligible(s, &user, &message.id).await? {
                let prefs = preferences(s, &user).await?;
                let kind = visible(s, &user, channel).await?.kind;
                let reason = if prefs.mentions && message.mention_ids.contains(&user) {
                    Some(NotificationReason::Mention)
                } else if prefs.dms && kind == ChannelKind::Dm {
                    Some(NotificationReason::Dm)
                } else if prefs.subscribed_channel_ids.iter().any(|id| id == channel) {
                    Some(NotificationReason::SubscribedChannel)
                } else {
                    None
                };
                if let Some(reason) = reason {
                    if s.push.is_some() {
                        let badge = states(s, &user)
                            .await?
                            .iter()
                            .map(|v| v.notification_count)
                            .sum();
                        push::notification(s, &user, message, reason.clone(), badge);
                    }
                    let _ = s.events.send(Event::Notification {
                        user_id: user.clone(),
                        message: message.clone(),
                        reason,
                    });
                }
            }
        }
        let _ = s.events.send(Event::ReadStateUpdated {
            user_id: user,
            state,
        });
    }
    Ok(())
}
