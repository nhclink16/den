use super::*;

async fn voice_room(t: &Test) -> String {
    let v: Vec<Channel> = t
        .req(Method::GET, "/channels", &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    v.into_iter()
        .find(|c| c.kind == ChannelKind::Voice)
        .unwrap()
        .id
}

async fn get_jam(t: &Test, room: &str, token: &str) -> Option<Jam> {
    t.req(Method::GET, &format!("/rooms/{room}/jam"), token)
        .send()
        .await
        .unwrap()
        .json::<RoomJam>()
        .await
        .unwrap()
        .jam
}

async fn music_queue(t: &Test, room: &str) -> MusicQueue {
    t.req(Method::GET, &format!("/rooms/{room}/music"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn place_in_call(t: &Test, channel: &str, user_id: &str, key: &str) {
    t.state
        .calls
        .lock()
        .await
        .entry(channel.to_string())
        .or_default()
        .insert(format!("{user_id}:{key}"), user_id.to_string());
}

async fn remove_from_call(t: &Test, channel: &str) {
    t.state.calls.lock().await.remove(channel);
}

#[tokio::test]
async fn denied_start_from_text_room() {
    let t = Test::new().await;
    let member = t.member("jam_textroom").await;
    let text = t.general().await;
    let r = t
        .req(Method::POST, &format!("/rooms/{text}/jam"), &member.token)
        .json(&json!({"url":"https://spotify.link/abc"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::BAD_REQUEST,
        "text room rejects Jams"
    );
    // Reading still answers "no Jam": every client asks about every room on resync.
    assert!(get_jam(&t, &text, &member.token).await.is_none());
}

#[tokio::test]
async fn denied_start_when_not_in_call() {
    let t = Test::new().await;
    let member = t.member("jam_notincall").await;
    let room = voice_room(&t).await;
    let r = t
        .req(Method::POST, &format!("/rooms/{room}/jam"), &member.token)
        .json(&json!({"url":"https://spotify.link/abc"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::BAD_REQUEST,
        "must be in call to start Jam"
    );
}

#[tokio::test]
async fn end_permissions_host_and_admin_only() {
    let t = Test::new().await;
    let host = t.member("jam_end_host").await;
    let other = t.member("jam_end_other").await;
    let room = voice_room(&t).await;

    place_in_call(&t, &room, &host.user.id, "h").await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &host.token,
        json!({"url":"https://spotify.link/endtest"}),
    )
    .await;

    // Random member (not the host, not admin) cannot end.
    assert_eq!(
        t.req(Method::DELETE, &format!("/rooms/{room}/jam"), &other.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    // Host can end.
    assert_eq!(
        t.req(Method::DELETE, &format!("/rooms/{room}/jam"), &host.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(get_jam(&t, &room, &host.token).await.is_none());
}

#[tokio::test]
async fn admin_can_end_any_jam() {
    let t = Test::new().await;
    let host = t.member("jam_adminend_host").await;
    let room = voice_room(&t).await;

    place_in_call(&t, &room, &host.user.id, "h").await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &host.token,
        json!({"url":"https://spotify.link/adminend"}),
    )
    .await;

    // Admin (not the host) can end.
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/rooms/{room}/jam"),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::NO_CONTENT
    );
    assert!(get_jam(&t, &room, &t.admin.token).await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn music_pauses_on_jam_start_and_resumes_on_jam_end() {
    use std::os::unix::fs::PermissionsExt;
    // Resolver stub that returns a valid track.
    let path = std::env::temp_dir().join(format!("jam-resolver-{}", ulid::Ulid::new()));
    std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' '{\"url\":\"https://audio.googlevideo.com/test\",\"title\":\"Test\",\"duration\":120,\"thumbnail\":null}'\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());

    let t = Test::configured(None, Some(path.to_string_lossy().into())).await;
    let host = t.member("jam_music_host").await;
    let room = voice_room(&t).await;
    let music_path = format!("/rooms/{room}/music");
    let jam_path = format!("/rooms/{room}/jam");

    // Add a track to the queue.
    t.req(Method::POST, &format!("{music_path}/queue"), &host.token)
        .json(&json!({"url":"https://www.youtube.com/watch?v=aaaaaaaaaaa"}))
        .send()
        .await
        .unwrap();

    // Queue is not paused initially.
    let q = music_queue(&t, &room).await;
    assert!(!q.paused, "queue starts unpaused");
    assert!(!q.paused_for_jam, "not paused for jam");

    // Start a Jam — the queue should pause.
    place_in_call(&t, &room, &host.user.id, "h").await;
    t.post(
        &jam_path,
        &host.token,
        json!({"url":"https://spotify.link/musictest"}),
    )
    .await;

    let q = music_queue(&t, &room).await;
    assert!(q.paused, "queue paused when Jam started");
    assert!(q.paused_for_jam, "pause attributed to Jam");

    // End the Jam — queue should resume.
    t.req(Method::DELETE, &jam_path, &host.token)
        .send()
        .await
        .unwrap();

    let q = music_queue(&t, &room).await;
    assert!(!q.paused, "queue resumed after Jam ended");
    assert!(!q.paused_for_jam, "jam_paused cleared");
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

async fn start(t: &Test, room: &str, host: &Session, link: &str) {
    place_in_call(t, room, &host.user.id, "h").await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &host.token,
        json!({ "url": link }),
    )
    .await;
}

#[tokio::test]
async fn an_empty_call_ends_the_jam_after_ten_observed_minutes() {
    let t = Test::new().await;
    let host = t.member("jam_autoend_empty").await;
    let room = voice_room(&t).await;
    start(&t, &room, &host, "https://spotify.link/autoend").await;
    let at = unix_now();

    // Occupied: nothing happens, however long it runs.
    t.state.jam_watcher_tick(at + 3600).await.unwrap();
    assert!(get_jam(&t, &room, &host.token).await.is_some());

    // Emptied: the first empty pass starts the timer, it does not end anything.
    remove_from_call(&t, &room).await;
    t.state.jam_watcher_tick(at).await.unwrap();
    t.state.jam_watcher_tick(at + 599).await.unwrap();
    assert!(
        get_jam(&t, &room, &host.token).await.is_some(),
        "9m59s is not ten minutes"
    );

    // Someone comes back briefly: the timer resets.
    place_in_call(&t, &room, &host.user.id, "h").await;
    t.state.jam_watcher_tick(at + 600).await.unwrap();
    // Gone again, but the room's DJ is still connected. The DJ is not a person.
    remove_from_call(&t, &room).await;
    t.state
        .calls
        .lock()
        .await
        .entry(room.clone())
        .or_default()
        .insert(format!("den-dj-{room}"), "PA_dj".into());
    t.state.jam_watcher_tick(at + 660).await.unwrap();
    t.state.jam_watcher_tick(at + 1200).await.unwrap();
    assert!(
        get_jam(&t, &room, &host.token).await.is_some(),
        "timer restarted at +660"
    );

    t.state.jam_watcher_tick(at + 1260).await.unwrap();
    assert!(get_jam(&t, &room, &host.token).await.is_none());
}

#[tokio::test]
async fn a_host_without_spotify_is_never_ended_for_silence() {
    let t = Test::with_spotify().await;
    let host = t.member("jam_idle_nospotify").await;
    let room = voice_room(&t).await;
    start(&t, &room, &host, "https://spotify.link/nospotify").await;
    t.state
        .jam_watcher_tick(unix_now() + 6 * 3600)
        .await
        .unwrap();
    assert!(
        get_jam(&t, &room, &host.token).await.is_some(),
        "we cannot see their playback, so silence proves nothing"
    );
}

#[tokio::test]
async fn migration_ends_jams_already_pinned_in_text_rooms() {
    use sqlx::{migrate::Migrator, Connection};
    let dir = std::env::temp_dir().join(format!("den-migrate-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.join("den.db"))
            .create_if_missing(true),
    )
    .await
    .unwrap();
    // Same as AppState::open: table rebuilds must not cascade into children.
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut db)
        .await
        .unwrap();
    // Everything up to 0023, then a text-room Jam and a voice-room Jam as an
    // older server would have left them, then 0024.
    let all = Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await
    .unwrap();
    let mut before = Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await
    .unwrap();
    before.migrations = all
        .migrations
        .iter()
        .filter(|m| m.version < 24)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    before.run(&mut db).await.unwrap();
    for q in [
        "INSERT INTO users(id,username,display_name,role) VALUES('u','u','u','admin')",
        "INSERT INTO channels(id,name,kind,position) VALUES('t','old-text','text',0)",
        "INSERT INTO channels(id,name,kind,position) VALUES('v','old-voice','voice',0)",
        "INSERT INTO jams(id,channel_id,url,host_id,started_at) VALUES('jt','t','https://spotify.link/a','u',1)",
        "INSERT INTO jams(id,channel_id,url,host_id,started_at) VALUES('jv','v','https://spotify.link/b','u',1)",
    ] {
        sqlx::query(q).execute(&mut db).await.unwrap();
    }
    all.run(&mut db).await.unwrap();
    let live: Vec<String> = sqlx::query_scalar("SELECT id FROM jams WHERE ended_at IS NULL")
        .fetch_all(&mut db)
        .await
        .unwrap();
    assert_eq!(live, vec!["jv".to_string()]);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn only_the_host_or_an_admin_can_replace_a_live_jam() {
    let t = Test::new().await;
    let host = t.member("jam_replace_host").await;
    let other = t.member("jam_replace_other").await;
    let room = voice_room(&t).await;
    start(&t, &room, &host, "https://spotify.link/original").await;
    let original = get_jam(&t, &room, &host.token).await.unwrap().id;
    place_in_call(&t, &room, &other.user.id, "o").await;

    // Replacing ends the current Jam, so it needs the same right as ending it.
    let replaced = t
        .req(Method::POST, &format!("/rooms/{room}/jam"), &other.token)
        .json(&json!({"url":"https://spotify.link/takeover"}))
        .send()
        .await
        .unwrap();
    assert_eq!(replaced.status(), StatusCode::FORBIDDEN);
    assert_eq!(get_jam(&t, &room, &host.token).await.unwrap().id, original);

    place_in_call(&t, &room, &t.admin.user.id, "a").await;
    t.post(
        &format!("/rooms/{room}/jam"),
        &t.admin.token,
        json!({"url":"https://spotify.link/admin"}),
    )
    .await;
    assert_ne!(get_jam(&t, &room, &host.token).await.unwrap().id, original);
}

#[tokio::test]
async fn an_unverifiable_call_is_not_treated_as_empty() {
    // LiveKit is configured but unreachable, as it can be just after a restart.
    let t = Test::with_voice(true).await;
    let host = t.member("jam_unverified").await;
    let room = voice_room(&t).await;
    start(&t, &room, &host, "https://spotify.link/unverified").await;
    // The restart lost the webhook-fed map; LiveKit cannot be asked.
    remove_from_call(&t, &room).await;
    let at = unix_now();
    t.state.jam_watcher_tick(at).await.unwrap();
    t.state.jam_watcher_tick(at + 3600).await.unwrap();
    assert!(
        get_jam(&t, &room, &host.token).await.is_some(),
        "occupancy was never verified, so nothing may expire"
    );
}
