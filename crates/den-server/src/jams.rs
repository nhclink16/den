//! Jam cards. A Jam is a share link pinned to a room, plus the Den members who
//! pressed Join. Spotify cannot tell us who is listening to a Jam, so this
//! counts clicks and says so; `now_playing` is the host's own playback and
//! nothing else.
use crate::{auth::Auth, chat::visible, *};
use axum::extract::Path;
use sqlx::Row;

async fn authorize(s: &AppState, a: &Auth, room: &str) -> Result<()> {
    visible(s, &a.user.id, room).await?;
    Ok(())
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
    authorize(&s, &a, &room).await?;
    let url = jam_url(&v.url)?;
    let _guard = s.writes.lock().await;
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
    if jam.host_id != a.user.id {
        a.admin()?;
    }
    broadcast(&s, &room, Some(jam.clone())).await;
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
    sqlx::query("UPDATE jams SET ended_at=? WHERE id=?")
        .bind(now())
        .bind(&jam.id)
        .execute(&s.db)
        .await?;
    broadcast(&s, &room, None).await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
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
