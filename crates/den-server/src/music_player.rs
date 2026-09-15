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
    async fn finished(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(self.dj.wait().await?.success(), "publisher failed");
        anyhow::ensure!(self.ffmpeg.wait().await?.success(), "decoder failed");
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
                    let _guard=s.writes.lock().await;
                    if load(&s,&room).await.map_or(true,|q|q.epoch!=epoch){break None;}
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
