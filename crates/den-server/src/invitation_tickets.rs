use crate::*;

pub(crate) async fn issue(
    db: &SqlitePool,
    device: &str,
    generation: i64,
    invite: &CallInvitation,
) -> Result<Option<String>> {
    let secret = auth::secret();
    let changed=sqlx::query("INSERT INTO call_fetch_tickets(digest,device_id,generation,invitation_id,expires_at)
        SELECT ?,d.id,d.generation,i.id,i.expires_at FROM devices d JOIN call_invite_recipients r ON r.user_id=d.user_id JOIN call_invitations i ON i.id=r.invitation_id
        WHERE d.id=? AND d.generation=? AND d.purpose='voip' AND i.id=? AND i.state IN ('ringing','active') AND i.expires_at>? AND r.declined=0 AND r.answer_id IS NULL
        AND EXISTS(SELECT 1 FROM channel_members WHERE channel_id=i.channel_id AND user_id=d.user_id)
        AND (EXISTS(SELECT 1 FROM sessions WHERE id=d.session_id AND expires_at>?) OR EXISTS(SELECT 1 FROM tokens WHERE id=d.token_id))
        ON CONFLICT(device_id,invitation_id) DO UPDATE SET digest=excluded.digest,generation=excluded.generation,expires_at=excluded.expires_at")
        .bind(auth::hash(&secret)).bind(device).bind(generation).bind(&invite.id).bind(now()).bind(now()).execute(db).await?.rows_affected();
    Ok((changed == 1).then_some(secret))
}

fn gone() -> Error {
    Error(
        StatusCode::GONE,
        "ticket_expired",
        "Invitation ticket is invalid or expired".into(),
    )
}

async fn destination(
    s: &AppState,
    device: &str,
    generation: i64,
    invitation: &str,
) -> Result<auth::Auth> {
    let row:Option<(String,Option<String>,Option<String>)>=sqlx::query_as("SELECT d.user_id,d.session_id,d.token_id FROM devices d JOIN call_invite_recipients r ON r.user_id=d.user_id JOIN call_invitations i ON i.id=r.invitation_id JOIN channel_members m ON m.channel_id=i.channel_id AND m.user_id=d.user_id WHERE d.id=? AND d.generation=? AND d.purpose='voip' AND i.id=? AND i.expires_at>? AND (EXISTS(SELECT 1 FROM sessions WHERE id=d.session_id AND expires_at>?) OR EXISTS(SELECT 1 FROM tokens WHERE id=d.token_id))")
        .bind(device).bind(generation).bind(invitation).bind(now()).bind(now()).fetch_optional(&s.db).await?;
    let (user, session, token) = row.ok_or_else(gone)?;
    let is_session = session.is_some();
    Ok(auth::Auth {
        user: auth::user(s, &user).await?,
        credential: session.or(token).ok_or_else(gone)?,
        session: is_session,
    })
}

#[utoipa::path(post,path="/calls/invitations/redeem",request_body=RedeemCallInvitation,responses((status=200,body=CallInvitationState),(status=410,body=ApiError)),description="Redeem a one-use recipient/device-bound capability for invitation state only. No Den bearer is needed and no login or media credentials are returned. Ticket expires at the original ringing deadline. Invalid, replayed, or revoked tickets return 410.")]
pub(crate) async fn redeem(
    State(s): State<AppState>,
    ApiJson(v): ApiJson<RedeemCallInvitation>,
) -> Result<Response> {
    if v.ticket.is_empty() || v.ticket.len() > 128 {
        return Err(gone());
    }
    let guard = s.writes.lock().await;
    let row:Option<(String,i64,String,i64)>=sqlx::query_as("DELETE FROM call_fetch_tickets WHERE digest=? RETURNING device_id,generation,invitation_id,expires_at")
        .bind(auth::hash(&v.ticket)).fetch_optional(&s.db).await?;
    let (device, generation, invitation, expiry) = row.ok_or_else(gone)?;
    if expiry <= now() {
        return Err(gone());
    }
    let a = destination(&s, &device, generation, &invitation).await?;
    drop(guard);
    invitation_state::reconcile(&s, &a, &invitation)
        .await
        .map_err(|_| gone())?;
    let _guard = s.writes.lock().await;
    let a = destination(&s, &device, generation, &invitation).await?;
    let call = invitation_state::authorized(&s, &a, &invitation)
        .await
        .map_err(|_| gone())?;
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(call),
    )
        .into_response())
}
