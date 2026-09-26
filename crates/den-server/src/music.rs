use crate::{auth::Auth, chat::visible, *};
use axum::extract::Path;
use sqlx::Row;
use std::{collections::HashSet, sync::Weak};
use tokio::sync::watch;

#[path = "music_player.rs"]
mod player;

/// One cancellable worker per active voice room. Dropping AppState closes workers.
pub(crate) struct Music {
    tools: player::Tools,
    resolver_slots: Arc<tokio::sync::Semaphore>,
    runners: std::sync::Mutex<HashMap<String, watch::Sender<u64>>>,
}
impl Default for Music {
    fn default() -> Self {
        Self {
            tools: player::Tools::default(),
            runners: std::sync::Mutex::new(HashMap::new()),
            resolver_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}
impl Music {
    pub(crate) fn set_tools(&mut self, resolver: String, ffmpeg: String, publisher: String) {
        self.tools = player::Tools {
            resolver,
            ffmpeg,
            publisher,
        };
    }
    fn wake(&self, s: &AppState, room: &str) {
        let mut runners = self.runners.lock().expect("music runners");
        if let Some(tx) = runners.get(room).filter(|tx| !tx.is_closed()) {
            tx.send_modify(|n| *n += 1);
            return;
        }
        let (tx, rx) = watch::channel(0);
        runners.insert(room.into(), tx);
        tokio::spawn(player::run(Arc::downgrade(&s.0), room.into(), rx));
    }
}
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_millis() as i64
}
struct Queue {
    view: MusicQueue,
    epoch: i64,
    jam_paused: bool,
}
fn active(q: &MusicQueue) -> Option<&MusicTrack> {
    q.queue.iter().find(|t| {
        matches!(
            t.state,
            MusicTrackState::Loading | MusicTrackState::Playing | MusicTrackState::Paused
        )
    })
}
fn active_mut(q: &mut MusicQueue) -> Option<&mut MusicTrack> {
    q.queue.iter_mut().find(|t| {
        matches!(
            t.state,
            MusicTrackState::Loading | MusicTrackState::Playing | MusicTrackState::Paused
        )
    })
}
async fn authorize(s: &AppState, a: &Auth, room: &str) -> Result<()> {
    if visible(s, &a.user.id, room).await?.kind != ChannelKind::Voice {
        return Err(Error::bad("Music belongs in a voice room."));
    }
    Ok(())
}
async fn load(s: &AppState, room: &str) -> Result<Queue> {
    let r = sqlx::query(
        "SELECT paused,jam_paused,position_seconds,updated_at,revision,epoch FROM music_rooms WHERE room_id=?",
    )
    .bind(room)
    .fetch_optional(&s.db)
    .await?;
    let mut view = MusicQueue {
        room_id: room.into(),
        participant_id: format!("den-dj-{room}"),
        queue: vec![],
        paused: false,
        paused_for_jam: false,
        position_seconds: 0.,
        updated_at: now_ms(),
        revision: 0,
    };
    let mut epoch = 0;
    let mut jam_paused = false;
    if let Some(r) = r {
        view.paused = r.get("paused");
        jam_paused = r.get("jam_paused");
        view.paused_for_jam = jam_paused;
        view.position_seconds = r.get("position_seconds");
        view.updated_at = r.get("updated_at");
        view.revision = r.get("revision");
        epoch = r.get("epoch");
    }
    for r in sqlx::query("SELECT * FROM music_queue WHERE room_id=? ORDER BY position,id")
        .bind(room)
        .fetch_all(&s.db)
        .await?
    {
        let state: String = r.get("state");
        let state = serde_json::from_value(serde_json::Value::String(state))
            .map_err(|_| Error::bad("Invalid queue state"))?;
        view.queue.push(MusicTrack {
            id: r.get("id"),
            room_id: room.into(),
            url: r.get("url"),
            title: r.get("title"),
            duration: r.get("duration"),
            thumbnail: r.get("thumbnail"),
            added_by: r.get("added_by"),
            position: r.get("position"),
            state,
        });
    }
    if active(&view).is_some_and(|t| t.state == MusicTrackState::Playing) && !view.paused {
        view.position_seconds += ((now_ms() - view.updated_at).max(0) as f64) / 1000.;
        if let Some(duration) = active(&view).and_then(|t| t.duration) {
            view.position_seconds = view.position_seconds.min(duration);
        }
    }
    view.updated_at = now_ms();
    Ok(Queue {
        view,
        epoch,
        jam_paused,
    })
}
// Called with the shared write lock. Queue edits and their complete WS snapshot
// are committed together; clients discard snapshots older than their revision.
async fn save(s: &AppState, q: &mut Queue) -> Result<MusicQueue> {
    q.view.revision += 1;
    q.view.updated_at = now_ms();
    let mut tx = s.db.begin().await?;
    sqlx::query("INSERT INTO music_rooms(room_id,paused,jam_paused,position_seconds,updated_at,revision,epoch) VALUES(?,?,?,?,?,?,?) ON CONFLICT(room_id) DO UPDATE SET paused=excluded.paused,jam_paused=excluded.jam_paused,position_seconds=excluded.position_seconds,updated_at=excluded.updated_at,revision=excluded.revision,epoch=excluded.epoch")
        .bind(&q.view.room_id).bind(q.view.paused).bind(q.jam_paused).bind(q.view.position_seconds).bind(q.view.updated_at).bind(q.view.revision).bind(q.epoch).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM music_queue WHERE room_id=?")
        .bind(&q.view.room_id)
        .execute(&mut *tx)
        .await?;
    for (i, t) in q.view.queue.iter_mut().enumerate() {
        t.position = i as i64;
        let state = serde_json::to_value(&t.state).expect("track state");
        sqlx::query("INSERT INTO music_queue(id,room_id,url,title,duration,thumbnail,added_by,position,state) VALUES(?,?,?,?,?,?,?,?,?)")
            .bind(&t.id).bind(&t.room_id).bind(&t.url).bind(&t.title).bind(t.duration).bind(&t.thumbnail).bind(&t.added_by).bind(t.position).bind(state.as_str().expect("state string")).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    let _ = s.events.send(Event::MusicQueueUpdated {
        queue: q.view.clone(),
    });
    Ok(q.view.clone())
}
fn youtube_url(input: &str) -> Result<String> {
    let u =
        reqwest::Url::parse(input.trim()).map_err(|_| Error::bad("Paste a YouTube video URL."))?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
    {
        return Err(Error::bad("Paste an HTTPS YouTube video URL."));
    }
    let id = match u.host_str().unwrap_or("") {
        "youtu.be" => u.path().trim_start_matches('/').to_owned(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" => {
            if u.path() == "/watch" {
                u.query_pairs()
                    .find(|(k, _)| k == "v")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default()
            } else {
                u.path()
                    .strip_prefix("/shorts/")
                    .or_else(|| u.path().strip_prefix("/live/"))
                    .unwrap_or("")
                    .to_owned()
            }
        }
        _ => String::new(),
    };
    if id.len() != 11
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(Error::bad("Paste a YouTube video URL, not a playlist."));
    }
    Ok(format!("https://www.youtube.com/watch?v={id}"))
}
#[utoipa::path(get,path="/rooms/{id}/music",params(("id"=String,Path)),responses((status=200,body=MusicQueue)))]
pub(crate) async fn get(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    Ok(Json(load(&s, &room).await?.view))
}
#[utoipa::path(post,path="/rooms/{id}/music/queue",params(("id"=String,Path)),request_body=AddMusic,responses((status=200,body=MusicQueue)))]
pub(crate) async fn add(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
    ApiJson(v): ApiJson<AddMusic>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let url = youtube_url(&v.url)?;
    let _guard = s.writes.lock().await;
    let mut q = load(&s, &room).await?;
    if q.view.queue.len() >= 100 {
        return Err(Error::bad("The queue is full. Remove a track first."));
    }
    let id = s.id();
    q.view.queue.push(MusicTrack {
        id: id.clone(),
        room_id: room.clone(),
        url: url.clone(),
        title: "Loading track…".into(),
        duration: None,
        thumbnail: None,
        added_by: a.user.id,
        position: 0,
        state: MusicTrackState::Queued,
    });
    let view = save(&s, &mut q).await?;
    tokio::spawn(player::prepare(Arc::downgrade(&s.0), room.clone(), id, url));
    s.music.wake(&s, &room);
    Ok(Json(view))
}
#[utoipa::path(delete,path="/rooms/{id}/music/queue/{track_id}",params(("id"=String,Path),("track_id"=String,Path)),responses((status=200,body=MusicQueue)))]
pub(crate) async fn remove(
    State(s): State<AppState>,
    a: Auth,
    Path((room, id)): Path<(String, String)>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let mut q = load(&s, &room).await?;
    if !q.view.queue.iter().any(|t| t.id == id) {
        return Err(Error::missing());
    }
    if active(&q.view).is_some_and(|t| t.id == id) {
        q.epoch += 1;
        q.view.position_seconds = 0.;
    }
    q.view.queue.retain(|t| t.id != id);
    let view = save(&s, &mut q).await?;
    s.music.wake(&s, &room);
    Ok(Json(view))
}
#[utoipa::path(post,path="/rooms/{id}/music/skip",params(("id"=String,Path)),responses((status=200,body=MusicQueue)))]
pub(crate) async fn skip(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let mut q = load(&s, &room).await?;
    let id = active(&q.view)
        .or_else(|| {
            q.view
                .queue
                .iter()
                .find(|t| t.state == MusicTrackState::Queued)
        })
        .map(|t| t.id.clone());
    if let Some(id) = id {
        q.view.queue.retain(|t| t.id != id);
        q.epoch += 1;
        q.view.position_seconds = 0.;
    }
    let view = save(&s, &mut q).await?;
    s.music.wake(&s, &room);
    Ok(Json(view))
}
#[utoipa::path(post,path="/rooms/{id}/music/pause",params(("id"=String,Path)),request_body=PauseMusic,responses((status=200,body=MusicQueue)))]
pub(crate) async fn pause(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
    ApiJson(v): ApiJson<PauseMusic>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let mut q = load(&s, &room).await?;
    // A person's own Play or Pause outranks the Jam: ending the Jam must not undo it.
    q.jam_paused = false;
    q.view.paused_for_jam = false;
    if q.view.paused != v.paused {
        q.epoch += 1;
        q.view.paused = v.paused;
        if let Some(t) = active_mut(&mut q.view) {
            t.state = if v.paused {
                MusicTrackState::Paused
            } else {
                MusicTrackState::Loading
            };
        }
    }
    let view = save(&s, &mut q).await?;
    s.music.wake(&s, &room);
    Ok(Json(view))
}
#[utoipa::path(post,path="/rooms/{id}/music/seek",params(("id"=String,Path)),request_body=SeekMusic,responses((status=200,body=MusicQueue)))]
pub(crate) async fn seek(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
    ApiJson(v): ApiJson<SeekMusic>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let mut q = load(&s, &room).await?;
    let duration = active(&q.view)
        .and_then(|t| t.duration)
        .ok_or_else(|| Error::bad("Wait for the track to load before seeking."))?;
    if !v.position_seconds.is_finite() || v.position_seconds < 0. || v.position_seconds >= duration
    {
        return Err(Error::bad("Choose a position within this track."));
    }
    q.view.position_seconds = v.position_seconds;
    q.epoch += 1;
    if !q.view.paused {
        if let Some(t) = active_mut(&mut q.view) {
            t.state = MusicTrackState::Loading;
        }
    }
    let view = save(&s, &mut q).await?;
    s.music.wake(&s, &room);
    Ok(Json(view))
}
#[utoipa::path(put,path="/rooms/{id}/music/queue/order",params(("id"=String,Path)),request_body=OrderMusic,responses((status=200,body=MusicQueue)))]
pub(crate) async fn order(
    State(s): State<AppState>,
    a: Auth,
    Path(room): Path<String>,
    ApiJson(v): ApiJson<OrderMusic>,
) -> Result<Json<MusicQueue>> {
    authorize(&s, &a, &room).await?;
    let _guard = s.writes.lock().await;
    let mut q = load(&s, &room).await?;
    let current = active(&q.view).map(|t| t.id.clone());
    let ids: HashSet<_> = q
        .view
        .queue
        .iter()
        .filter(|t| Some(&t.id) != current.as_ref())
        .map(|t| t.id.clone())
        .collect();
    if v.ids.len() != ids.len()
        || v.ids.iter().collect::<HashSet<_>>().len() != ids.len()
        || !v.ids.iter().all(|id| ids.contains(id))
    {
        return Err(Error::bad("The queue changed. Refresh it and try again."));
    }
    q.view.queue.sort_by_key(|t| {
        if Some(&t.id) == current.as_ref() {
            0
        } else {
            v.ids.iter().position(|id| id == &t.id).unwrap() + 1
        }
    });
    let view = save(&s, &mut q).await?;
    s.music.wake(&s, &room);
    Ok(Json(view))
}

/// Pause the queue because a Jam started. No-op if already paused. Marks it, so
/// the queue resumes when the Jam ends. The caller holds the write lock.
pub(crate) async fn jam_pause(s: &AppState, room: &str) -> Result<()> {
    let mut q = load(s, room).await?;
    if q.view.paused {
        return Ok(());
    }
    q.epoch += 1;
    q.view.paused = true;
    q.jam_paused = true;
    q.view.paused_for_jam = true;
    if let Some(t) = active_mut(&mut q.view) {
        t.state = MusicTrackState::Paused;
    }
    save(s, &mut q).await?;
    s.music.wake(s, room);
    Ok(())
}

/// Resume the queue after a Jam ends, only if the Jam paused it. The caller
/// holds the write lock.
pub(crate) async fn jam_resume(s: &AppState, room: &str) -> Result<()> {
    let mut q = load(s, room).await?;
    if !q.jam_paused {
        return Ok(());
    }
    q.epoch += 1;
    q.view.paused = false;
    q.jam_paused = false;
    q.view.paused_for_jam = false;
    if let Some(t) = active_mut(&mut q.view) {
        t.state = MusicTrackState::Loading;
    }
    save(s, &mut q).await?;
    s.music.wake(s, room);
    Ok(())
}
