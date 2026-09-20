use super::*;

#[tokio::test]
async fn voice_preferences_merge_devices_validate_and_sync_only_to_owner() {
    let t = Test::new().await;
    let alice = t.member("voice_alice").await;
    let bob = t.member("voice_bob").await;
    let path = "/users/me/voice";
    assert_eq!(
        t.req(Method::GET, path, "").send().await.unwrap().status(),
        401
    );
    assert_eq!(
        t.http
            .put(format!("{}{path}", t.url))
            .header("Cookie", format!("den_session={}", alice.token))
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let mut sockets = vec![];
    for session in [&alice, &alice, &bob] {
        let mut request = format!("{}/ws", t.url.replace("http:", "ws:"))
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", session.token).parse().unwrap(),
        );
        let (mut ws, _) = connect_async(request).await.unwrap();
        ws.next().await.unwrap().unwrap();
        sockets.push(ws);
    }
    for patch in [
        json!({"microphones":{"mic-a":{"gain":0.5,"noise_suppression":false}}}),
        json!({"cameras":{"cam-a":{"resolution":"1080p","frame_rate":60,"mirror":false,"background":"light_blur"}}}),
        json!({"microphones":{"mic-b":{"gain":2}}}),
    ] {
        assert_eq!(
            t.req(Method::PUT, path, &alice.token)
                .json(&patch)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    for socket in &mut sockets[..2] {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(Ok(Frame::Text(text))) = socket.next().await {
                    let event: Value = serde_json::from_str(&text).unwrap();
                    if event["type"] == "voice_preferences_updated"
                        && event["preferences"]["microphones"]["mic-b"]["gain"] == 2.0
                    {
                        break;
                    }
                }
            }
        })
        .await
        .unwrap();
    }
    let leaked = tokio::time::timeout(Duration::from_millis(200), async {
        while let Some(Ok(Frame::Text(text))) = sockets[2].next().await {
            if serde_json::from_str::<Value>(&text).unwrap()["type"] == "voice_preferences_updated"
            {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(!leaked);
    let saved: VoicePreferences = t
        .req(Method::GET, path, &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved.microphones["mic-a"].gain, 0.5);
    assert!(!saved.microphones["mic-a"].noise_suppression);
    assert_eq!(saved.microphones["mic-b"].gain, 2.0);
    assert_eq!(saved.cameras["cam-a"].resolution, CameraResolution::FullHd);
    assert_eq!(
        saved.cameras["cam-a"].background,
        CameraBackground::LightBlur
    );
    let other: VoicePreferences = t
        .req(Method::GET, path, &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(other, VoicePreferences::default());
    for invalid in [
        json!({"microphones":{"bad":{"gain":2.1}}}),
        json!({"cameras":{"bad":{"frame_rate":17}}}),
        json!({"microphones":{"":{"gain":1}}}),
        json!({"cameras":{"bad":{"resolution":"4k"}}}),
        json!({"cameras":{"bad":{"background":"sparkles"}}}),
    ] {
        assert!(t
            .req(Method::PUT, path, &alice.token)
            .json(&invalid)
            .send()
            .await
            .unwrap()
            .status()
            .is_client_error());
    }
    let after: VoicePreferences = t
        .req(Method::GET, path, &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after, saved);
    let persisted: String =
        sqlx::query_scalar("SELECT preferences FROM user_voice_preferences WHERE user_id=?")
            .bind(&alice.user.id)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<VoicePreferences>(&persisted).unwrap(),
        saved
    );
}
