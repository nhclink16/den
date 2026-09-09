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
    sqlx::query!("INSERT INTO notification_preferences(user_id,mentions,dms) VALUES(?,?,?) ON CONFLICT(user_id) DO UPDATE SET mentions=excluded.mentions,dms=excluded.dms",a.user.id,v.mentions,v.dms).execute(&mut *tx).await?;
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
    tx.commit().await?;
    let _ = s.events.send(Event::NotificationPreferencesUpdated {
        user_id: a.user.id.clone(),
        preferences: v.clone(),
    });
    for state in states(&s, &a.user.id).await? {
        let _ = s.events.send(Event::ReadStateUpdated {
            user_id: a.user.id.clone(),
            state,
        });
    }
    Ok(Json(v))
}
pub(crate) async fn state(s: &AppState, user: &str, channel: &str) -> Result<ChannelReadState> {
    // Each count uses the same SQLite statement snapshot. Own messages never count.
    let row=sqlx::query!(r#"SELECT r.last_read_id as "last_read_id?",
        (SELECT count(*) FROM messages m WHERE m.channel_id=c.id AND m.author_id<>? AND (r.last_read_id IS NULL OR m.id>r.last_read_id)) as "unread_count!: i64",
        (SELECT count(*) FROM messages m JOIN message_mentions mm ON mm.message_id=m.id AND mm.user_id=? WHERE m.channel_id=c.id AND m.author_id<>? AND (r.last_read_id IS NULL OR m.id>r.last_read_id)) as "mention_count!: i64",
        (SELECT count(*) FROM messages m WHERE m.channel_id=c.id AND m.author_id<>? AND (r.last_read_id IS NULL OR m.id>r.last_read_id)
            AND ((coalesce(p.mentions,1)=1 AND EXISTS(SELECT 1 FROM message_mentions mm WHERE mm.message_id=m.id AND mm.user_id=?))
                OR (coalesce(p.dms,1)=1 AND c.kind='dm') OR EXISTS(SELECT 1 FROM channel_subscriptions cs WHERE cs.channel_id=c.id AND cs.user_id=?))) as "notification_count!: i64"
        FROM channels c LEFT JOIN read_state r ON r.channel_id=c.id AND r.user_id=? LEFT JOIN notification_preferences p ON p.user_id=? WHERE c.id=?"#,
        user,user,user,user,user,user,user,user,channel).fetch_one(&s.db).await?;
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
    if sqlx::query_scalar!(
        "SELECT count(*) FROM messages WHERE id=? AND channel_id=?",
        v.message_id,
        channel
    )
    .fetch_one(&s.db)
    .await?
        != 1
    {
        return Err(Error::bad(
            "Read marker must identify a message in this channel",
        ));
    }
    sqlx::query!("INSERT INTO read_state(user_id,channel_id,last_read_id) VALUES(?,?,?) ON CONFLICT(user_id,channel_id) DO UPDATE SET last_read_id=max(read_state.last_read_id,excluded.last_read_id)",a.user.id,channel,v.message_id).execute(&s.db).await?;
    let state = state(&s, &a.user.id, &channel).await?;
    let _ = s.events.send(Event::ReadStateUpdated {
        user_id: a.user.id,
        state: state.clone(),
    });
    Ok(Json(state))
}
// Called after committed message mutations while the message write lock is held.
pub(crate) async fn changed(s: &AppState, channel: &str, message: Option<&Message>) -> Result<()> {
    let users=sqlx::query_scalar!("SELECT u.id FROM users u JOIN channels c ON c.id=? WHERE c.kind='text' OR EXISTS(SELECT 1 FROM channel_members cm WHERE cm.channel_id=c.id AND cm.user_id=u.id)",channel).fetch_all(&s.db).await?;
    for user in users {
        let state = state(s, &user, channel).await?;
        if let Some(message) = message {
            if message.author_id != user
                && state
                    .last_read_id
                    .as_ref()
                    .is_none_or(|id| id < &message.id)
            {
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
