// What people are doing right now. Held in memory like presence: nothing here
// is worth a migration, and a restart losing "Playing Minecraft" costs one poll.
// Each person has a slot per source; every entry expires unless refreshed.
use crate::{auth::Auth, *};
use axum::extract::Path;
use std::collections::BTreeMap;

pub(crate) type Activities = std::sync::Mutex<HashMap<String, BTreeMap<String, Activity>>>;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_millis() as i64
}

/// One person's live activities, newest first.
fn current(map: &BTreeMap<String, Activity>, at: i64) -> Vec<Activity> {
    let mut list: Vec<Activity> = map
        .values()
        .filter(|a| a.expires_at > at)
        .cloned()
        .collect();
    list.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(a.slot.cmp(&b.slot)));
    list
}

pub(crate) fn snapshot(s: &AppState) -> Vec<UserActivities> {
    let at = now_ms();
    let all = s.activities.lock().expect("activities mutex");
    let mut out: Vec<UserActivities> = all
        .iter()
        .map(|(user, map)| UserActivities {
            user_id: user.clone(),
            activities: current(map, at),
        })
        .filter(|u| !u.activities.is_empty())
        .collect();
    out.sort_by(|a, b| a.user_id.cmp(&b.user_id));
    out
}

fn announce(s: &AppState, user: &str) {
    let activities = {
        let all = s.activities.lock().expect("activities mutex");
        all.get(user)
            .map(|m| current(m, now_ms()))
            .unwrap_or_default()
    };
    let _ = s.events.send(Event::ActivityUpdated {
        user_id: user.to_string(),
        activities,
    });
}

/// Store an activity, keeping its start time when the same thing is refreshed.
/// Broadcasts only when what people would see changed.
pub(crate) struct Doing {
    pub kind: ActivityKind,
    pub name: String,
    pub details: Option<String>,
    pub image_url: Option<String>,
}
pub(crate) fn put(s: &AppState, user: &str, slot: &str, doing: Doing, ttl_seconds: u32) {
    let Doing {
        kind,
        name,
        details,
        image_url,
    } = doing;
    let at = now_ms();
    let changed = {
        let mut all = s.activities.lock().expect("activities mutex");
        let slots = all.entry(user.to_string()).or_default();
        let previous = slots.get(slot).filter(|a| a.expires_at > at);
        let same = previous.is_some_and(|p| p.kind == kind && p.name == name);
        let started_at = if same {
            previous.map(|p| p.started_at).unwrap_or(at)
        } else {
            at
        };
        let changed =
            !same || previous.is_some_and(|p| p.details != details || p.image_url != image_url);
        slots.insert(
            slot.to_string(),
            Activity {
                slot: slot.to_string(),
                kind,
                name,
                details,
                image_url,
                started_at,
                expires_at: at + i64::from(ttl_seconds) * 1000,
            },
        );
        changed
    };
    if changed {
        announce(s, user);
    }
}

pub(crate) fn clear(s: &AppState, user: &str, slot: &str) {
    let removed = {
        let mut all = s.activities.lock().expect("activities mutex");
        let Some(slots) = all.get_mut(user) else {
            return;
        };
        let removed = slots.remove(slot).is_some_and(|a| a.expires_at > now_ms());
        if slots.is_empty() {
            all.remove(user);
        }
        removed
    };
    if removed {
        announce(s, user);
    }
}

/// `me` or a real user id. Setting someone else's activity is an admin's job:
/// the Minecraft relay runs with an admin token and speaks for the players.
async fn target(s: &AppState, a: &Auth, id: &str) -> Result<String> {
    if id == "me" || id == a.user.id {
        return Ok(a.user.id.clone());
    }
    a.admin()?;
    sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await
        .map_err(|_| Error::missing())?
        .ok_or_else(Error::missing)
}

fn writable(slot: &str) -> Result<()> {
    if !valid_activity_slot(slot) {
        return Err(Error::bad(
            "Slot must be 1-32 lowercase letters, digits or hyphens.",
        ));
    }
    if ACTIVITY_SERVER_SLOTS.contains(&slot) {
        return Err(Error::bad("That slot is filled by the server."));
    }
    Ok(())
}

#[utoipa::path(put,path="/users/{id}/activities/{slot}",params(("id"=String,Path,description="A user id, or `me`"),("slot"=String,Path,description="The source setting it, e.g. `desktop`, `cli`, `minecraft`")),request_body=SetActivity,responses((status=200,body=UserActivities),(status=400,body=ApiError),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn set(
    State(s): State<AppState>,
    a: Auth,
    Path((id, slot)): Path<(String, String)>,
    ApiJson(mut body): ApiJson<SetActivity>,
) -> Result<Json<UserActivities>> {
    writable(&slot)?;
    body.validate().map_err(Error::bad)?;
    let user = target(&s, &a, &id).await?;
    let doing = Doing {
        kind: body.kind,
        name: body.name,
        details: body.details,
        image_url: None,
    };
    put(
        &s,
        &user,
        &slot,
        doing,
        body.ttl_seconds.unwrap_or(ACTIVITY_DEFAULT_TTL),
    );
    Ok(Json(of(&s, &user)))
}

#[utoipa::path(delete,path="/users/{id}/activities/{slot}",params(("id"=String,Path,description="A user id, or `me`"),("slot"=String,Path)),responses((status=200,body=UserActivities),(status=403,body=ApiError),(status=404,body=ApiError)))]
pub(crate) async fn remove(
    State(s): State<AppState>,
    a: Auth,
    Path((id, slot)): Path<(String, String)>,
) -> Result<Json<UserActivities>> {
    writable(&slot)?;
    let user = target(&s, &a, &id).await?;
    clear(&s, &user, &slot);
    Ok(Json(of(&s, &user)))
}

#[utoipa::path(get,path="/activities",responses((status=200,body=[UserActivities])))]
pub(crate) async fn list(State(s): State<AppState>, _a: Auth) -> Json<Vec<UserActivities>> {
    Json(snapshot(&s))
}

fn of(s: &AppState, user: &str) -> UserActivities {
    let all = s.activities.lock().expect("activities mutex");
    UserActivities {
        user_id: user.to_string(),
        activities: all
            .get(user)
            .map(|m| current(m, now_ms()))
            .unwrap_or_default(),
    }
}

impl AppState {
    /// Drops expired activities and tells everyone, so a source that stopped
    /// refreshing disappears within a few seconds of its deadline.
    #[doc(hidden)]
    pub fn expire_activities(&self) {
        self.expire_activities_at(now_ms())
    }
    /// The same, at a chosen instant, so tests need not sleep past a deadline.
    #[doc(hidden)]
    pub fn expire_activities_at(&self, at: i64) {
        let expired: Vec<String> = {
            let mut all = self.activities.lock().expect("activities mutex");
            let mut users = Vec::new();
            all.retain(|user, slots| {
                let before = slots.len();
                slots.retain(|_, a| a.expires_at > at);
                if slots.len() != before {
                    users.push(user.clone());
                }
                !slots.is_empty()
            });
            users
        };
        for user in expired {
            announce(self, &user);
        }
    }

    /// "Listening to …" for everyone online with Spotify connected. Only online
    /// people are polled, so an idle account costs nothing.
    #[doc(hidden)]
    pub async fn poll_spotify_activities(&self) {
        let online: Vec<String> = self
            .presence
            .lock()
            .expect("presence mutex")
            .keys()
            .cloned()
            .collect();
        if online.is_empty() {
            return;
        }
        let connected = sqlx::query_scalar::<_, String>(
            "SELECT user_id FROM spotify_accounts WHERE needs_reauth=0",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();
        for user in connected.iter().filter(|u| online.contains(u)) {
            match spotify::now_playing(self, user).await {
                Some(p) if p.is_playing => {
                    let doing = Doing {
                        kind: ActivityKind::Listening,
                        name: p.track.chars().take(64).collect(),
                        details: Some(p.artists.chars().take(128).collect()),
                        image_url: p.album_art,
                    };
                    put(self, user, "spotify", doing, 60)
                }
                _ => clear(self, user, "spotify"),
            }
        }
    }

    pub fn run_activities(&self) {
        let weak = Arc::downgrade(&self.0);
        tokio::spawn(async move {
            let mut expiry = tokio::time::interval(Duration::from_secs(5));
            let mut tick = 0u32;
            loop {
                expiry.tick().await;
                let Some(inner) = weak.upgrade() else { break };
                let state = AppState(inner);
                state.expire_activities();
                // Spotify every 20 seconds: fresh enough to follow a playlist,
                // far below any rate limit for a handful of people.
                if tick.is_multiple_of(4) {
                    state.poll_spotify_activities().await;
                }
                tick = tick.wrapping_add(1);
            }
        });
    }
}
