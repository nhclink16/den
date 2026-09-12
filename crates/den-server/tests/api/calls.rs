use super::*;
use livekit_api::access_token::{AccessToken, TokenVerifier};

#[tokio::test]
async fn call_token_authorization_and_voice_channel() {
    let t = Test::with_voice(true).await;
    let member = t.member("member").await;
    let channels: Vec<Channel> = t
        .req(Method::GET, "/channels", &member.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hangout = channels
        .iter()
        .find(|c| c.kind == ChannelKind::Voice)
        .unwrap();
    assert_eq!(hangout.name, "hangout");
    for id in [&hangout.id, &t.general().await] {
        let result = t
            .post(&format!("/calls/{id}/token"), &member.token, json!({}))
            .await;
        let claims =
            TokenVerifier::with_api_key("test-key", "test-secret-at-least-thirty-two-bytes")
                .verify(result["token"].as_str().unwrap())
                .unwrap();
        assert_eq!(claims.sub, member.user.id);
        assert_eq!(claims.name, member.user.display_name);
        assert_eq!(claims.video.room, *id);
        assert!(claims.video.room_join && claims.video.can_publish && claims.video.can_subscribe);
        assert!(!claims.video.room_admin);
    }
    let dm = t
        .post(
            "/dms",
            &member.token,
            json!({"member_ids":[t.admin.user.id]}),
        )
        .await;
    t.post(
        &format!("/calls/{}/token", dm["id"].as_str().unwrap()),
        &member.token,
        json!({}),
    )
    .await;
    let outsider = t.member("outsider").await;
    for id in ["absent", dm["id"].as_str().unwrap()] {
        assert_eq!(
            t.req(Method::POST, &format!("/calls/{id}/token"), &outsider.token)
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/channels/{}/messages", hangout.id),
            &member.token
        )
        .json(&json!({"content":"no text here"}))
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    let off = Test::new().await;
    assert_eq!(
        off.req(
            Method::POST,
            &format!("/calls/{}/token", off.general().await),
            &off.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        503
    );
}

#[tokio::test]
async fn webhook_rejects_missing_invalid_and_tampered_signatures() {
    let t = Test::with_voice(true).await;
    let channel = t.general().await;
    let body = json!({"event":"participant_joined", "room":{"name":channel,"sid":"RM_test"}, "participant":{"identity":t.admin.user.id,"sid":"PA_test"}}).to_string();
    let digest = {
        use sha2::{Digest, Sha256};
        // Use the SDK's body hash field with a standard base64 encoder.
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(body.as_bytes()))
    };
    let token = AccessToken::with_api_key("test-key", "test-secret-at-least-thirty-two-bytes")
        .with_sha256(&digest)
        .to_jwt()
        .unwrap();
    for auth in [None, Some("garbage")] {
        let mut r = t
            .http
            .post(format!("{}/livekit/webhook", t.url))
            .body(body.clone());
        if let Some(auth) = auth {
            r = r.header("authorization", auth);
        }
        assert_eq!(r.send().await.unwrap().status(), 401);
    }
    assert_eq!(
        t.http
            .post(format!("{}/livekit/webhook", t.url))
            .header("authorization", &token)
            .body(format!("{body} "))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let mut rx = t.state.events.subscribe();
    assert_eq!(
        t.http
            .post(format!("{}/livekit/webhook", t.url))
            .header("authorization", &token)
            .body(body)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::CallState {
            channel_id: channel,
            participant_ids: vec![t.admin.user.id.clone()]
        }
    );
}
