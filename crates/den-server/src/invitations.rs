use crate::{auth::Auth, chat::visible, invitation_state as lifecycle, *};
use axum::extract::Path;

pub(crate) async fn pending(
    db: &SqlitePool,
    user: &str,
    invite: &CallInvitation,
) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM call_invitations i JOIN call_invite_recipients r ON r.invitation_id=i.id JOIN channel_members m ON m.channel_id=i.channel_id AND m.user_id=r.user_id WHERE i.id=? AND i.channel_id=? AND i.from_user_id=? AND i.expires_at=? AND i.expires_at>? AND i.state IN ('ringing','active') AND r.user_id=? AND r.declined=0 AND r.answer_id IS NULL)")
        .bind(&invite.id).bind(&invite.channel_id).bind(&invite.from_user_id).bind(invite.expires_at).bind(now()).bind(user).fetch_one(db).await
}

#[utoipa::path(post,path="/calls/{channel_id}/invite",params(("channel_id"=String,Path)),responses((status=200,body=CallInvitation),(status=409,body=ApiError)),description="Create a 45-second DM invitation after joining its call. The active caller's retries return the same generation without re-ringing. Accepted media survives the ringing deadline.")]
pub(crate) async fn invite(
    State(s): State<AppState>,
    a: Auth,
    Path(channel_id): Path<String>,
) -> Result<Json<CallInvitation>> {
    visible(&s, &a.user.id, &channel_id).await?;
    calls::refresh(&s, std::slice::from_ref(&channel_id)).await?;
    let _guard = s.writes.lock().await;
    let channel = visible(&s, &a.user.id, &channel_id).await?;
    if channel.kind != ChannelKind::Dm {
        return Err(Error::bad("Only DM calls can invite members"));
    }
    if !a.valid(&s).await {
        return Err(Error::unauthorized());
    }
    lifecycle::expire(&s).await?;
    let calls = s.calls.lock().await;
    if !calls.get(&channel_id).is_some_and(|connections| {
        connections
            .keys()
            .any(|id| id.split(':').next() == Some(a.user.id.as_str()))
    }) {
        return Err(Error::conflict("Join the call before inviting its members"));
    }
    drop(calls);
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM call_invitations WHERE channel_id=? AND state IN ('ringing','active')",
    )
    .bind(&channel_id)
    .fetch_optional(&s.db)
    .await?;
    if let Some(id) = existing {
        let call = lifecycle::view(&s.db, &id).await?;
        if call.invitation.from_user_id != a.user.id {
            return Err(Error::conflict(
                "This call already has an active invitation",
            ));
        }
        return Ok(Json(call.invitation));
    }
    let invitation = CallInvitation {
        id: s.id(),
        channel_id: channel_id.clone(),
        from_user_id: a.user.id.clone(),
        expires_at: now() + 45,
    };
    let mut tx = s.db.begin().await?;
    sqlx::query(
        "INSERT INTO call_invitations(id,channel_id,from_user_id,expires_at) VALUES(?,?,?,?)",
    )
    .bind(&invitation.id)
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
        sqlx::query("INSERT INTO call_invite_recipients(invitation_id,user_id) VALUES(?,?)")
            .bind(&invitation.id)
            .bind(user)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let _ = s.events.send(Event::CallInvite {
        invitation_id: invitation.id.clone(),
        channel_id,
        from_user_id: a.user.id,
        expires_at: invitation.expires_at,
    });
    lifecycle::emit(&s, &lifecycle::view(&s.db, &invitation.id).await?);
    for user in recipients {
        push::invitation(&s, &user, &invitation);
    }
    let weak = Arc::downgrade(&s.0);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(45)).await;
        if let Some(inner) = weak.upgrade() {
            let state = AppState(inner);
            let _guard = state.writes.lock().await;
            if lifecycle::expire(&state).await.is_err() {
                tracing::warn!("Call invitation expiry failed");
            }
        }
    });
    Ok(Json(invitation))
}

#[utoipa::path(get,path="/calls/invitations",responses((status=200,body=Vec<CallInvitationState>)))]
pub(crate) async fn list(
    State(s): State<AppState>,
    a: Auth,
) -> Result<Json<Vec<CallInvitationState>>> {
    let Json(channels) = chat::channels(State(s.clone()), a.clone()).await?;
    calls::refresh(&s, &channels.into_iter().map(|c| c.id).collect::<Vec<_>>()).await?;
    let _guard = s.writes.lock().await;
    if !a.valid(&s).await {
        return Err(Error::unauthorized());
    }
    lifecycle::expire(&s).await?;
    let ids:Vec<String>=sqlx::query_scalar("SELECT i.id FROM call_invitations i JOIN channel_members m ON m.channel_id=i.channel_id WHERE m.user_id=? AND (i.from_user_id=? OR EXISTS(SELECT 1 FROM call_invite_recipients r WHERE r.invitation_id=i.id AND r.user_id=?)) ORDER BY i.id")
        .bind(&a.user.id).bind(&a.user.id).bind(&a.user.id).fetch_all(&s.db).await?;
    let mut result = Vec::new();
    for id in ids {
        result.push(lifecycle::view(&s.db, &id).await?);
    }
    Ok(Json(result))
}

#[utoipa::path(get,path="/calls/invitations/{invitation_id}",params(("invitation_id"=String,Path)),responses((status=200,body=CallInvitationState),(status=404,body=ApiError)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<CallInvitationState>> {
    lifecycle::reconcile(&s, &a, &id).await?;
    let _guard = s.writes.lock().await;
    lifecycle::expire(&s).await?;
    Ok(Json(lifecycle::authorized(&s, &a, &id).await?))
}

#[utoipa::path(post,path="/calls/{channel_id}/invite/accept",params(("channel_id"=String,Path)),request_body=AcceptCallInvitation,responses((status=200,body=CallInvitationState),(status=409,body=ApiError)),description="First answer wins per recipient account. Matching answer UUID and credential retries return state; another credential/answer returns answered_elsewhere. Fetch media separately through the existing token endpoint. An answer that never joins is cleared after a 20-second grace only when LiveKit confirms no connection for that recipient account; an outage preserves it.")]
pub(crate) async fn accept(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<AcceptCallInvitation>,
) -> Result<Json<CallInvitationState>> {
    let answer = devices::uuid(&v.answer_id)?;
    lifecycle::reconcile(&s, &a, &v.invitation_id).await?;
    let _guard = s.writes.lock().await;
    lifecycle::expire(&s).await?;
    let call = lifecycle::authorized(&s, &a, &v.invitation_id).await?;
    if call.invitation.channel_id != channel {
        return Err(Error::missing());
    }
    type AnswerRow = (i64, Option<String>, Option<String>, Option<bool>);
    let recipient:Option<AnswerRow>=sqlx::query_as("SELECT declined,answer_id,answer_credential,answer_session FROM call_invite_recipients WHERE invitation_id=? AND user_id=?")
        .bind(&v.invitation_id).bind(&a.user.id).fetch_optional(&s.db).await?;
    let (declined, prior, credential, session) = recipient.ok_or_else(Error::missing)?;
    if lifecycle::terminal(&call) {
        return Err(Error::conflict("Invitation is no longer active"));
    }
    if let Some(prior) = prior {
        if prior == answer
            && credential.as_deref() == Some(&a.credential)
            && session == Some(a.session)
        {
            return Ok(Json(call));
        }
        return Err(Error(
            StatusCode::CONFLICT,
            "answered_elsewhere",
            "This invitation was already answered".into(),
        ));
    }
    if declined != 0 || call.invitation.expires_at <= now() {
        return Err(Error::conflict("Invitation is no longer ringing"));
    }
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE call_invite_recipients SET answer_id=?,answer_credential=?,answer_session=?,accepted_at=? WHERE invitation_id=? AND user_id=?")
        .bind(&answer).bind(&a.credential).bind(a.session).bind(now()).bind(&v.invitation_id).bind(&a.user.id).execute(&mut *tx).await?;
    sqlx::query("UPDATE call_invitations SET state='active' WHERE id=?")
        .bind(&v.invitation_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let call = lifecycle::view(&s.db, &v.invitation_id).await?;
    let _ = s.events.send(Event::CallInviteAccepted {
        call: call.clone(),
        user_id: a.user.id,
        answer_id: answer,
    });
    lifecycle::emit(&s, &call);
    Ok(Json(call))
}

#[utoipa::path(post,path="/calls/{channel_id}/invite/decline",params(("channel_id"=String,Path)),request_body=DeclineCallInvitation,responses((status=204),(status=404,body=ApiError),(status=409,body=ApiError)),description="Decline for your recipient account. Legacy caller/expiry matching remains supported; include invitation_id to identify the exact generation. Accepted answers cannot be declined.")]
pub(crate) async fn decline(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<DeclineCallInvitation>,
) -> Result<StatusCode> {
    let _guard = s.writes.lock().await;
    visible(&s, &a.user.id, &channel).await?;
    lifecycle::expire(&s).await?;
    let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM call_invitations WHERE channel_id=? AND from_user_id=? AND expires_at=? AND (? IS NULL OR id=?)")
        .bind(&channel).bind(&v.from_user_id).bind(v.expires_at).bind(&v.invitation_id).bind(&v.invitation_id).fetch_all(&s.db).await?;
    if ids.len() != 1 {
        return Err(Error::missing());
    }
    let call = lifecycle::authorized(&s, &a, &ids[0]).await?;
    if lifecycle::terminal(&call) || call.invitation.expires_at <= now() {
        return Err(Error::missing());
    }
    let prior: Option<(i64, Option<String>)> = sqlx::query_as(
        "SELECT declined,answer_id FROM call_invite_recipients WHERE invitation_id=? AND user_id=?",
    )
    .bind(&ids[0])
    .bind(&a.user.id)
    .fetch_optional(&s.db)
    .await?;
    let (declined, answered) = prior.ok_or_else(Error::missing)?;
    if answered.is_some() {
        return Err(Error::conflict("An answered invitation cannot be declined"));
    }
    if declined == 0 {
        sqlx::query(
            "UPDATE call_invite_recipients SET declined=1 WHERE invitation_id=? AND user_id=?",
        )
        .bind(&ids[0])
        .bind(&a.user.id)
        .execute(&s.db)
        .await?;
        lifecycle::emit(&s, &lifecycle::view(&s.db, &ids[0]).await?);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post,path="/calls/{channel_id}/invite/cancel",params(("channel_id"=String,Path)),request_body=IdentifyCallInvitation,responses((status=204),(status=409,body=ApiError)),description="Caller-only cancellation before any acceptance. A cancelled generation retries with 204; active or other terminal generations return 409. Never disconnects media.")]
pub(crate) async fn cancel(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<IdentifyCallInvitation>,
) -> Result<StatusCode> {
    lifecycle::reconcile(&s, &a, &v.invitation_id).await?;
    let _guard = s.writes.lock().await;
    lifecycle::expire(&s).await?;
    let call = lifecycle::authorized(&s, &a, &v.invitation_id).await?;
    if call.invitation.channel_id != channel {
        return Err(Error::missing());
    }
    if call.invitation.from_user_id != a.user.id {
        return Err(Error::forbidden());
    }
    if call.state == InvitationStatus::Cancelled {
        return Ok(StatusCode::NO_CONTENT);
    }
    if call.state != InvitationStatus::Ringing {
        return Err(Error::conflict(
            "Only an unanswered invitation can be cancelled",
        ));
    }
    lifecycle::finish(&s, &v.invitation_id, "cancelled").await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post,path="/calls/{channel_id}/invite/end",params(("channel_id"=String,Path)),request_body=IdentifyCallInvitation,responses((status=204)),description="Caller-only signaling cleanup. Already terminal generations are idempotent. Does not disconnect any LiveKit participants; normally leave only your own media connection.")]
pub(crate) async fn end(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<IdentifyCallInvitation>,
) -> Result<StatusCode> {
    lifecycle::reconcile(&s, &a, &v.invitation_id).await?;
    let _guard = s.writes.lock().await;
    lifecycle::expire(&s).await?;
    let call = lifecycle::authorized(&s, &a, &v.invitation_id).await?;
    if call.invitation.channel_id != channel {
        return Err(Error::missing());
    }
    if call.invitation.from_user_id != a.user.id {
        return Err(Error::forbidden());
    }
    if !lifecycle::terminal(&call) {
        lifecycle::finish(&s, &v.invitation_id, "ended").await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
