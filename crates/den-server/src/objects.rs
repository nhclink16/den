use crate::{auth::Auth, chat::visible, *};
use axum::extract::Path;
use den_core::Object as LiveObject;
use sqlx::Row;
use std::collections::BTreeMap;

fn decode(r: sqlx::sqlite::SqliteRow) -> Result<Object> {
    let thumbnail_upload_id: Option<String> = r.try_get("thumbnail_upload_id")?;
    Ok(Object {
        summary: ObjectSummary {
            id: r.try_get("id")?,
            channel_id: r.try_get("channel_id")?,
            message_id: r.try_get("message_id")?,
            kind: r.try_get("kind")?,
            name: r.try_get("name")?,
            version: r.try_get("version")?,
            thumbnail_url: thumbnail_upload_id
                .as_ref()
                .map(|id| format!("/uploads/{id}/file")),
            thumbnail_upload_id,
            created_by: r.try_get("created_by")?,
            created_at: r.try_get("created_at")?,
            updated_at: r.try_get("updated_at")?,
        },
        state: serde_json::from_str(r.try_get("state")?)
            .map_err(|_| Error::bad("Invalid stored document"))?,
    })
}
pub(crate) async fn load(s: &AppState, id: &str) -> Result<Object> {
    decode(
        sqlx::query("SELECT * FROM objects WHERE id=?")
            .bind(id)
            .fetch_one(&s.db)
            .await?,
    )
}
pub(crate) async fn for_message(s: &AppState, id: &str) -> Result<Vec<ObjectSummary>> {
    sqlx::query("SELECT id,channel_id,message_id,kind,name,version,thumbnail_upload_id,created_by,created_at,updated_at,'{}' AS state FROM objects WHERE message_id=?").bind(id).fetch_all(&s.db).await?
        .into_iter().map(|r| decode(r).map(|o| o.summary)).collect()
}
fn record_id(v: &serde_json::Value) -> Result<&str> {
    v.as_object()
        .and_then(|v| v.get("id"))
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty() && id.len() <= 256)
        .ok_or_else(|| Error::bad("Each record needs an id string of 1 to 256 bytes"))
}
fn encode(state: &BTreeMap<String, serde_json::Value>) -> Result<String> {
    for (id, v) in state {
        if record_id(v)? != id {
            return Err(Error::bad("Record key must match its id"));
        }
    }
    let json = serde_json::to_string(state).map_err(|_| Error::bad("Invalid document"))?;
    if json.len() > 8 * 1024 * 1024 {
        return Err(Error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "object_too_large",
            "Canvas state exceeds 8 MiB".into(),
        ));
    }
    Ok(json)
}
#[utoipa::path(get,path="/settings",responses((status=200,body=Settings)))]
pub(crate) async fn settings(State(s): State<AppState>, _a: Auth) -> Result<Json<Settings>> {
    Ok(Json(read_settings(&s).await?))
}
async fn read_settings(s: &AppState) -> Result<Settings> {
    Ok(Settings {
        canvas_enabled: sqlx::query_scalar("SELECT canvas_enabled FROM settings WHERE id=1")
            .fetch_one(&s.db)
            .await?,
    })
}
#[utoipa::path(put,path="/settings",request_body=Settings,responses((status=200,body=Settings)))]
pub(crate) async fn save_settings(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(v): ApiJson<Settings>,
) -> Result<Json<Settings>> {
    if a.user.role != Role::Admin {
        return Err(Error::forbidden());
    }
    let _guard = s.writes.lock().await;
    sqlx::query("UPDATE settings SET canvas_enabled=? WHERE id=1")
        .bind(v.canvas_enabled)
        .execute(&s.db)
        .await?;
    let _ = s.events.send(Event::SettingsUpdated {
        settings: v.clone(),
    });
    Ok(Json(v))
}
#[utoipa::path(post,path="/channels/{id}/objects",params(("id"=String,Path)),request_body=CreateObject,responses((status=200,body=LiveObject)))]
pub(crate) async fn create(
    State(s): State<AppState>,
    a: Auth,
    Path(channel): Path<String>,
    ApiJson(v): ApiJson<CreateObject>,
) -> Result<Json<Object>> {
    if visible(&s, &a.user.id, &channel).await?.kind == ChannelKind::Voice {
        return Err(Error::bad("Voice rooms do not accept objects"));
    }
    if v.kind != "canvas" {
        return Err(Error::bad(
            "Use the dedicated endpoint for this object kind",
        ));
    }
    name(&v.kind)?;
    let object_name = if v.name.trim().is_empty() {
        "Untitled canvas"
    } else {
        v.name.trim()
    };
    name(object_name)?;
    let state = encode(&v.state)?;
    let _guard = s.writes.lock().await;
    if !read_settings(&s).await?.canvas_enabled {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "canvas_disabled",
            "Canvases are turned off on this server.".into(),
        ));
    }
    let id = s.id();
    let message = s.id();
    let mut tx = s.db.begin().await?;
    sqlx::query("INSERT INTO messages(id,channel_id,author_id,content) VALUES(?,?,?,'')")
        .bind(&message)
        .bind(&channel)
        .bind(&a.user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO objects(id,channel_id,message_id,kind,name,state,created_by) VALUES(?,?,?,?,?,?,?)")
        .bind(&id).bind(&channel).bind(&message).bind(&v.kind).bind(object_name).bind(state).bind(&a.user.id).execute(&mut *tx).await?;
    tx.commit().await?;
    let msg = messages::get_message(&s, &message).await?;
    let _ = s.events.send(Event::MessageCreated(msg.clone()));
    if let Err(e) = inbox::changed(&s, &channel, Some(&msg)).await {
        tracing::error!(code = e.1, "object inbox update failed");
    }
    Ok(Json(load(&s, &id).await?))
}
#[utoipa::path(get,path="/objects/{id}",params(("id"=String,Path)),responses((status=200,body=LiveObject)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<Object>> {
    let o = load(&s, &id).await?;
    visible(&s, &a.user.id, &o.summary.channel_id).await?;
    Ok(Json(o))
}
#[utoipa::path(get,path="/objects/{id}/summary",params(("id"=String,Path)),responses((status=200,body=ObjectSummary)))]
pub(crate) async fn summary(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<ObjectSummary>> {
    Ok(Json(get(State(s), a, Path(id)).await?.0.summary))
}
#[utoipa::path(post,path="/objects/{id}/patch",params(("id"=String,Path)),request_body=ObjectPatch,responses((status=200,body=ObjectVersion)))]
pub(crate) async fn patch(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<ObjectPatch>,
) -> Result<Json<ObjectVersion>> {
    let _guard = s.writes.lock().await;
    let mut o = load(&s, &id).await?;
    if o.summary.kind != "canvas" {
        return Err(Error::forbidden());
    }
    visible(&s, &a.user.id, &o.summary.channel_id).await?;
    for record in &v.put {
        o.state.insert(record_id(record)?.into(), record.clone());
    }
    for id in &v.remove {
        o.state.remove(id);
    }
    let state = encode(&o.state)?;
    let version = o.summary.version + 1;
    sqlx::query("UPDATE objects SET state=?,version=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(state).bind(version).bind(&id).execute(&s.db).await?;
    let _ = s.events.send(Event::ObjectPatched {
        id,
        channel_id: o.summary.channel_id,
        version,
        put: v.put,
        remove: v.remove,
        author_id: a.user.id,
    });
    Ok(Json(ObjectVersion { version }))
}
#[utoipa::path(patch,path="/objects/{id}",params(("id"=String,Path)),request_body=UpdateObject,responses((status=200,body=ObjectSummary)))]
pub(crate) async fn update(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(v): ApiJson<UpdateObject>,
) -> Result<Json<ObjectSummary>> {
    let _guard = s.writes.lock().await;
    let o = load(&s, &id).await?;
    visible(&s, &a.user.id, &o.summary.channel_id).await?;
    if o.summary.kind != "canvas" {
        return Err(Error::forbidden());
    }
    let new_name = v.name.as_deref().map(str::trim).unwrap_or(&o.summary.name);
    let new_name = if new_name.is_empty() {
        "Untitled canvas"
    } else {
        new_name
    };
    name(new_name)?;
    if let Some(upload) = &v.thumbnail_upload_id {
        let ok: i64 = sqlx::query_scalar("SELECT count(*) FROM uploads WHERE id=? AND channel_id=? AND owner_id=? AND complete=1 AND content_type='image/png' AND message_id IS NULL")
            .bind(upload).bind(&o.summary.channel_id).bind(&a.user.id).fetch_one(&s.db).await?;
        if ok != 1 {
            return Err(Error::bad(
                "Thumbnail must be your complete unattached PNG in this channel",
            ));
        }
    }
    sqlx::query("UPDATE objects SET name=?,thumbnail_upload_id=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(new_name).bind(v.thumbnail_upload_id.or(o.summary.thumbnail_upload_id)).bind(&id).execute(&s.db).await?;
    if let Some(message) = o.summary.message_id {
        let _ = s.events.send(Event::MessageEdited(
            messages::get_message(&s, &message).await?,
        ));
    }
    Ok(Json(load(&s, &id).await?.summary))
}
