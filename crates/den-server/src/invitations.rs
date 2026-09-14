use crate::{auth::Auth, chat::visible, *};
use axum::extract::Path;

pub(crate) async fn pending(
    db: &SqlitePool,
    user: &str,
    invite: &CallInvitation,
) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM call_invitations i JOIN call_invite_recipients r USING(channel_id)
         JOIN channel_members m ON m.channel_id=i.channel_id AND m.user_id=r.user_id
         WHERE i.channel_id=? AND i.from_user_id=? AND i.expires_at=? AND i.expires_at>?
         AND r.user_id=? AND r.declined=0)",
    ).bind(&invite.channel_id).bind(&invite.from_user_id).bind(invite.expires_at).bind(now())
        .bind(user).fetch_one(db).await
}

async fn expire(db: &SqlitePool, invite: &CallInvitation) -> std::result::Result<(), sqlx::Error> {
    // Match the generation so an old task can never remove a replacement call.
    sqlx::query("DELETE FROM call_invitations WHERE channel_id=? AND from_user_id=? AND expires_at=? AND expires_at<=?")
        .bind(&invite.channel_id).bind(&invite.from_user_id).bind(invite.expires_at).bind(now())
        .execute(db).await?;
    Ok(())
}

#[utoipa::path(post,path="/calls/{channel_id}/invite",params(("channel_id"=String,Path)),responses((status=200,body=CallInvitation),(status=409,body=ApiError)),description="Invite other members of a DM/group DM after joining its call. A repeated request by the active caller returns the same invitation without re-notifying. Another caller conflicts until the 45-second invitation expires. This is invitation signaling, not VoIP delivery or an accept/cancel protocol.")]
pub(crate) async fn invite(
    State(s): State<AppState>,
    a: Auth,
    Path(channel_id): Path<String>,
) -> Result<Json<CallInvitation>> {
    let _guard = s.writes.lock().await;
    let channel = visible(&s, &a.user.id, &channel_id).await?;
    if channel.kind != ChannelKind::Dm {
        return Err(Error::bad("Only DM calls can invite members"));
    }
    if !a.valid(&s).await {
        return Err(Error::unauthorized());
    }
    let calls = s.calls.lock().await;
    let participating = calls.get(&channel_id).is_some_and(|connections| {
        connections.keys().any(|identity| {
            identity
                .split_once(':')
                .map_or(identity.as_str(), |(user, _)| user)
                == a.user.id
        })
    });
    if !participating {
        return Err(Error::conflict("Join the call before inviting its members"));
    }
    drop(calls);
    let existing: Option<(String, i64)> = sqlx::query_as(
        "SELECT from_user_id,expires_at FROM call_invitations WHERE channel_id=? AND expires_at>?",
    )
    .bind(&channel_id)
    .bind(now())
    .fetch_optional(&s.db)
    .await?;
    if let Some((from_user_id, expires_at)) = existing {
        if from_user_id != a.user.id {
            return Err(Error::conflict(
                "This call already has an active invitation",
            ));
        }
        return Ok(Json(CallInvitation {
            channel_id,
            from_user_id,
            expires_at,
        }));
    }
    let invitation = CallInvitation {
        channel_id: channel_id.clone(),
        from_user_id: a.user.id.clone(),
        expires_at: now() + 45,
    };
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM call_invitations WHERE channel_id=?")
        .bind(&channel_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO call_invitations(channel_id,from_user_id,expires_at) VALUES(?,?,?)")
        .bind(&channel_id)
        .bind(&a.user.id)
        .bind(invitation.expires_at)
        .execute(&mut *tx)
        .await?;
    let recipients: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM channel_members WHERE channel_id=? AND user_id<>?")
            .bind(&channel_id)
            .bind(&a.user.id)
            .fetch_all(&mut *tx)
            .await?;
    for user in &recipients {
        sqlx::query("INSERT INTO call_invite_recipients(channel_id,user_id) VALUES(?,?)")
            .bind(&channel_id)
            .bind(user)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let _ = s.events.send(Event::CallInvite {
        channel_id: channel_id.clone(),
        from_user_id: a.user.id,
        expires_at: invitation.expires_at,
    });
    for user in &recipients {
        push::invitation(&s, user, &invitation);
    }
    let weak = Arc::downgrade(&s.0);
    let deadline = invitation.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(45)).await;
        if let Some(state) = weak.upgrade() {
            if expire(&state.db, &deadline).await.is_err() {
                tracing::warn!("Call invitation expiry cleanup failed");
            }
        }
    });
    Ok(Json(invitation))
}

#[utoipa::path(post,path="/calls/{channel_id}/invite/decline",params(("channel_id"=String,Path)),request_body=DeclineCallInvitation,responses((status=204),(status=404,body=ApiError)),description="Decline the identified invitation for your account across its devices. Repeated declines are idempotent while that invitation exists. An expired or replaced invitation returns 404, so a late decline cannot dismiss a new call. It does not end another participant's call.")]
pub(crate) async fn decline(
    State(s): State<AppState>,
    a: Auth,
    Path(channel_id): Path<String>,
    ApiJson(v): ApiJson<DeclineCallInvitation>,
) -> Result<StatusCode> {
    let _guard = s.writes.lock().await;
    visible(&s, &a.user.id, &channel_id).await?;
    let updated = sqlx::query("UPDATE call_invite_recipients SET declined=1 WHERE channel_id=? AND user_id=? AND EXISTS(SELECT 1 FROM call_invitations WHERE channel_id=? AND from_user_id=? AND expires_at=? AND expires_at>?)")
        .bind(&channel_id).bind(&a.user.id).bind(&channel_id).bind(&v.from_user_id).bind(v.expires_at).bind(now()).execute(&s.db).await?.rows_affected();
    if updated == 0 {
        return Err(Error::missing());
    }
    Ok(StatusCode::NO_CONTENT)
}
