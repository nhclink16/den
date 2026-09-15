use super::*;
use audio::Bundle;
use sha2::{Digest, Sha256};

async fn install(
    s: &AppState,
    user: &str,
    mut bundle: Bundle,
    keep_pack: bool,
) -> Result<SoundState> {
    // unpack validated every entry before this function can write anything.
    let _guard = s.writes.lock().await;
    let mut prefs = preferences(s, user).await?;
    bundle.pack.id = s.id();
    let mut files = BTreeMap::new();
    for sound in bundle.pack.sounds.values_mut() {
        if let SoundRef::Upload { id } = sound {
            let data = bundle
                .files
                .get(id)
                .ok_or_else(|| Error::bad("Missing pack audio"))?;
            *id = format!("{:x}", Sha256::digest(data));
            files.insert(id.clone(), data);
        }
    }
    if keep_pack {
        prefs.custom_packs.push(bundle.pack.clone());
    }
    // Layer only authored events. Omitted events retain their current source.
    prefs.overrides.extend(bundle.pack.sounds);
    prefs.validate().map_err(Error::bad)?;
    for (id, data) in files {
        write(s, user, &id, data).await?;
    }
    save(s, user, &prefs).await?;
    state(s, user).await
}
#[utoipa::path(put,path="/users/me/sounds/import",request_body(content=String,content_type="application/zip"),responses((status=200,body=SoundState)))]
pub(crate) async fn import_pack(
    State(s): State<AppState>,
    a: Auth,
    body: Body,
) -> Result<Json<SoundState>> {
    let bytes = to_bytes(body, audio::MAX_PACK)
        .await
        .map_err(|_| Error::bad("Sound pack exceeds 6 MiB"))?
        .to_vec();
    let bundle = decode_bundle(bytes).await?;
    Ok(Json(install(&s, &a.user.id, bundle, true).await?))
}
async fn decode_bundle(bytes: Vec<u8>) -> Result<Bundle> {
    tokio::task::spawn_blocking(move || audio::unpack(bytes))
        .await
        .map_err(|_| Error::bad("Pack validation failed"))?
}
#[utoipa::path(get,path="/users/me/sounds/export",responses((status=200,description="Current effective pack as .den-sounds.zip")))]
pub(crate) async fn export_pack(State(s): State<AppState>, a: Auth) -> Result<Response> {
    let _guard = s.writes.lock().await;
    let value = state(&s, &a.user.id).await?;
    let name = value
        .preferences
        .custom_packs
        .iter()
        .find(|p| Some(&p.id) == value.preferences.pack_id.as_ref())
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "My sounds".into());
    let mut pack = SoundPack {
        id: "my-sounds".into(),
        name,
        sounds: BTreeMap::new(),
    };
    let mut files = BTreeMap::new();
    for (event, resolved) in value.resolved {
        if let SoundRef::Upload { id } = &resolved.sound {
            let owner = if resolved
                .url
                .as_ref()
                .is_some_and(|url| url.starts_with("/settings/"))
            {
                "server"
            } else {
                &a.user.id
            };
            files.insert(id.clone(), read(&s, owner, id).await?);
        }
        pack.sounds.insert(event, resolved.sound);
    }
    let bytes = audio::pack(Bundle { pack, files })?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=My-sounds.den-sounds.zip",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}
async fn attachment(s: &AppState, a: &Auth, id: &str) -> Result<Bundle> {
    let upload = uploads::load(s, id).await?;
    chat::visible(s, &a.user.id, &upload.channel_id).await?;
    if !upload.complete || upload.size > audio::MAX_PACK as i64 {
        return Err(Error::bad("Sound attachment is incomplete or too large"));
    }
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(s.uploads.join(&upload.id)).await?;
    let mut data = Vec::new();
    file.take(audio::MAX_PACK as u64 + 1)
        .read_to_end(&mut data)
        .await?;
    if upload.filename.ends_with(".den-sounds.zip") {
        return decode_bundle(data).await;
    }
    if !matches!(
        upload.content_type.as_str(),
        "audio/ogg" | "audio/mpeg" | "audio/wav"
    ) {
        return Err(Error::bad("Not a sound attachment"));
    }
    tokio::task::spawn_blocking(move || {
        audio::validate(&data)?;
        Ok(Bundle {
            pack: SoundPack {
                id: upload.id,
                name: upload.filename.chars().take(60).collect(),
                sounds: BTreeMap::from([(
                    SoundEvent::Message,
                    SoundRef::Upload { id: "sound".into() },
                )]),
            },
            files: BTreeMap::from([("sound".into(), data)]),
        })
    })
    .await
    .map_err(|_| Error::bad("Audio validation failed"))?
}
#[utoipa::path(get,path="/uploads/{id}/sounds",params(("id"=String,Path)),responses((status=200,body=SoundPack)))]
pub(crate) async fn preview(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
) -> Result<Json<SoundPack>> {
    Ok(Json(attachment(&s, &a, &id).await?.pack))
}
#[utoipa::path(get,path="/uploads/{id}/sounds/{sound}",params(("id"=String,Path),("sound"=String,Path)),responses((status=200,description="Validated attachment audio")))]
pub(crate) async fn preview_file(
    State(s): State<AppState>,
    a: Auth,
    Path((id, sound)): Path<(String, String)>,
) -> Result<Response> {
    Ok(audio_response(
        attachment(&s, &a, &id)
            .await?
            .files
            .remove(&sound)
            .ok_or_else(|| Error::bad("Unknown pack sound"))?,
    ))
}
#[utoipa::path(post,path="/uploads/{id}/sounds",params(("id"=String,Path)),request_body=InstallSound,responses((status=200,body=SoundState)))]
pub(crate) async fn install_attachment(
    State(s): State<AppState>,
    a: Auth,
    Path(id): Path<String>,
    ApiJson(choice): ApiJson<InstallSound>,
) -> Result<Json<SoundState>> {
    let mut bundle = attachment(&s, &a, &id).await?;
    if let Some(event) = choice.event {
        if bundle.files.len() != 1 || bundle.pack.sounds.len() != 1 {
            return Err(Error::bad("Choose an event only for a single sound"));
        }
        let sound = bundle.pack.sounds.into_values().next().unwrap();
        bundle.pack.sounds = BTreeMap::from([(event, sound)]);
    }
    Ok(Json(
        install(&s, &a.user.id, bundle, choice.event.is_none()).await?,
    ))
}
