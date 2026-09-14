use crate::{auth::Auth, chat::visible, *};

impl AppState {
    /// Restart-safe expiry events even if no client asks for reconciliation.
    pub fn run_invitation_expiry(&self) {
        let weak = Arc::downgrade(&self.0);
        let media_weak = weak.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(1));
            loop {
                timer.tick().await;
                let Some(inner) = weak.upgrade() else {
                    break;
                };
                let state = AppState(inner);
                let _guard = state.writes.lock().await;
                if expire(&state).await.is_err() {
                    tracing::warn!("Call invitation expiry failed");
                }
            }
        });
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let Some(inner) = media_weak.upgrade() else {
                    break;
                };
                let state = AppState(inner);
                let channels = sqlx::query_scalar::<_, String>(
                    "SELECT channel_id FROM call_invitations WHERE state IN ('ringing','active')",
                )
                .fetch_all(&state.db)
                .await;
                match channels {
                    Ok(channels) if !channels.is_empty() => {
                        if calls::refresh(&state, &channels).await.is_err() {
                            tracing::warn!("Call media reconciliation failed");
                        }
                    }
                    Err(_) => tracing::warn!("Call media reconciliation failed"),
                    _ => {}
                }
            }
        });
    }
}

pub(crate) async fn reconcile(s: &AppState, a: &Auth, id: &str) -> Result<()> {
    let call = authorized(s, a, id).await?;
    if !terminal(&call) {
        calls::refresh(s, &[call.invitation.channel_id]).await?;
    }
    Ok(())
}

pub(crate) async fn view(db: &SqlitePool, id: &str) -> Result<CallInvitationState> {
    let (id,channel_id,from_user_id,expires_at,state): (String,String,String,i64,String) =
        sqlx::query_as("SELECT id,channel_id,from_user_id,expires_at,state FROM call_invitations WHERE id=? AND (finished_at IS NULL OR finished_at>?)")
        .bind(id).bind(now()-600).fetch_one(db).await?;
    let accepted: Vec<(String,String)> = sqlx::query_as("SELECT user_id,answer_id FROM call_invite_recipients WHERE invitation_id=? AND answer_id IS NOT NULL ORDER BY user_id")
        .bind(&id).fetch_all(db).await?;
    let declined_user_ids = sqlx::query_scalar("SELECT user_id FROM call_invite_recipients WHERE invitation_id=? AND declined=1 ORDER BY user_id")
        .bind(&id).fetch_all(db).await?;
    let state = match state.as_str() {
        "ringing" => InvitationStatus::Ringing,
        "active" => InvitationStatus::Active,
        "cancelled" => InvitationStatus::Cancelled,
        "expired" => InvitationStatus::Expired,
        "ended" => InvitationStatus::Ended,
        _ => unreachable!("database status constraint"),
    };
    Ok(CallInvitationState {
        invitation: CallInvitation {
            id,
            channel_id,
            from_user_id,
            expires_at,
        },
        state,
        accepted: accepted
            .into_iter()
            .map(|(user_id, answer_id)| CallAcceptance { user_id, answer_id })
            .collect(),
        declined_user_ids,
    })
}

pub(crate) async fn authorized(s: &AppState, a: &Auth, id: &str) -> Result<CallInvitationState> {
    if !a.valid(s).await {
        return Err(Error::unauthorized());
    }
    let call = view(&s.db, id).await?;
    visible(s, &a.user.id, &call.invitation.channel_id).await?;
    let recipient: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM call_invite_recipients WHERE invitation_id=? AND user_id=?)",
    )
    .bind(id)
    .bind(&a.user.id)
    .fetch_one(&s.db)
    .await?;
    if call.invitation.from_user_id != a.user.id && !recipient {
        return Err(Error::missing());
    }
    Ok(call)
}

pub(crate) fn emit(s: &AppState, call: &CallInvitationState) {
    let _ = s
        .events
        .send(Event::CallInvitationState { call: call.clone() });
}

/// Caller holds writes so expiry, answers, and cancellation have a single order.
pub(crate) async fn expire(s: &AppState) -> Result<()> {
    let ids:Vec<String>=sqlx::query_scalar("UPDATE call_invitations SET ringing_expired=1,state=CASE WHEN state='ringing' THEN 'expired' ELSE state END,finished_at=CASE WHEN state='ringing' THEN ? ELSE finished_at END WHERE expires_at<=? AND ringing_expired=0 AND state IN ('ringing','active') RETURNING id")
        .bind(now()).bind(now()).fetch_all(&s.db).await?;
    for id in ids {
        let call = view(&s.db, &id).await?;
        let _ = s
            .events
            .send(Event::CallInviteExpired { call: call.clone() });
        emit(s, &call);
    }
    sqlx::query("DELETE FROM call_invitations WHERE finished_at<=?")
        .bind(now() - 600)
        .execute(&s.db)
        .await?;
    sqlx::query("DELETE FROM call_fetch_tickets WHERE expires_at<=?")
        .bind(now())
        .execute(&s.db)
        .await?;
    Ok(())
}

pub(crate) async fn finish(s: &AppState, id: &str, state: &str) -> Result<CallInvitationState> {
    sqlx::query("UPDATE call_invitations SET state=?,finished_at=? WHERE id=?")
        .bind(state)
        .bind(now())
        .bind(id)
        .execute(&s.db)
        .await?;
    let call = view(&s.db, id).await?;
    let event = if state == "cancelled" {
        Event::CallInviteCancelled { call: call.clone() }
    } else {
        Event::CallEnded { call: call.clone() }
    };
    let _ = s.events.send(event);
    emit(s, &call);
    Ok(call)
}

pub(crate) fn terminal(call: &CallInvitationState) -> bool {
    !matches!(
        call.state,
        InvitationStatus::Ringing | InvitationStatus::Active
    )
}

/// Reconcile only from an authoritative media snapshot/webhook; caller holds writes.
pub(crate) async fn media(
    s: &AppState,
    channel: &str,
    users: &[String],
    authoritative: bool,
) -> Result<()> {
    let current:Option<(String,String,String)>=sqlx::query_as("SELECT id,from_user_id,state FROM call_invitations WHERE channel_id=? AND state IN ('ringing','active')")
        .bind(channel).fetch_optional(&s.db).await?;
    if let Some((id, caller, state)) = current {
        // A positive join webhook proves presence. Absence needs a complete SDK
        // snapshot or a matching room_finished, never an incomplete restart cache.
        for user in users {
            sqlx::query("UPDATE call_invite_recipients SET joined=1 WHERE invitation_id=? AND user_id=? AND answer_id IS NOT NULL")
                .bind(&id).bind(user).execute(&s.db).await?;
        }
        if !authoritative {
            return Ok(());
        }
        if !users.contains(&caller) {
            finish(
                s,
                &id,
                if state == "ringing" {
                    "cancelled"
                } else {
                    "ended"
                },
            )
            .await?;
            return Ok(());
        }
        let abandoned:Vec<String>=sqlx::query_scalar("SELECT user_id FROM call_invite_recipients WHERE invitation_id=? AND answer_id IS NOT NULL AND joined=0 AND accepted_at<=?")
            .bind(&id).bind(now()-20).fetch_all(&s.db).await?;
        let mut changed = false;
        for user in abandoned.into_iter().filter(|user| !users.contains(user)) {
            sqlx::query("UPDATE call_invite_recipients SET answer_id=NULL,answer_credential=NULL,answer_session=NULL,accepted_at=NULL,declined=1 WHERE invitation_id=? AND user_id=?")
                .bind(&id).bind(user).execute(&s.db).await?;
            changed = true;
        }
        if changed {
            let call = view(&s.db, &id).await?;
            if call.accepted.is_empty() {
                let expired = call.invitation.expires_at <= now();
                sqlx::query("UPDATE call_invitations SET state=?,finished_at=?,ringing_expired=? WHERE id=?")
                    .bind(if expired {"expired"} else {"ringing"}).bind(expired.then_some(now())).bind(expired).bind(&id).execute(&s.db).await?;
                if expired {
                    let _ = s.events.send(Event::CallInviteExpired {
                        call: view(&s.db, &id).await?,
                    });
                }
            }
            emit(s, &view(&s.db, &id).await?);
        }
    }
    Ok(())
}
