use crate::{auth::Auth, *};
use axum::{
    body::{to_bytes, Body},
    extract::Path,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
};
use std::collections::BTreeMap;
mod audio;
mod sharing;
pub(crate) use sharing::*;

fn path(s: &AppState, owner: &str, id: &str) -> Result<std::path::PathBuf> {
    if !sound_id(id) {
        return Err(Error::bad("Invalid sound ID"));
    }
    Ok(s.uploads.join("sounds").join(owner).join(id))
}
async fn read(s: &AppState, owner: &str, id: &str) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path(s, owner, id)?)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::missing()
            } else {
                e.into()
            }
        })?;
    let mut bytes = Vec::new();
    file.take(audio::MAX_AUDIO as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > audio::MAX_AUDIO {
        return Err(Error::bad("Stored sound exceeds limit"));
    }
    Ok(bytes)
}
async fn write(s: &AppState, owner: &str, id: &str, bytes: &[u8]) -> Result<()> {
    let dest = path(s, owner, id)?;
    tokio::fs::create_dir_all(dest.parent().unwrap()).await?;
    let temporary = dest.parent().unwrap().join(format!(".{}.tmp", s.id()));
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(temporary, dest).await?;
    Ok(())
}
async fn preferences(s: &AppState, user: &str) -> Result<SoundPreferences> {
    let json: Option<String> =
        sqlx::query_scalar("SELECT preferences FROM user_sounds WHERE user_id=?")
            .bind(user)
            .fetch_optional(&s.db)
            .await?;
    json.map(|v| serde_json::from_str(&v).map_err(|_| Error::bad("Invalid sound preferences")))
        .unwrap_or_else(|| Ok(SoundPreferences::default()))
}
async fn server_pack(s: &AppState) -> Result<SoundPack> {
    let json: Option<String> = sqlx::query_scalar("SELECT pack FROM server_sounds WHERE id=1")
        .fetch_optional(&s.db)
        .await?;
    json.map(|v| serde_json::from_str(&v).map_err(|_| Error::bad("Invalid server pack")))
        .unwrap_or_else(|| Ok(builtin_sound_pack()))
}
async fn resolve(s: &AppState, owner: &str, sound: &SoundRef) -> Option<ResolvedSound> {
    let url = match sound {
        SoundRef::Silent => None,
        SoundRef::Builtin { name } => {
            if !SoundEvent::ALL.iter().any(|e| e.name() == *name) {
                return None;
            }
            Some(format!("/sounds/{name}.wav"))
        }
        SoundRef::Upload { id } => {
            if read(s, owner, id).await.is_err() {
                return None;
            }
            Some(if owner == "server" {
                format!("/settings/sounds/files/{id}")
            } else {
                format!("/users/me/sounds/{id}")
            })
        }
    };
    Some(ResolvedSound {
        sound: sound.clone(),
        url,
    })
}
async fn state(s: &AppState, user: &str) -> Result<SoundState> {
    let preferences = preferences(s, user).await?;
    let server_pack = server_pack(s).await?;
    let builtin = builtin_sound_pack();
    let selected = if preferences.pack_id.as_deref() == Some("den") {
        Some(&builtin)
    } else {
        preferences
            .custom_packs
            .iter()
            .find(|p| Some(&p.id) == preferences.pack_id.as_ref())
    };
    let mut resolved = BTreeMap::new();
    for event in SoundEvent::ALL {
        let choices = [
            preferences.overrides.get(&event).map(|r| (user, r)),
            selected
                .and_then(|p| p.sounds.get(&event))
                .map(|r| (user, r)),
            server_pack.sounds.get(&event).map(|r| ("server", r)),
            builtin.sounds.get(&event).map(|r| ("server", r)),
        ];
        for (owner, sound) in choices.into_iter().flatten() {
            if let Some(value) = resolve(s, owner, sound).await {
                resolved.insert(event, value);
                break;
            }
        }
    }
    Ok(SoundState {
        preferences,
        server_pack,
        resolved,
    })
}
async fn save(s: &AppState, user: &str, value: &SoundPreferences) -> Result<()> {
    value.validate().map_err(Error::bad)?;
    sqlx::query("INSERT INTO user_sounds(user_id,preferences) VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET preferences=excluded.preferences").bind(user).bind(serde_json::to_string(value).unwrap()).execute(&s.db).await?;
    let _ = s.events.send(Event::SoundsUpdated {
        user_id: Some(user.into()),
    });
    Ok(())
}
#[utoipa::path(get,path="/users/me/sounds",responses((status=200,body=SoundState)))]
pub(crate) async fn get(State(s): State<AppState>, a: Auth) -> Result<Json<SoundState>> {
    Ok(Json(state(&s, &a.user.id).await?))
}
#[utoipa::path(put,path="/users/me/sounds",request_body=SoundPreferences,responses((status=200,body=SoundState)))]
pub(crate) async fn put(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(value): ApiJson<SoundPreferences>,
) -> Result<Json<SoundState>> {
    let _guard = s.writes.lock().await;
    save(&s, &a.user.id, &value).await?;
    Ok(Json(state(&s, &a.user.id).await?))
}
#[utoipa::path(put,path="/users/me/sounds/{id}",params(("id"=String,Path)),request_body(content=String,content_type="audio/wav"),responses((status=200,body=SoundRef)))]
pub(crate) async fn put_file(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<SoundRef>> {
    path(&s, &a.user.id, &id)?;
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !matches!(mime, "audio/ogg" | "audio/mpeg" | "audio/wav") {
        return Err(Error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "sound_type",
            "Use Ogg Vorbis, MP3 or WAV".into(),
        ));
    }
    let data = to_bytes(body, audio::MAX_AUDIO)
        .await
        .map_err(|_| {
            Error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "sound_size",
                "Sounds must be at most 512 KiB".into(),
            )
        })?
        .to_vec();
    let data = tokio::task::spawn_blocking(move || {
        audio::validate(&data)?;
        Ok::<_, Error>(data)
    })
    .await
    .map_err(|_| Error::bad("Audio validation failed"))??;
    if audio::mime(&data) != mime {
        return Err(Error::bad("Audio does not match its content type"));
    }
    let _guard = s.writes.lock().await;
    // Immutable IDs keep references, browser caches and ZIP exports consistent.
    if let Ok(existing) = read(&s, &a.user.id, &id).await {
        if existing != data {
            return Err(Error::conflict("Use a new ID to replace a sound"));
        }
    }
    write(&s, &a.user.id, &id, &data).await?;
    Ok(Json(SoundRef::Upload { id }))
}
fn audio_response(data: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, audio::mime(&data)),
            (header::CACHE_CONTROL, "private, max-age=3600"),
            (header::VARY, "Authorization, Cookie"),
        ],
        data,
    )
        .into_response()
}
#[utoipa::path(get,path="/users/me/sounds/{id}",params(("id"=String,Path)),responses((status=200,description="Audio bytes")))]
pub(crate) async fn get_file(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Response> {
    Ok(audio_response(read(&s, &a.user.id, &id).await?))
}
#[utoipa::path(get,path="/settings/sounds/files/{id}",params(("id"=String,Path)),responses((status=200,description="Server audio bytes")))]
pub(crate) async fn server_file(
    State(s): State<AppState>,
    _a: Auth,
    Path(id): Path<String>,
) -> Result<Response> {
    Ok(audio_response(read(&s, "server", &id).await?))
}
#[utoipa::path(put,path="/settings/sounds",request_body=SoundPack,responses((status=200,body=SoundState)))]
pub(crate) async fn put_server(
    State(s): State<AppState>,
    a: Auth,
    ApiJson(mut pack): ApiJson<SoundPack>,
) -> Result<Json<SoundState>> {
    if a.user.role != Role::Admin {
        return Err(Error::forbidden());
    }
    pack.validate().map_err(Error::bad)?;
    let _guard = s.writes.lock().await;
    let mut files = BTreeMap::new();
    for sound in pack.sounds.values_mut() {
        if let SoundRef::Upload { id } = sound {
            let data = match read(&s, &a.user.id, id).await {
                Ok(v) => v,
                Err(_) => read(&s, "server", id).await?,
            };
            audio::validate(&data)?;
            use sha2::{Digest, Sha256};
            *id = format!("{:x}", Sha256::digest(&data));
            files.insert(id.clone(), data);
        }
    }
    for (id, data) in files {
        write(&s, "server", &id, &data).await?;
    }
    sqlx::query("INSERT INTO server_sounds(id,pack) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET pack=excluded.pack").bind(serde_json::to_string(&pack).unwrap()).execute(&s.db).await?;
    let _ = s.events.send(Event::SoundsUpdated { user_id: None });
    Ok(Json(state(&s, &a.user.id).await?))
}

#[utoipa::path(get,path="/settings/sounds",responses((status=200,body=SoundPack)))]
pub(crate) async fn get_server(State(s): State<AppState>, _a: Auth) -> Result<Json<SoundPack>> {
    Ok(Json(server_pack(&s).await?))
}
