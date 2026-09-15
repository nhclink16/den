use super::*;
use std::os::unix::fs::PermissionsExt;
struct Resolver(PathBuf);
impl Resolver {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("den-resolver-{}", ulid::Ulid::new()));
        std::fs::write(&path,"#!/bin/sh\nfor arg do video=$arg; done\ncase \"$video\" in *badbadbad00) exit 1;; esac\nprintf '%s\\n' '{\"url\":\"https://audio.googlevideo.com/test\",\"title\":\"Test song\",\"duration\":120,\"thumbnail\":\"https://i.ytimg.com/vi/aaaaaaaaaaa/default.jpg\"}'\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Resolver {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
async fn room(t: &Test) -> String {
    let channels: Vec<Channel> = t
        .req(Method::GET, "/channels", &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    channels
        .into_iter()
        .find(|c| c.kind == ChannelKind::Voice)
        .unwrap()
        .id
}
async fn queue(t: &Test, path: &str) -> MusicQueue {
    t.req(Method::GET, path, &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
#[tokio::test]
async fn queue_order_skip_and_events_are_shared_and_authorized() {
    let resolver = Resolver::new();
    let t = Test::configured(None, Some(resolver.0.to_string_lossy().into())).await;
    let member = t.member("listener").await;
    let room = room(&t).await;
    let path = format!("/rooms/{room}/music");
    assert_eq!(
        t.req(Method::GET, &path, "").send().await.unwrap().status(),
        401
    );
    assert_eq!(
        t.http
            .post(format!("{}{path}/skip", t.url))
            .header("Cookie", format!("den_session={}", member.token))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let wrong = format!("/rooms/{}/music", t.general().await);
    assert_eq!(
        t.req(Method::GET, &wrong, &member.token)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    for url in [
        "http://127.0.0.1/secret",
        "https://example.com/video",
        "https://www.youtube.com/playlist?list=x",
        "https://youtu.be/abc",
    ] {
        assert_eq!(
            t.req(Method::POST, &format!("{path}/queue"), &member.token)
                .json(&json!({"url":url}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    t.post(
        &format!("{path}/pause"),
        &member.token,
        json!({"paused":true}),
    )
    .await;
    let mut request = format!("{}/ws?music=true", t.url.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", member.token).parse().unwrap(),
    );
    let mut legacy_request = request.clone();
    *legacy_request.uri_mut() = format!("{}/ws", t.url.replace("http:", "ws:"))
        .parse()
        .unwrap();
    let (mut legacy, _) = connect_async(legacy_request).await.unwrap();
    let (mut ws, _) = connect_async(request).await.unwrap();
    ws.next().await.unwrap().unwrap();
    for id in ["aaaaaaaaaaa", "bbbbbbbbbbb", "ccccccccccc"] {
        t.post(
            &format!("{path}/queue"),
            &member.token,
            json!({"url":format!("https://youtu.be/{id}")}),
        )
        .await;
    }
    let initial = queue(&t, &path).await;
    let mut ids: Vec<_> = initial.queue.iter().map(|t| t.id.clone()).collect();
    ids.reverse();
    let reordered: MusicQueue = t
        .req(Method::PUT, &format!("{path}/queue/order"), &member.token)
        .json(&json!({"ids":ids}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        reordered
            .queue
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(
        t.req(Method::PUT, &format!("{path}/queue/order"), &member.token)
            .json(&json!({"ids":[ids[0],ids[0]]}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let skipped: MusicQueue = serde_json::from_value(
        t.post(&format!("{path}/skip"), &member.token, json!({}))
            .await,
    )
    .unwrap();
    assert_eq!(
        skipped
            .queue
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        ids[1..]
    );
    assert!(skipped.revision > reordered.revision);
    let revisions = tokio::time::timeout(Duration::from_secs(3), async {
        let mut seen = vec![];
        loop {
            if let Some(Ok(Frame::Text(text))) = ws.next().await {
                if let Event::MusicQueueUpdated { queue } =
                    serde_json::from_str::<Event>(&text).unwrap()
                {
                    seen.push(queue.revision);
                    if queue.revision == skipped.revision {
                        break seen;
                    }
                }
            }
        }
    })
    .await
    .unwrap();
    assert!(revisions.contains(&reordered.revision));
    assert!(revisions.windows(2).all(|p| p[0] < p[1]));
    // A pre-music socket keeps receiving known events without the new enum variant.
    legacy
        .send(Frame::Text(
            json!({"type":"typing","channel_id":room})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match legacy.next().await.unwrap().unwrap() {
                Frame::Text(text) => {
                    let event: Event = serde_json::from_str(&text).unwrap();
                    assert!(!matches!(event, Event::MusicQueueUpdated { .. }));
                    if matches!(event, Event::Typing { .. }) {
                        break;
                    }
                }
                Frame::Ping(bytes) => legacy.send(Frame::Pong(bytes)).await.unwrap(),
                _ => {}
            }
        }
    })
    .await
    .unwrap();

    let removed = &skipped.queue[0].id;
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("{path}/queue/{removed}"),
            &member.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(queue(&t, &path).await.queue.len(), 1);
}
#[tokio::test]
async fn failed_resolution_keeps_the_remaining_queue() {
    let resolver = Resolver::new();
    let t = Test::configured(None, Some(resolver.0.to_string_lossy().into())).await;
    let path = format!("/rooms/{}/music", room(&t).await);
    t.post(
        &format!("{path}/pause"),
        &t.admin.token,
        json!({"paused":true}),
    )
    .await;
    for id in ["badbadbad00", "aaaaaaaaaaa"] {
        t.post(
            &format!("{path}/queue"),
            &t.admin.token,
            json!({"url":format!("https://youtu.be/{id}")}),
        )
        .await;
    }
    let q = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let q = queue(&t, &path).await;
            if q.queue[0].state == MusicTrackState::Failed && q.queue[1].duration == Some(120.) {
                break q;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(q.queue.len(), 2);
    assert_eq!(q.queue[0].title, "Could not load this track");
    assert_eq!(q.queue[1].state, MusicTrackState::Queued);
    assert_eq!(q.queue[1].title, "Test song");
    assert!(q.paused);
}
