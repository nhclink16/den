use crate::{
    auth::{self, Auth},
    *,
};
use axum::{
    extract::{ws::WebSocketUpgrade, Path},
    http::HeaderMap,
};
use sqlx::Row;
use tokio::sync::mpsc;

#[path = "hosts_connection.rs"]
mod connection;

pub(crate) struct Connection {
    id: String,
    tx: mpsc::Sender<HostFrame>,
    ready: bool,
}

#[derive(Default)]
pub(crate) struct Hosts {
    pub connections: Mutex<HashMap<String, Connection>>,
    pub direct_tokens: Mutex<HashMap<String, (String, String, i64)>>,
    pub viewers: Mutex<HashMap<(String, String), std::collections::HashSet<String>>>,
}
pub(crate) async fn load(s: &AppState, id: &str) -> Result<Host> {
    let r = sqlx::query("SELECT * FROM hosts WHERE id=?")
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    Ok(Host {
        id: r.try_get("id")?,
        owner_id: r.try_get("owner_id")?,
        name: r.try_get("name")?,
        online: s
            .hosts
            .connections
            .lock()
            .await
            .get(id)
            .is_some_and(|c| c.ready),
        last_seen: r.try_get("last_seen")?,
        direct_url: r.try_get("direct_url")?,
    })
}
pub(crate) async fn permitted(s: &AppState, user: &str, host: &str, control: bool) -> bool {
    let Ok(h) = load(s, host).await else {
        return false;
    };
    if h.owner_id == user {
        return true;
    }
    sqlx::query_scalar::<_,i64>("SELECT count(*) FROM grants WHERE host_id=? AND grantee_id=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?) AND (capability='terminal_control' OR ?=0)")
        .bind(host).bind(user).bind(now()).bind(control).fetch_one(&s.db).await.unwrap_or(0)>0
}
pub(crate) async fn audit(
    s: &AppState,
    host: &Host,
    actor: &str,
    action: &str,
    subject: Option<&str>,
) -> Result<()> {
    sqlx::query("INSERT INTO access_log(id,host_id,owner_id,actor_id,action,subject_id,created_at) VALUES(?,?,?,?,?,?,?)").bind(s.id()).bind(&host.id).bind(&host.owner_id).bind(actor).bind(action).bind(subject).bind(now()).execute(&s.db).await?;
    Ok(())
}
#[utoipa::path(get,path="/hosts",responses((status=200,body=Vec<Host>)))]
pub(crate) async fn list(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<Host>>> {
    let ids =
        sqlx::query_scalar::<_, String>("SELECT id FROM hosts WHERE owner_id=? ORDER BY name")
            .bind(a.user.id)
            .fetch_all(&s.db)
            .await?;
    let mut hosts = Vec::new();
    for id in ids {
        hosts.push(load(&s, &id).await?);
    }
    Ok(Json(hosts))
}
#[utoipa::path(post,path="/hosts/enroll",responses((status=200,body=HostEnrollment)))]
pub(crate) async fn enroll(State(s): State<AppState>, a: Auth) -> Result<Json<HostEnrollment>> {
    let code = auth::secret();
    let expires_at = now() + 600;
    let _g = s.writes.lock().await;
    sqlx::query("DELETE FROM host_enrollments WHERE owner_id=? OR expires_at<=?")
        .bind(&a.user.id)
        .bind(now())
        .execute(&s.db)
        .await?;
    sqlx::query("INSERT INTO host_enrollments VALUES(?,?,?)")
        .bind(auth::hash(&code))
        .bind(a.user.id)
        .bind(expires_at)
        .execute(&s.db)
        .await?;
    Ok(Json(HostEnrollment {
        code: format!("{}#{code}", s.origin),
        expires_at,
    }))
}
#[utoipa::path(post,path="/hosts/login",request_body=HostLogin,responses((status=200,body=HostCredential)))]
pub(crate) async fn login(
    State(s): State<AppState>,
    ApiJson(v): ApiJson<HostLogin>,
) -> Result<Json<HostCredential>> {
    name(&v.name)?;
    let _g = s.writes.lock().await;
    let mut tx = s.db.begin().await?;
    let owner: Option<String> = sqlx::query_scalar(
        "DELETE FROM host_enrollments WHERE code_hash=? AND expires_at>? RETURNING owner_id",
    )
    .bind(auth::hash(&v.code))
    .bind(now())
    .fetch_optional(&mut *tx)
    .await?;
    let owner = owner.ok_or_else(Error::unauthorized)?;
    let id = s.id();
    let token = auth::secret();
    sqlx::query("INSERT INTO hosts(id,owner_id,name,token_hash) VALUES(?,?,?,?)")
        .bind(&id)
        .bind(owner)
        .bind(&v.name)
        .bind(auth::hash(&token))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(HostCredential {
        server_url: s.origin.clone(),
        host_id: id,
        name: v.name,
        token,
    }))
}
#[utoipa::path(delete,path="/hosts/{id}",params(("id"=String,Path)),responses((status=204)))]
pub(crate) async fn remove(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let _g = s.writes.lock().await;
    let host = load(&s, &id).await?;
    if host.owner_id != a.user.id {
        return Err(Error::missing());
    }
    let ids = sqlx::query_scalar::<_, String>("SELECT id FROM terminal_sessions WHERE host_id=?")
        .bind(&id)
        .fetch_all(&s.db)
        .await?;
    for session in ids {
        terminal::finish(&s, &session).await?;
        let _ = send(
            &s,
            &id,
            HostFrame::Close {
                session_id: session,
            },
        )
        .await;
    }
    audit(&s, &host, &a.user.id, "unenroll", None).await?;
    sqlx::query("DELETE FROM hosts WHERE id=?")
        .bind(&id)
        .execute(&s.db)
        .await?;
    s.hosts.connections.lock().await.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn authenticate(s: &AppState, headers: &HeaderMap) -> Result<Host> {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(Error::unauthorized)?;
    let id: Option<String> = sqlx::query_scalar("SELECT id FROM hosts WHERE token_hash=?")
        .bind(auth::hash(token))
        .fetch_optional(&s.db)
        .await?;
    load(s, &id.ok_or_else(Error::unauthorized)?).await
}
#[utoipa::path(get,path="/hosts/ws",responses((status=101)))]
pub(crate) async fn connect(
    State(s): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    let h = authenticate(&s, &headers).await?;
    Ok(ws
        .max_message_size(2 * 1024 * 1024)
        .on_upgrade(move |ws| connection::run(s, h, ws)))
}
pub(crate) async fn send(s: &AppState, id: &str, frame: HostFrame) -> Result<()> {
    let connections = s.hosts.connections.lock().await;
    let connection = connections
        .get(id)
        .filter(|c| c.ready)
        .ok_or_else(|| Error::conflict("Machine is offline"))?;
    connection
        .tx
        .try_send(frame)
        .map_err(|_| Error::conflict("Machine is busy; retry"))
}
fn valid_direct_url(url: &str) -> bool {
    let Ok(uri) = url.parse::<axum::http::Uri>() else {
        return false;
    };
    let host = uri.host().unwrap_or("");
    if uri.query().is_some() || uri.port_u16().is_none() {
        return false;
    }
    if uri.scheme_str() == Some("wss") {
        return host.ends_with(".ts.net");
    }
    let Ok(ip) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    let o = ip.octets();
    uri.scheme_str() == Some("ws") && o[0] == 100 && (64..=127).contains(&o[1])
}
#[utoipa::path(post,path="/hosts/direct/check",request_body=DirectCheck,responses((status=200,body=DirectPermission)))]
pub(crate) async fn direct_check(
    State(s): State<AppState>,
    headers: HeaderMap,
    ApiJson(v): ApiJson<DirectCheck>,
) -> Result<Json<DirectPermission>> {
    let host = authenticate(&s, &headers).await?;
    let grant = s
        .hosts
        .direct_tokens
        .lock()
        .await
        .get(&auth::hash(&v.token))
        .cloned()
        .ok_or_else(Error::unauthorized)?;
    if grant.1 != v.session_id || grant.2 <= now() {
        return Err(Error::unauthorized());
    }
    let t = terminal::load(&s, &v.session_id).await?;
    if t.host_id != host.id
        || t.ended_at.is_some()
        || !terminal::can_view(&s, &grant.0, &v.session_id).await
        || (v.input
            && (t.active_controller_id.as_deref() != Some(&grant.0)
                || !permitted(&s, &grant.0, &host.id, true).await))
    {
        return Err(Error::forbidden());
    }
    let name = sqlx::query_scalar("SELECT username FROM users WHERE id=?")
        .bind(&grant.0)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(DirectPermission {
        owner: host.owner_id == grant.0,
        user_id: grant.0,
        name,
    }))
}
