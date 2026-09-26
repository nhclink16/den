//! Jam cards. A Jam is a share link pinned to a room, plus the Den members who
//! pressed Join. Spotify cannot tell us who is listening to a Jam, so this
//! counts clicks and says so; `now_playing` is the host's own playback and
//! nothing else.
use crate::{auth::Auth, chat::visible, *};
use axum::extract::Path;
use sqlx::Row;

async fn authorize(s: &AppState, a: &Auth, room: &str) -> Result<Channel> {
    visible(s, &a.user.id, room).await
}

/// People in the call, by user id. The room's own DJ is not a person.
async fn people(s: &AppState, channel_id: &str) -> Vec<String> {
    calls::user_ids(s.calls.lock().await.get(channel_id))
}
async fn in_call(s: &AppState, user_id: &str, channel_id: &str) -> bool {
    people(s, channel_id).await.iter().any(|u| u == user_id)
}

/// Share links only. Everything else gets the same one-line answer.
fn jam_url(input: &str) -> Result<String> {
    let bad = || Error::bad("Paste a Spotify Jam link.");
    let u = reqwest::Url::parse(input.trim()).map_err(|_| bad())?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
    {
        return Err(Error::bad("Paste an HTTPS Spotify Jam link."));
    }
    let segments: Vec<_> = u.path().trim_matches('/').split('/').collect();
    let token = |v: &str| {
        !v.is_empty()
            && v.len() <= 64
            && v.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    };
    let path = match (u.host_str().unwrap_or(""), segments.as_slice()) {
        ("open.spotify.com", ["jam", id]) if token(id) => format!("/jam/{id}"),
        ("spotify.link", [id]) if token(id) => format!("/{id}"),
        _ => return Err(bad()),
    };
    // Jam links carry a share token. Keep that one parameter, drop the rest.
    let share = u
        .query_pairs()
        .find(|(k, v)| k == "si" && token(v))
        .map(|(_, v)| format!("?si={v}"))
        .unwrap_or_default();
    Ok(format!(
        "https://{}{path}{share}",
        u.host_str().unwrap_or("")
    ))
}

/// The live Jam for a room, without reading anyone's playback.
async fn live(s: &AppState, room: &str) -> Result<Option<Jam>> {
    let Some(r) = sqlx::query(
        "SELECT id,url,host_id,started_at FROM jams WHERE channel_id=? AND ended_at IS NULL",
    )
    .bind(room)
    .fetch_optional(&s.db)
    .await?
    else {
        return Ok(None);
    };
    let id: String = r.get("id");
    let joined =
        sqlx::query("SELECT user_id FROM jam_joins WHERE jam_id=? ORDER BY joined_at,user_id")
            .bind(&id)
            .fetch_all(&s.db)
            .await?
            .into_iter()
            .map(|r| r.get::<String, _>("user_id"))
            .collect();
    Ok(Some(Jam {
        id,
        channel_id: room.into(),
        url: r.get("url"),
        host_id: r.get("host_id"),
        started_at: r.get("started_at"),
        joined_user_ids: joined,
        now_playing: None,
    }))
}

/// Broadcast without an outbound request: the last sample if one is warm, so
/// pressing Join does not blink the track off the card.
async fn broadcast(s: &AppState, room: &str, jam: Option<Jam>) {
    let jam = match jam {
        Some(mut jam) => {
            jam.now_playing = spotify::cached(s, &jam.host_id).await;
            Some(jam)
        }
        None => None,
    };
    let _ = s.events.send(Event::JamUpdated {
        channel_id: room.into(),
        jam,
    });
}

/// The caller holds the write lock, so the end, its event and the queue
/// resuming land together, never interleaved with a start.
async fn end_jam(s: &AppState, jam_id: &str, room: &str) -> Result<()> {
    let ended = sqlx::query("UPDATE jams SET ended_at=? WHERE id=? AND ended_at IS NULL")
        .bind(now())
        .bind(jam_id)
        .execute(&s.db)
        .await?
        .rows_affected()
        > 0;
    if ended {
        broadcast(s, room, None).await;
        music::jam_resume(s, room).await?;
    }
    Ok(())
}

#[utoipa::path(get,path="/rooms/{id}/jam",params(("id"=String,Path)),responses((status=200,body=RoomJam)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
) -> Result<Json<RoomJam>> {
    authorize(&s, &a, &room).await?;
    let mut jam = live(&s, &room).await?;
    if let Some(jam) = jam.as_mut() {
        jam.now_playing = spotify::now_playing(&s, &jam.host_id).await;
    }
    Ok(Json(RoomJam { jam }))
}

#[utoipa::path(post,path="/rooms/{id}/jam",params(("id"=String,Path)),request_body=StartJam,responses((status=200,body=Jam)))]
pub(crate) async fn start(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
    ApiJson(v): ApiJson<StartJam>,
) -> Result<Json<Jam>> {
    // Jams belong to calls: a voice room or a DM call, and only from inside it.
    if authorize(&s, &a, &room).await?.kind == ChannelKind::Text {
        return Err(Error::bad("Start a Jam from a call, not a text room."));
    }
    // The call map follows LiveKit webhooks. Someone who joined a moment ago
    // may not be in it yet, so ask LiveKit before refusing them.
    if !in_call(&s, &a.user.id, &room).await {
        calls::refresh(&s, std::slice::from_ref(&room)).await?;
        if !in_call(&s, &a.user.id, &room).await {
            return Err(Error::bad("Join the call to start a Jam."));
        }
    }
    let url = jam_url(&v.url)?;
    let _guard = s.writes.lock().await;
    // Replacing a Jam ends it, so it takes the same right as ending it.
    if let Some(current) = live(&s, &room).await? {
        if current.host_id != a.user.id {
            a.admin()?;
        }
    }
    let time = now();
    let id = s.id();
    let mut tx = s.db.begin().await?;
    // A room has one live Jam. Starting a new one retires the old one.
    sqlx::query("UPDATE jams SET ended_at=? WHERE channel_id=? AND ended_at IS NULL")
        .bind(time)
        .bind(&room)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO jams(id,channel_id,url,host_id,started_at) VALUES(?,?,?,?,?)")
        .bind(&id)
        .bind(&room)
        .bind(&url)
        .bind(&a.user.id)
        .bind(time)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO jam_joins(jam_id,user_id,joined_at) VALUES(?,?,?)")
        .bind(&id)
        .bind(&a.user.id)
        .bind(time)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let jam = live(&s, &room).await?.ok_or_else(Error::missing)?;
    // Still under the lock: an End cannot slip between the start's event and
    // its queue pause.
    broadcast(&s, &room, Some(jam.clone())).await;
    if let Err(e) = music::jam_pause(&s, &room).await {
        tracing::warn!(code = e.1, "Could not pause the queue for a Jam");
    }
    Ok(Json(jam))
}

#[utoipa::path(post,path="/rooms/{id}/jam/join",params(("id"=String,Path)),responses((status=200,body=Jam)))]
pub(crate) async fn join(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
) -> Result<Json<Jam>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let jam = live(&s, &room).await?.ok_or_else(Error::missing)?;
    // Joining twice from two tabs is one join, not an error and not an event.
    let inserted = sqlx::query("INSERT INTO jam_joins(jam_id,user_id,joined_at) VALUES(?,?,?) ON CONFLICT(jam_id,user_id) DO NOTHING")
        .bind(&jam.id)
        .bind(&a.user.id)
        .bind(now())
        .execute(&s.db)
        .await?
        .rows_affected()
        > 0;
    let jam = live(&s, &room).await?.ok_or_else(Error::missing)?;
    if inserted {
        broadcast(&s, &room, Some(jam.clone())).await;
    }
    Ok(Json(jam))
}

#[utoipa::path(delete,path="/rooms/{id}/jam",params(("id"=String,Path)),responses((status=204,description="Jam ended")))]
pub(crate) async fn end(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
) -> Result<StatusCode> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let jam = live(&s, &room).await?.ok_or_else(Error::missing)?;
    if jam.host_id != a.user.id {
        a.admin()?;
    }
    end_jam(&s, &jam.id, &room).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Background task: auto-end Jams when the call has been empty for 10 minutes
/// or (when the host has Spotify) the host has been idle for 30 minutes.
pub(crate) async fn watcher(inner: std::sync::Weak<Inner>) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let Some(inner) = inner.upgrade() else { return };
        let s = AppState(inner);
        if let Err(e) = watch_tick(&s, now()).await {
            tracing::warn!(code = e.1, "Jam watcher tick failed");
        }
    }
}

const EMPTY_CALL_SECS: i64 = 10 * 60;
const HOST_IDLE_SECS: i64 = 30 * 60;

/// One pass over live Jams at time `at` (Unix seconds). A Jam that has not yet
/// been seen empty starts its timer now, so ten minutes means ten minutes of
/// observed emptiness, never "empty since before the server started".
pub(crate) async fn watch_tick(s: &AppState, at: i64) -> Result<()> {
    let rows = sqlx::query(
        "SELECT j.id,j.channel_id,j.host_id,j.started_at,j.empty_since,j.last_playing_at, \
         EXISTS(SELECT 1 FROM spotify_accounts a WHERE a.user_id=j.host_id AND a.needs_reauth=0) AS spotify \
         FROM jams j WHERE j.ended_at IS NULL",
    )
    .fetch_all(&s.db)
    .await?;
    // The call map is empty after a restart until webhooks arrive. "Nobody is
    // here" only counts once LiveKit has confirmed it.
    let rooms: Vec<String> = rows.iter().map(|r| r.get("channel_id")).collect();
    let verified = rooms.is_empty() || calls::refresh(s, &rooms).await?;
    for row in rows {
        let id: String = row.get("id");
        let room: String = row.get("channel_id");
        let host: String = row.get("host_id");
        let empty_since: Option<i64> = row.get("empty_since");
        let empty = people(s, &room).await.is_empty();
        match (empty, empty_since) {
            _ if !verified => {}
            (true, Some(since)) if at - since >= EMPTY_CALL_SECS => {
                let _guard = s.writes.lock().await;
                end_jam(s, &id, &room).await?;
                continue;
            }
            (true, None) => {
                sqlx::query("UPDATE jams SET empty_since=? WHERE id=?")
                    .bind(at)
                    .bind(&id)
                    .execute(&s.db)
                    .await?;
            }
            (false, Some(_)) => {
                sqlx::query("UPDATE jams SET empty_since=NULL WHERE id=?")
                    .bind(&id)
                    .execute(&s.db)
                    .await?;
            }
            _ => {}
        }
        // Idle only counts for a host whose playback we can read, and only
        // silence Spotify actually reported. A read that failed (rate limit,
        // timeout, dead grant) restarts the count like playback does, so thirty
        // minutes means thirty minutes of observed silence.
        if !row.get::<bool, _>("spotify") {
            continue;
        }
        if spotify::is_playing(s, &host).await == Some(false) {
            let last: i64 = row
                .get::<Option<i64>, _>("last_playing_at")
                .unwrap_or_else(|| row.get("started_at"));
            if at - last >= HOST_IDLE_SECS {
                let _guard = s.writes.lock().await;
                end_jam(s, &id, &room).await?;
            }
        } else {
            sqlx::query("UPDATE jams SET last_playing_at=? WHERE id=?")
                .bind(at)
                .bind(&id)
                .execute(&s.db)
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An End that lands while a start is between its commit and its queue
    /// pause. Holding the playback cache freezes the start exactly there: its
    /// broadcast reads the host's cached track. Only in-crate code can hold it.
    #[tokio::test]
    async fn an_end_racing_a_start_leaves_the_queue_and_events_consistent() {
        let dir = std::env::temp_dir().join(format!("den-jam-race-{}", ulid::Ulid::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let s = AppState::open(
            dir.join("den.db"),
            dir.join("uploads"),
            dir.join("bootstrap.key"),
            base.clone(),
            1024,
        )
        .await
        .unwrap();
        let app = crate::router_with_web(s.clone(), dir.join("spa"));
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap()
        });
        let http = reqwest::Client::new();
        let key = std::fs::read_to_string(dir.join("bootstrap.key")).unwrap();
        let session: serde_json::Value = http
            .post(format!("{base}/auth/init"))
            .json(&serde_json::json!({"username":"host","password":"test-password-123","bootstrap_token":key}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let token = session["token"].as_str().unwrap().to_string();
        let user = session["user"]["id"].as_str().unwrap().to_string();
        let room: String = sqlx::query_scalar("SELECT id FROM channels WHERE kind='voice' LIMIT 1")
            .fetch_one(&s.db)
            .await
            .unwrap();
        s.calls
            .lock()
            .await
            .entry(room.clone())
            .or_default()
            .insert(format!("{user}:test"), "PA_host".into());
        let mut events = s.events.subscribe();
        let url = format!("{base}/rooms/{room}/jam");

        let cache = s.spotify.playback.lock().await;
        let start = tokio::spawn({
            let (http, url, token) = (http.clone(), url.clone(), token.clone());
            async move {
                http.post(url)
                    .bearer_auth(token)
                    .json(&serde_json::json!({"url":"https://spotify.link/race"}))
                    .send()
                    .await
                    .unwrap()
                    .status()
            }
        });
        // Wait for the commit; the start is now parked on the cache.
        while sqlx::query_scalar::<_, i64>("SELECT count(*) FROM jams WHERE ended_at IS NULL")
            .fetch_one(&s.db)
            .await
            .unwrap()
            == 0
        {
            tokio::task::yield_now().await;
        }
        let end = tokio::spawn({
            let (http, url, token) = (http.clone(), url.clone(), token.clone());
            async move {
                http.delete(url)
                    .bearer_auth(token)
                    .send()
                    .await
                    .unwrap()
                    .status()
            }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(cache);
        assert_eq!(start.await.unwrap(), StatusCode::OK);
        assert_eq!(end.await.unwrap(), StatusCode::NO_CONTENT);

        let jam_paused: bool =
            sqlx::query_scalar("SELECT jam_paused FROM music_rooms WHERE room_id=?")
                .bind(&room)
                .fetch_one(&s.db)
                .await
                .unwrap();
        assert!(!jam_paused, "the queue is paused for a Jam that has ended");
        let mut last = None;
        while let Ok(event) = events.try_recv() {
            if let Event::JamUpdated { jam, .. } = event {
                last = Some(jam);
            }
        }
        assert_eq!(last, Some(None), "the last Jam event must be the end");
        server.abort();
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn only_spotify_share_links_become_jams() {
        assert_eq!(
            jam_url(" https://open.spotify.com/jam/abc123?si=xyz&utm=1 ")
                .ok()
                .expect("share link"),
            "https://open.spotify.com/jam/abc123?si=xyz"
        );
        assert_eq!(
            jam_url("https://spotify.link/aBc-1_2")
                .ok()
                .expect("short link"),
            "https://spotify.link/aBc-1_2"
        );
        for bad in [
            "http://open.spotify.com/jam/abc123",
            "https://open.spotify.com:8443/jam/abc123",
            "https://user@open.spotify.com/jam/abc123",
            "https://open.spotify.com/track/abc123",
            "https://open.spotify.com/jam/",
            "https://open.spotify.com.evil.test/jam/abc123",
            "https://evil.test/jam/abc123",
            "javascript:alert(1)",
            "not a url",
        ] {
            assert!(jam_url(bad).is_err(), "{bad}");
        }
    }
}
