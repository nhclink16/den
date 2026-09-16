use crate::{auth::Auth, hosts, *};
use axum::extract::Path;
use sqlx::Row;

#[utoipa::path(post,path="/hosts/{id}/requests",params(("id"=String,Path)),request_body=RequestAccess,responses((status=200,body=Object)))]
pub(crate) async fn request(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<RequestAccess>,
) -> Result<Json<Object>> {
    if (v.standing && v.duration_minutes.is_some())
        || v.duration_minutes.is_some_and(|m| m == 0 || m > 43200)
    {
        return Err(Error::bad(
            "Choose a duration of 1 to 43200 minutes or standing access",
        ));
    }
    let host = hosts::load(&s, &id).await?;
    let channel = chat::dm(
        State(s.clone()),
        a.clone(),
        ApiJson(CreateDm {
            member_ids: vec![host.owner_id.clone()],
        }),
    )
    .await?
    .0;
    let _g = s.writes.lock().await;
    let id = s.id();
    let request = AccessRequest {
        id: id.clone(),
        host_id: host.id.clone(),
        host_name: host.name.clone(),
        owner_id: host.owner_id.clone(),
        requester_id: a.user.id.clone(),
        capability: v.capability,
        duration_minutes: if v.standing {
            None
        } else {
            Some(v.duration_minutes.unwrap_or(60))
        },
        standing: v.standing,
        status: "pending".into(),
        expires_at: now() + 600,
        grant_id: None,
    };
    // Empty context on purpose: an access-request card is the person asking for a
    // machine, not output from whatever job they may also be running, so it must not
    // be swept into an unrelated task's conversation.
    let o = terminal::create_object(
        &s,
        &id,
        &channel.id,
        &a.user.id,
        "access_request",
        &host.name,
        "request",
        serde_json::to_value(request).unwrap(),
        &threads::Context {
            thread_id: None,
            task_id: None,
            reply_to: None,
            hint: Some(&host.name),
        },
    )
    .await?;
    hosts::audit(&s, &host, &a.user.id, "request", Some(&id)).await?;
    Ok(Json(o))
}
#[utoipa::path(post,path="/requests/{id}/decide",params(("id"=String,Path)),request_body=AccessDecision,responses((status=200,body=AccessRequest)))]
pub(crate) async fn decide(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<AccessDecision>,
) -> Result<Json<AccessRequest>> {
    let _g = s.writes.lock().await;
    let o = objects::load(&s, &id).await?;
    let mut r: AccessRequest =
        serde_json::from_value(o.state.get("request").cloned().ok_or_else(Error::missing)?)
            .map_err(|_| Error::missing())?;
    let host = hosts::load(&s, &r.host_id).await?;
    if a.user.id != host.owner_id {
        return Err(Error::missing());
    }
    if r.status != "pending" || r.expires_at <= now() {
        return Err(Error::conflict("Request already decided or expired"));
    }
    let mut tx = s.db.begin().await?;
    if v.allow {
        let grant = s.id();
        sqlx::query("INSERT INTO grants(id,host_id,grantee_id,capability,expires_at,created_by,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(&grant).bind(&r.host_id).bind(&r.requester_id).bind(r.capability.as_str()).bind(r.duration_minutes.map(|m|now()+i64::from(m)*60)).bind(&a.user.id).bind(now()).execute(&mut *tx).await?;
        audit_in(&s, &mut tx, &host, &a.user.id, "grant", &grant).await?;
        r.grant_id = Some(grant);
        r.status = "allowed".into();
    } else {
        r.status = "denied".into();
    }
    sqlx::query("UPDATE objects SET state=?,version=version+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(serde_json::json!({"request":r}).to_string()).bind(&id).execute(&mut *tx).await?;
    audit_in(
        &s,
        &mut tx,
        &host,
        &a.user.id,
        if v.allow { "allow" } else { "deny" },
        &id,
    )
    .await?;
    tx.commit().await?;
    if let Some(message) = o.summary.message_id {
        let _ = s.events.send(Event::MessageEdited(
            messages::get_message(&s, &message).await?,
        ));
    }
    let _ = s.events.send(Event::AccessDecided {
        user_id: r.requester_id.clone(),
        request: r.clone(),
    });
    Ok(Json(r))
}
#[utoipa::path(get,path="/grants",responses((status=200,body=Vec<Grant>)))]
pub(crate) async fn grants(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<Grant>>> {
    let rows=sqlx::query("SELECT g.* FROM grants g JOIN hosts h ON h.id=g.host_id WHERE (h.owner_id=? OR g.grantee_id=?) AND g.revoked_at IS NULL AND (g.expires_at IS NULL OR g.expires_at>?) ORDER BY g.id DESC").bind(&a.user.id).bind(&a.user.id).bind(now()).fetch_all(&s.db).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| {
                Ok(Grant {
                    id: r.try_get("id")?,
                    host_id: r.try_get("host_id")?,
                    grantee_id: r.try_get("grantee_id")?,
                    capability: if r.try_get::<&str, _>("capability")? == "terminal_control" {
                        Capability::TerminalControl
                    } else {
                        Capability::TerminalView
                    },
                    expires_at: r.try_get("expires_at")?,
                    created_by: r.try_get("created_by")?,
                    created_at: r.try_get("created_at")?,
                    revoked_at: r.try_get("revoked_at")?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    ))
}
#[utoipa::path(delete,path="/grants/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn revoke(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let _g = s.writes.lock().await;
    let r = sqlx::query("SELECT host_id,grantee_id FROM grants WHERE id=?")
        .bind(&id)
        .fetch_one(&s.db)
        .await?;
    let host = hosts::load(&s, r.try_get("host_id")?).await?;
    let grantee: String = r.try_get("grantee_id")?;
    if host.owner_id != a.user.id && grantee != a.user.id {
        return Err(Error::missing());
    }
    sqlx::query("UPDATE grants SET revoked_at=? WHERE id=?")
        .bind(now())
        .bind(&id)
        .execute(&s.db)
        .await?;
    hosts::audit(&s, &host, &a.user.id, "revoke", Some(&id)).await?;
    terminal::revoke_user(&s, &host.id, &grantee).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/access/log",responses((status=200,body=Vec<AccessLog>)))]
pub(crate) async fn log(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<AccessLog>>> {
    let rows = sqlx::query(
        "SELECT * FROM access_log WHERE owner_id=? OR actor_id=? ORDER BY id DESC LIMIT 200",
    )
    .bind(&a.user.id)
    .bind(&a.user.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| {
                Ok(AccessLog {
                    id: r.try_get("id")?,
                    host_id: r.try_get("host_id")?,
                    actor_id: r.try_get("actor_id")?,
                    action: r.try_get("action")?,
                    subject_id: r.try_get("subject_id")?,
                    created_at: r.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    ))
}

async fn audit_in(
    s: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    host: &Host,
    actor: &str,
    action: &str,
    subject: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO access_log(id,host_id,owner_id,actor_id,action,subject_id,created_at) VALUES(?,?,?,?,?,?,?)")
        .bind(s.id()).bind(&host.id).bind(&host.owner_id).bind(actor).bind(action).bind(subject).bind(now()).execute(&mut **tx).await?;
    Ok(())
}
