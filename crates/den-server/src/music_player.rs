use super::*;
use livekit_api::access_token::{AccessToken, VideoGrants};
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::{Child, Command},
};

#[derive(Clone)]
pub(crate) struct Tools {
    pub resolver: String,
    pub ffmpeg: String,
    pub publisher: String,
}
impl Default for Tools {
    fn default() -> Self {
        Self {
            resolver: std::env::var("DEN_YTDLP").unwrap_or_else(|_| "yt-dlp".into()),
            ffmpeg: std::env::var("DEN_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
            publisher: std::env::var("DEN_DJ_BIN").unwrap_or_else(|_| "den-dj".into()),
        }
    }
}
struct Source {
    url: String,
    title: String,
    duration: f64,
    thumbnail: Option<String>,
}
async fn resolve(tools: &Tools, url: &str) -> anyhow::Result<Source> {
    tokio::time::timeout(Duration::from_secs(40), async {
        let mut child = Command::new(&tools.resolver)
            .args([
                "--ignore-config",
                "--no-cache-dir",
                "--no-playlist",
                "--no-warnings",
                "--socket-timeout",
                "10",
                "--retries",
                "1",
                "--extractor-retries",
                "1",
                "--js-runtimes",
                "node",
                "--dump-single-json",
                "-f",
                "bestaudio[protocol=https]",
                "--",
                url,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut bytes = vec![];
        child
            .stdout
            .take()
            .expect("resolver stdout")
            .take(2 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .await?;
        anyhow::ensure!(bytes.len() < 2 * 1024 * 1024, "metadata too large");
        anyhow::ensure!(child.wait().await?.success(), "resolution failed");
        let v: serde_json::Value = serde_json::from_slice(&bytes)?;
        let stream = reqwest::Url::parse(v["url"].as_str().unwrap_or(""))?;
        anyhow::ensure!(
            stream.scheme() == "https"
                && stream
                    .host_str()
                    .is_some_and(|h| h.ends_with(".googlevideo.com"))
                && stream.port().is_none()
                && stream.username().is_empty()
                && stream.password().is_none(),
            "unexpected media origin"
        );
        let duration = v["duration"].as_f64().unwrap_or(0.);
        anyhow::ensure!(
            duration.is_finite() && duration > 0. && duration <= 21600. && v["is_live"] != true,
            "unsupported duration"
        );
        let title = v["title"]
            .as_str()
            .unwrap_or("YouTube track")
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        // Render only YouTube's image CDN, never arbitrary metadata URLs.
        let thumbnail = v["thumbnail"]
            .as_str()
            .and_then(|u| reqwest::Url::parse(u).ok())
            .filter(|u| {
                u.scheme() == "https"
                    && u.host_str()
                        .is_some_and(|h| h == "i.ytimg.com" || h == "img.youtube.com")
            })
            .map(|u| u.to_string());
        Ok(Source {
            url: stream.into(),
            title,
            duration,
            thumbnail,
        })
    })
    .await?
}
pub(super) async fn prepare(weak: Weak<Inner>, room: String, id: String, url: String) {
    let Some(s) = weak.upgrade().map(AppState) else {
        return;
    };
    let tools = s.music.tools.clone();
    let slots = s.music.resolver_slots.clone();
    drop(s);
    let Ok(_permit) = slots.acquire_owned().await else {
        return;
    };
    if weak.upgrade().is_none() {
        return;
    }
    let result = resolve(&tools, &url).await;
    let Some(s) = weak.upgrade().map(AppState) else {
        return;
    };
    let _guard = s.writes.lock().await;
    let Ok(mut q) = load(&s, &room).await else {
        return;
    };
    let Some(t) = q
        .view
        .queue
        .iter_mut()
        .find(|t| t.id == id && t.state == MusicTrackState::Queued)
    else {
        return;
    };
    match result {
        Ok(source) => {
            t.title = source.title;
            t.duration = Some(source.duration);
            t.thumbnail = source.thumbnail;
        }
        Err(_) => {
            t.state = MusicTrackState::Failed;
            t.title = "Could not load this track".into();
        }
    }
    if save(&s, &mut q).await.is_ok() {
        s.music.wake(&s, &room);
    }
}
struct Pipeline {
    ffmpeg: Child,
    dj: Child,
    copy: tokio::task::JoinHandle<std::io::Result<u64>>,
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        self.copy.abort();
        let _ = self.ffmpeg.start_kill();
        let _ = self.dj.start_kill();
    }
}
impl Pipeline {
    async fn start(
        tools: &Tools,
        source: &Source,
        offset: f64,
        url: &str,
        token: &str,
    ) -> anyhow::Result<Self> {
        let mut ffmpeg = Command::new(&tools.ffmpeg)
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-rw_timeout",
                "15000000",
                "-ss",
                &offset.to_string(),
                "-i",
                &source.url,
                "-vn",
                "-ac",
                "2",
                "-ar",
                "48000",
                "-c:a",
                "libopus",
                "-b:a",
                "128k",
                "-vbr",
                "on",
                "-application",
                "audio",
                "-frame_duration",
                "20",
                "-page_duration",
                "20000",
                "-f",
                "opus",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut dj = Command::new(&tools.publisher)
            .env("DEN_DJ_URL", url)
            .env("DEN_DJ_TOKEN", token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut input = ffmpeg.stdout.take().expect("ffmpeg stdout");
        let mut output = dj.stdin.take().expect("dj stdin");
        let copy = tokio::spawn(async move { tokio::io::copy(&mut input, &mut output).await });
        let mut pipe = Self { ffmpeg, dj, copy };
        let mut ready = String::new();
        tokio::time::timeout(
            Duration::from_secs(30),
            BufReader::new(pipe.dj.stdout.take().expect("dj stdout")).read_line(&mut ready),
        )
        .await??;
        anyhow::ensure!(ready.trim() == "READY", "publisher did not start");
        Ok(pipe)
    }
    // Both children and the transfer between them own the outcome together.
    // Waiting for the publisher first hid a decoder that died early: den-dj
    // outlives a stream that never carried an Opus header, so the track stayed
    // Playing forever instead of failing over to the next one.
    async fn finished(&mut self) -> anyhow::Result<()> {
        let Self { ffmpeg, dj, copy } = self;
        tokio::try_join!(
            async {
                anyhow::ensure!(ffmpeg.wait().await?.success(), "decoder failed");
                Ok(())
            },
            async {
                anyhow::ensure!(dj.wait().await?.success(), "publisher failed");
                Ok(())
            },
            async {
                // A publisher given nothing to publish never finishes writing.
                anyhow::ensure!(copy.await?? > 0, "no audio reached the publisher");
                Ok(())
            }
        )?;
        Ok(())
    }
}
// Workers hold only a Weak<AppState> while waiting or playing. App shutdown closes
// the watch channel and drops every subprocess, including a resolver in flight.
pub(super) async fn run(weak: Weak<Inner>, room: String, mut changed: watch::Receiver<u64>) {
    loop {
        let step = async {
            let s = weak.upgrade().map(AppState)?;
            let _guard = s.writes.lock().await;
            let mut q = load(&s, &room).await.ok()?;
            if q.view.paused {
                return None;
            }
            if active(&q.view).is_none() {
                let t = q
                    .view
                    .queue
                    .iter_mut()
                    .find(|t| t.state == MusicTrackState::Queued)?;
                t.state = MusicTrackState::Loading;
                q.view.position_seconds = 0.;
                q.epoch += 1;
                save(&s, &mut q).await.ok()?;
            }
            let t = active(&q.view)?.clone();
            let lk = s.livekit.as_ref();
            let name = objects::settings(State(s.clone()))
                .await
                .ok()?
                .0
                .instance_name;
            let token = lk.and_then(|lk| {
                AccessToken::with_api_key(&lk.key, &lk.secret)
                    .with_identity(&q.view.participant_id)
                    .with_name(&format!("{name} DJ"))
                    .with_ttl(Duration::from_secs(21660))
                    .with_grants(VideoGrants {
                        room_join: true,
                        room: room.clone(),
                        can_subscribe: false,
                        can_publish: true,
                        can_publish_data: false,
                        ..Default::default()
                    })
                    .to_jwt()
                    .ok()
            });
            Some((
                t,
                q.epoch,
                q.view.position_seconds,
                s.music.tools.clone(),
                lk.map(|v| v.url.clone()).unwrap_or_default(),
                token,
            ))
        }
        .await;
        let Some((track, epoch, offset, tools, url, token)) = step else {
            if changed.changed().await.is_err() {
                return;
            }
            continue;
        };
        let play = async {
            let token = token.ok_or_else(|| anyhow::anyhow!("voice unavailable"))?;
            let source = resolve(&tools, &track.url).await?;
            let mut pipe = Pipeline::start(&tools, &source, offset, &url, &token).await?;
            {
                let s = weak
                    .upgrade()
                    .map(AppState)
                    .ok_or_else(|| anyhow::anyhow!("server stopped"))?;
                let _guard = s.writes.lock().await;
                let mut q = load(&s, &room)
                    .await
                    .map_err(|_| anyhow::anyhow!("queue unavailable"))?;
                anyhow::ensure!(q.epoch == epoch, "playback changed");
                if let Some(t) = active_mut(&mut q.view) {
                    t.title = source.title;
                    t.duration = Some(source.duration);
                    t.thumbnail = source.thumbnail;
                    t.state = MusicTrackState::Playing;
                }
                save(&s, &mut q)
                    .await
                    .map_err(|_| anyhow::anyhow!("queue unavailable"))?;
            }
            pipe.finished().await
        };
        tokio::pin!(play);
        let result = loop {
            tokio::select! {
                result=&mut play=>break Some(result),
                event=changed.changed()=>{
                    if event.is_err(){return;}
                    let s=match weak.upgrade(){Some(s)=>AppState(s),None=>return};
                    // The pinned playback future may hold writes while saving its
                    // state. Never wait for that lock while playback is not polled.
                    let current = sqlx::query_scalar::<_, i64>("SELECT epoch FROM music_rooms WHERE room_id=?")
                        .bind(&room).fetch_optional(&s.db).await;
                    if !matches!(current, Ok(Some(value)) if value == epoch) {break None;}
                }
            }
        };
        if let Some(result) = result {
            let Some(s) = weak.upgrade().map(AppState) else {
                return;
            };
            let _guard = s.writes.lock().await;
            let Ok(mut q) = load(&s, &room).await else {
                return;
            };
            if q.epoch != epoch {
                continue;
            }
            if result.is_ok() {
                q.view.queue.retain(|t| t.id != track.id);
            } else if let Some(t) = q.view.queue.iter_mut().find(|t| t.id == track.id) {
                t.state = MusicTrackState::Failed;
                t.title = "Could not load this track".into();
            }
            q.view.position_seconds = 0.;
            q.epoch += 1;
            if save(&s, &mut q).await.is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Shell stand-ins for ffmpeg and den-dj, so the real supervisor drives real
    /// subprocesses. They imitate the two behaviours that matter: a decoder that
    /// dies without producing audio, and a publisher that stays alive after its
    /// stream ends, which is what den-dj does when no Opus header ever arrives.
    /// Fixtures that wait `exec` so the pipeline kills the waiting process itself
    /// rather than stranding it behind a dead shell.
    struct Fixtures(std::path::PathBuf);
    impl Drop for Fixtures {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    impl Fixtures {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("den-pipeline-{}", ulid::Ulid::new()));
            std::fs::create_dir_all(&dir).expect("fixture directory");
            Self(dir)
        }
        fn at(&self, name: &str) -> String {
            self.0.join(name).to_string_lossy().into()
        }
        fn script(&self, name: &str, body: &str) -> String {
            let path = self.0.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("fixture script");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                .expect("fixture permissions");
            path.to_string_lossy().into()
        }
    }
    async fn start(tools: &Tools) -> Pipeline {
        let source = Source {
            url: "https://audio.googlevideo.com/test".into(),
            title: "Test song".into(),
            duration: 120.,
            thumbnail: None,
        };
        Pipeline::start(tools, &source, 0., "ws://127.0.0.1:1", "test-token")
            .await
            .expect("pipeline start")
    }
    async fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !done() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
    }
    /// Cleanup evidence reads procfs, so it is only checked on Linux, where Den
    /// is served from. A process that has vanished or become a zombie was
    /// terminated; whether it has been reaped yet is the runtime's business.
    #[cfg(target_os = "linux")]
    async fn wait_gone(what: &str, pid: u32) {
        wait_for(&format!("the {what} to be killed"), || {
            !std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                stat.rsplit_once(')')
                    .and_then(|(_, rest)| rest.split_whitespace().next())
                    .is_some_and(|state| state != "Z")
            })
        })
        .await;
    }
    #[cfg(not(target_os = "linux"))]
    async fn wait_gone(_what: &str, _pid: u32) {}

    #[tokio::test]
    async fn a_decoder_that_stops_early_does_not_wait_on_a_live_publisher() {
        // The same header-less stream is reached two ways: a decoder that fails,
        // and one that believes it succeeded without producing any audio. Either
        // way the publisher can never finish writing, so neither may be waited on.
        // Both are recorded before asserting, so a failure names both cases.
        let mut outcomes = vec![];
        for exit in [1, 0] {
            let fixtures = Fixtures::new();
            let tools = Tools {
                resolver: "true".into(),
                ffmpeg: fixtures.script("ffmpeg", &format!("exit {exit}\n")),
                publisher: fixtures.script(
                    "den-dj",
                    &format!(
                        "printf 'READY\\n'\ncat > {}\nexec sleep 600\n",
                        fixtures.at("stream")
                    ),
                ),
            };
            let mut pipe = start(&tools).await;
            let (decoder, publisher) = (
                pipe.ffmpeg.id().expect("decoder pid"),
                pipe.dj.id().expect("publisher pid"),
            );
            outcomes.push((
                exit,
                match tokio::time::timeout(Duration::from_secs(5), pipe.finished()).await {
                    Ok(Err(_)) => "failed the track",
                    Ok(Ok(())) => "finished the track as a success",
                    Err(_) => "was still waiting for the publisher after 5s",
                },
            ));
            drop(pipe);
            wait_gone("decoder", decoder).await;
            wait_gone("publisher", publisher).await;
        }
        assert_eq!(
            outcomes,
            [(1, "failed the track"), (0, "failed the track")],
            "a decoder that stops without audio must end the track"
        );
    }

    #[tokio::test]
    async fn a_finished_decoder_lets_the_publisher_drain_before_the_track_ends() {
        let fixtures = Fixtures::new();
        let tools = Tools {
            resolver: "true".into(),
            ffmpeg: fixtures.script("ffmpeg", "printf 'OggS-fixture-audio'\n"),
            publisher: fixtures.script(
                "den-dj",
                &format!(
                    "printf 'READY\\n'\ncat > {stream}\nwhile [ ! -e {release} ]; do sleep 0.02; done\n: > {drained}\n",
                    stream = fixtures.at("stream"),
                    release = fixtures.at("release"),
                    drained = fixtures.at("drained"),
                ),
            ),
        };
        let mut pipe = start(&tools).await;
        let mut finished = std::pin::pin!(pipe.finished());
        // The publisher holds the track open until this test releases it, so the
        // still-playing assertion is a barrier rather than a race with a sleep.
        wait_for("the decoder's audio to reach the publisher", || {
            std::fs::read_to_string(fixtures.at("stream")).is_ok_and(|s| s == "OggS-fixture-audio")
        })
        .await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut finished)
                .await
                .is_err(),
            "the track is still playing while the publisher drains"
        );
        std::fs::write(fixtures.at("release"), "").expect("release the publisher");
        tokio::time::timeout(Duration::from_secs(5), finished)
            .await
            .expect("publisher drain finishes the track")
            .expect("a drained track succeeds");
        assert!(std::path::Path::new(&fixtures.at("drained")).exists());
    }

    #[tokio::test]
    async fn a_failing_publisher_does_not_wait_on_its_decoder() {
        let fixtures = Fixtures::new();
        let tools = Tools {
            resolver: "true".into(),
            ffmpeg: fixtures.script("ffmpeg", "printf 'OggS'\nexec sleep 600\n"),
            publisher: fixtures.script("den-dj", "printf 'READY\\n'\nexit 3\n"),
        };
        let mut pipe = start(&tools).await;
        let (decoder, publisher) = (
            pipe.ffmpeg.id().expect("decoder pid"),
            pipe.dj.id().expect("publisher pid"),
        );
        let result = tokio::time::timeout(Duration::from_secs(5), pipe.finished())
            .await
            .expect("playback must report the publisher failure, not wait for the decoder");
        assert!(
            result.is_err(),
            "a failed publisher is not a finished track"
        );
        drop(pipe);
        wait_gone("decoder", decoder).await;
        wait_gone("publisher", publisher).await;
    }
}
