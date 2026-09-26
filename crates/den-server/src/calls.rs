use crate::{auth::Auth, chat::visible, *};
use axum::{extract::Path, http::HeaderMap};
use livekit_api::{
    access_token::{AccessToken, TokenVerifier, VideoGrants},
    services::room::RoomClient,
    webhooks::WebhookReceiver,
};

pub struct LiveKit {
    pub url: String,
    pub key: String,
    pub secret: String,
}
fn unavailable() -> Error {
    Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "voice_unavailable",
        "Voice isn't set up on this server yet.".into(),
    )
}
#[utoipa::path(post,path="/calls/{channel_id}/token",params(("channel_id"=String,Path)),responses((status=200,body=CallToken),(status=404,body=ApiError),(status=503,body=ApiError)))]
pub(crate) async fn token(
    State(s): State<AppState>,
    a: Auth,
    Path(channel_id): Path<String>,
) -> Result<Json<CallToken>> {
    visible(&s, &a.user.id, &channel_id).await?;
    let lk = s.livekit.as_ref().ok_or_else(unavailable)?;
    let token = AccessToken::with_api_key(&lk.key, &lk.secret)
        .with_identity(&format!("{}:{}", a.user.id, s.id()))
        .with_name(&a.user.display_name)
        .with_ttl(Duration::from_secs(600))
        .with_grants(VideoGrants {
            room_join: true,
            room: channel_id,
            can_publish_data: false,
            can_update_own_metadata: true,
            ..Default::default()
        })
        .to_jwt()
        .map_err(|_| unavailable())?;
    Ok(Json(CallToken {
        url: lk.url.clone(),
        token,
    }))
}

#[derive(Clone)]
pub(crate) struct MediaRoom {
    pub sid: String,
    pub created_at: i64,
    pub finished: bool,
}

/// Fetch outside the write lock. A newer webhook invalidates this snapshot.
/// True when this call applied a LiveKit read of every room, or there is no
/// LiveKit (so no call exists). False when LiveKit could not be read or the
/// snapshot was discarded, since the call map may then be stale.
pub(crate) async fn refresh(s: &AppState, channels: &[String]) -> Result<bool> {
    use std::sync::atomic::Ordering;
    let Some(lk) = &s.livekit else {
        return Ok(true);
    };
    let revision = s.call_revision.load(Ordering::SeqCst);
    let url = lk
        .url
        .replacen("wss://", "https://", 1)
        .replacen("ws://", "http://", 1);
    let client = RoomClient::with_api_key(&url, &lk.key, &lk.secret)
        .with_request_timeout(Duration::from_secs(2));
    let Ok(rooms) = client.list_rooms(channels.to_vec()).await else {
        return Ok(false);
    };
    let mut verified = true;
    let mut snapshots = Vec::new();
    for channel in channels {
        if let Some(room) = rooms.iter().find(|r| &r.name == channel) {
            if let Ok(users) = client.list_participants(channel).await {
                snapshots.push((
                    channel.clone(),
                    Some(MediaRoom {
                        sid: room.sid.clone(),
                        created_at: room.creation_time_ms.max(room.creation_time * 1000),
                        finished: false,
                    }),
                    users
                        .into_iter()
                        .map(|p| (p.identity, p.sid))
                        .collect::<HashMap<_, _>>(),
                ));
            } else {
                verified = false;
            }
        } else {
            snapshots.push((channel.clone(), None, HashMap::new()));
        }
    }
    let _guard = s.writes.lock().await;
    if s.call_revision.load(Ordering::SeqCst) != revision {
        // Discarded: a webhook, possibly for another room, landed meanwhile.
        return Ok(false);
    }
    let mut calls = s.calls.lock().await;
    let mut known = s.call_rooms.lock().await;
    for (channel, room, users) in snapshots {
        let changed = calls.get(&channel) != Some(&users);
        if let Some(room) = room {
            known.insert(channel.clone(), room);
        } else if let Some(room) = known.get_mut(&channel) {
            room.finished = true;
        }
        let participants = user_ids(Some(&users));
        calls.insert(channel.clone(), users);
        if changed {
            let _ = s.events.send(Event::CallState {
                channel_id: channel.clone(),
                participant_ids: participants.clone(),
            });
        }
        invitation_state::media(s, &channel, &participants, true).await?;
    }
    s.call_revision.fetch_add(1, Ordering::SeqCst);
    Ok(verified)
}

#[utoipa::path(get,path="/calls",responses((status=200,body=Vec<CallState>)))]
pub(crate) async fn list(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<CallState>>> {
    let Json(channels) = chat::channels(State(s.clone()), a).await?;
    refresh(
        &s,
        &channels.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
    )
    .await?;
    let calls = s.calls.lock().await;
    Ok(Json(
        channels
            .into_iter()
            .map(|c| CallState {
                participant_ids: user_ids(calls.get(&c.id)),
                channel_id: c.id,
            })
            .collect(),
    ))
}

#[utoipa::path(post,path="/livekit/webhook",request_body(content=String,content_type="application/webhook+json"),description="LiveKit signed webhook. Authorization is the LiveKit JWT over the raw body, not a Den session.",responses((status=204),(status=401,body=ApiError),(status=503,body=ApiError)))]
pub(crate) async fn webhook(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<StatusCode> {
    let lk = s.livekit.as_ref().ok_or_else(unavailable)?;
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(Error::unauthorized)?;
    let event = WebhookReceiver::new(TokenVerifier::with_api_key(&lk.key, &lk.secret))
        .receive(&body, token)
        .map_err(|_| Error::unauthorized())?;
    let Some(room) = event.room else {
        return Ok(StatusCode::NO_CONTENT);
    };
    if !matches!(
        event.event.as_str(),
        "participant_joined" | "participant_left" | "room_finished"
    ) {
        return Ok(StatusCode::NO_CONTENT);
    }
    // After restart an unmatched finish may describe an older room generation.
    // A provider snapshot must establish which room exists before using absence.
    if event.event == "room_finished" && !s.call_rooms.lock().await.contains_key(&room.name) {
        refresh(&s, std::slice::from_ref(&room.name)).await?;
    }
    use std::sync::atomic::Ordering;
    let _guard = s.writes.lock().await;
    let mut calls = s.calls.lock().await;
    let mut known = s.call_rooms.lock().await;
    if event.event == "room_finished" {
        if known
            .get(&room.name)
            .is_none_or(|current| current.sid != room.sid || current.finished)
        {
            return Ok(StatusCode::NO_CONTENT);
        }
        calls.remove(&room.name);
        known.insert(
            room.name.clone(),
            MediaRoom {
                sid: room.sid.clone(),
                created_at: room.creation_time_ms.max(room.creation_time * 1000),
                finished: true,
            },
        );
    } else if let Some(p) = event.participant {
        if visible(&s, user_id(&p.identity), &room.name).await.is_err() {
            return Ok(StatusCode::NO_CONTENT);
        }
        let created_at = room.creation_time_ms.max(room.creation_time * 1000);
        if let Some(current) = known.get(&room.name) {
            if current.sid != room.sid {
                if event.event != "participant_joined" || created_at < current.created_at {
                    return Ok(StatusCode::NO_CONTENT);
                }
                calls.remove(&room.name);
            } else if current.finished {
                return Ok(StatusCode::NO_CONTENT);
            }
        }
        known.insert(
            room.name.clone(),
            MediaRoom {
                sid: room.sid.clone(),
                created_at,
                finished: false,
            },
        );
        let users = calls.entry(room.name.clone()).or_default();
        if event.event == "participant_joined" {
            users.insert(p.identity, p.sid);
        } else if users.get(&p.identity) == Some(&p.sid) {
            users.remove(&p.identity);
        }
    }
    s.call_revision.fetch_add(1, Ordering::SeqCst);
    let participant_ids = user_ids(calls.get(&room.name));
    let _ = s.events.send(Event::CallState {
        channel_id: room.name.clone(),
        participant_ids: participant_ids.clone(),
    });
    invitation_state::media(
        &s,
        &room.name,
        &participant_ids,
        event.event == "room_finished",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Identity is account:connection. Keep legacy account-only identities working
// while already-connected clients finish calls after a rolling deployment.
fn user_id(identity: &str) -> &str {
    identity.split_once(':').map_or(identity, |(user, _)| user)
}

pub(crate) fn user_ids(participants: Option<&HashMap<String, String>>) -> Vec<String> {
    let mut ids = participants
        .into_iter()
        .flat_map(|p| {
            p.keys()
                .filter(|id| !id.starts_with("den-dj-"))
                .map(|id| user_id(id).to_owned())
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A webhook for another room lands while LiveKit is being asked. The
    /// snapshot is then thrown away, so it verified nothing about this room.
    #[tokio::test]
    async fn a_snapshot_discarded_for_a_newer_webhook_is_not_verification() {
        let dir = std::env::temp_dir().join(format!("den-refresh-{}", ulid::Ulid::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let s = AppState::open(
            dir.join("den.db"),
            dir.join("uploads"),
            dir.join("bootstrap.key"),
            "http://127.0.0.1:7000".into(),
            1024,
        )
        .await
        .unwrap()
        .with_livekit(
            url,
            "key".into(),
            "secret-at-least-thirty-two-bytes!".into(),
        );
        let webhook = s.clone();
        let stub = Router::new().fallback(move || {
            let s = webhook.clone();
            async move {
                s.call_revision
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // An empty body is an empty ListRoomsResponse.
                (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/protobuf")],
                    Vec::<u8>::new(),
                )
            }
        });
        let server = tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
        assert!(matches!(refresh(&s, &["room".into()]).await, Ok(false)));
        server.abort();
        let _ = std::fs::remove_dir_all(dir);
    }
}
