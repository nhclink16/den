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
            ..Default::default()
        })
        .to_jwt()
        .map_err(|_| unavailable())?;
    Ok(Json(CallToken {
        url: lk.url.clone(),
        token,
    }))
}

#[utoipa::path(get,path="/calls",responses((status=200,body=Vec<CallState>)))]
pub(crate) async fn list(State(s): State<AppState>, a: Auth) -> Result<Json<Vec<CallState>>> {
    let Json(channels) = chat::channels(State(s.clone()), a).await?;
    let mut calls = s.calls.lock().await;
    // Repair missed webhooks and server restarts during resync. A media outage
    // must not prevent the text client from booting; retain the last snapshot.
    if let Some(lk) = &s.livekit {
        let url = lk
            .url
            .replacen("wss://", "https://", 1)
            .replacen("ws://", "http://", 1);
        let client = RoomClient::with_api_key(&url, &lk.key, &lk.secret)
            .with_request_timeout(Duration::from_secs(2));
        if let Ok(rooms) = client
            .list_rooms(channels.iter().map(|c| c.id.clone()).collect())
            .await
        {
            for c in &channels {
                if rooms.iter().any(|r| r.name == c.id) {
                    if let Ok(users) = client.list_participants(&c.id).await {
                        calls.insert(
                            c.id.clone(),
                            users.into_iter().map(|p| (p.identity, p.sid)).collect(),
                        );
                    }
                } else {
                    calls.remove(&c.id);
                }
            }
        }
    }
    Ok(Json(
        channels
            .into_iter()
            .map(|c| {
                let participant_ids = user_ids(calls.get(&c.id));
                CallState {
                    channel_id: c.id,
                    participant_ids,
                }
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
    let mut calls = s.calls.lock().await;
    if event.event == "room_finished" {
        calls.remove(&room.name);
    } else if let Some(p) = event.participant {
        // Shared dev LiveKit may send rooms belonging to another Den database.
        if visible(&s, user_id(&p.identity), &room.name).await.is_err() {
            return Ok(StatusCode::NO_CONTENT);
        }
        let users = calls.entry(room.name.clone()).or_default();
        if event.event == "participant_joined" {
            users.insert(p.identity, p.sid);
        } else if users.get(&p.identity) == Some(&p.sid) {
            users.remove(&p.identity);
        }
    }
    let participant_ids = user_ids(calls.get(&room.name));
    let _ = s.events.send(Event::CallState {
        channel_id: room.name,
        participant_ids,
    });
    Ok(StatusCode::NO_CONTENT)
}

// Identity is account:connection. Keep legacy account-only identities working
// while already-connected clients finish calls after a rolling deployment.
fn user_id(identity: &str) -> &str {
    identity.split_once(':').map_or(identity, |(user, _)| user)
}

fn user_ids(participants: Option<&HashMap<String, String>>) -> Vec<String> {
    let mut ids = participants
        .into_iter()
        .flat_map(|p| p.keys().map(|id| user_id(id).to_owned()))
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}
