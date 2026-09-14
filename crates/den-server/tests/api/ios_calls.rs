use super::*;
use livekit_api::access_token::AccessToken;

async fn webhook(
    t: &Test,
    channel: &str,
    event: &str,
    identity: &str,
    participant: &str,
    room: &str,
) {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let body = json!({"event":event,"room":{"name":channel,"sid":room},"participant":{"identity":identity,"sid":participant}}).to_string();
    let digest = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(body.as_bytes()));
    let token = AccessToken::with_api_key("test-key", "test-secret-at-least-thirty-two-bytes")
        .with_sha256(&digest)
        .to_jwt()
        .unwrap();
    assert_eq!(
        t.http
            .post(format!("{}/livekit/webhook", t.url))
            .header("authorization", token)
            .body(body)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
}

#[tokio::test]
async fn call_answers_are_atomic_per_recipient_and_reconcile_without_exposing_credentials() {
    let t = Test::with_voice(true).await;
    let bob = t.member("answer_bob").await;
    let bob2 = t
        .post(
            "/auth/login",
            "",
            json!({"username":"answer_bob","password":"test-password-123"}),
        )
        .await;
    let carol = t.member("answer_carol").await;
    let outsider = t.member("answer_outsider").await;
    let dm = t
        .post(
            "/dms",
            &t.admin.token,
            json!({"member_ids":[bob.user.id,carol.user.id]}),
        )
        .await;
    let channel = dm["id"].as_str().unwrap();
    let path = format!("/calls/{channel}/invite");
    webhook(
        &t,
        channel,
        "participant_joined",
        &format!("{}:caller", t.admin.user.id),
        "PA_caller",
        "RM_call",
    )
    .await;
    let invite = t.post(&path, &t.admin.token, json!({})).await;
    let id = invite["id"]
        .as_str()
        .expect("invitation has its own generation ID");
    let answer_a = json!({"invitation_id":id,"answer_id":"b5ec4983-4e47-4b26-940c-2dd01f65d4b6"});
    let answer_b = json!({"invitation_id":id,"answer_id":"9dd052ca-6587-41d5-a1f5-ac00f01a5341"});
    assert_eq!(
        t.req(Method::POST, &format!("{path}/accept"), &outsider.token)
            .json(&answer_a)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.req(Method::POST, &format!("{path}/accept"), &t.admin.token)
            .json(&answer_a)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let (a, b) = tokio::join!(
        t.req(Method::POST, &format!("{path}/accept"), &bob.token)
            .json(&answer_a)
            .send(),
        t.req(
            Method::POST,
            &format!("{path}/accept"),
            bob2["token"].as_str().unwrap()
        )
        .json(&answer_b)
        .send()
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(matches!(
        (a.status().as_u16(), b.status().as_u16()),
        (200, 409) | (409, 200)
    ));
    let (winning_token, winning_body, loser) = if a.status() == 200 {
        (bob.token.as_str(), &answer_a, b)
    } else {
        (bob2["token"].as_str().unwrap(), &answer_b, a)
    };
    assert_eq!(
        loser.json::<Value>().await.unwrap()["error"],
        "answered_elsewhere"
    );
    let repeated = t
        .post(
            &format!("{path}/accept"),
            winning_token,
            winning_body.clone(),
        )
        .await;
    assert_eq!(repeated["state"], "active");
    assert_eq!(repeated["accepted"].as_array().unwrap().len(), 1);
    assert_eq!(repeated["accepted"][0]["user_id"], bob.user.id);
    let state = t
        .post(&format!("{path}/accept"), &carol.token, answer_a.clone())
        .await;
    assert_eq!(
        state["accepted"].as_array().unwrap().len(),
        2,
        "Each group recipient can answer independently"
    );
    assert_eq!(
        t.req(Method::POST, &format!("{path}/cancel"), &t.admin.token)
            .json(&json!({"invitation_id":id}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    for token in [&bob.token, &carol.token] {
        let current = t
            .req(Method::GET, &format!("/calls/invitations/{id}"), token)
            .send()
            .await
            .unwrap();
        assert_eq!(current.status(), 200);
        assert_eq!(current.json::<Value>().await.unwrap(), state);
    }
    assert_eq!(
        t.req(
            Method::GET,
            &format!("/calls/invitations/{id}"),
            &outsider.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
    assert!(!state.to_string().contains("token"));
    assert!(!state.to_string().contains("credential"));
    // Additional devices still use the ordinary media endpoint after one wins ringing.
    t.post(
        &format!("/calls/{channel}/token"),
        bob2["token"].as_str().unwrap(),
        json!({}),
    )
    .await;
}

#[tokio::test]
async fn media_departure_preserves_sibling_connections_and_ignores_old_room_finish() {
    let t = Test::with_voice(true).await;
    let bob = t.member("media_bob").await;
    let dm = t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let channel = dm["id"].as_str().unwrap();
    let phone = format!("{}:phone", t.admin.user.id);
    let desktop = format!("{}:desktop", t.admin.user.id);
    webhook(
        &t,
        channel,
        "participant_joined",
        &phone,
        "PA_phone",
        "RM_old",
    )
    .await;
    webhook(
        &t,
        channel,
        "participant_joined",
        &desktop,
        "PA_desktop",
        "RM_old",
    )
    .await;
    let invite = t
        .post(
            &format!("/calls/{channel}/invite"),
            &t.admin.token,
            json!({}),
        )
        .await;
    let id = invite["id"].as_str().unwrap();
    t.post(
        &format!("/calls/{channel}/invite/accept"),
        &bob.token,
        json!({"invitation_id":id,"answer_id":"d7c58c92-d1bf-4ba1-9f61-29a34999cb3b"}),
    )
    .await;
    async fn state(t: &Test, id: &str) -> Value {
        t.req(
            Method::GET,
            &format!("/calls/invitations/{id}"),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
    }
    webhook(
        &t,
        channel,
        "participant_left",
        &phone,
        "PA_phone",
        "RM_old",
    )
    .await;
    assert_eq!(state(&t, id).await["state"], "active");
    webhook(
        &t,
        channel,
        "participant_joined",
        &phone,
        "PA_new",
        "RM_new",
    )
    .await;
    webhook(&t, channel, "room_finished", "", "", "RM_old").await;
    assert_eq!(
        state(&t, id).await["state"],
        "active",
        "A late old-room callback cannot end the new transport"
    );
    webhook(&t, channel, "room_finished", "", "", "RM_new").await;
    assert_eq!(state(&t, id).await["state"], "ended");
    // Retained state is available for a reconnecting caller and callee.
    let list: Value = t
        .req(Method::GET, "/calls/invitations", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["invitation"]["id"] == id && c["state"] == "ended"));
}

struct MediaFixture {
    url: String,
    peers: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
    available: std::sync::Arc<std::sync::atomic::AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for MediaFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl MediaFixture {
    async fn new() -> Self {
        use axum::{
            body::Bytes, http::StatusCode as Status, response::IntoResponse, routing::post, Router,
        };
        use prost::Message;
        let peers = std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::<
            String,
            Vec<String>,
        >::new()));
        let available = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let state = peers.clone();
        let on = available.clone();
        let rooms = post(move || {
            let state = state.clone();
            let on = on.clone();
            async move {
                if !on.load(std::sync::atomic::Ordering::SeqCst) {
                    return Status::SERVICE_UNAVAILABLE.into_response();
                }
                let rooms = state
                    .lock()
                    .await
                    .keys()
                    .map(|name| livekit_protocol::Room {
                        name: name.clone(),
                        sid: format!("RM_{name}"),
                        creation_time: 1,
                        ..Default::default()
                    })
                    .collect();
                livekit_protocol::ListRoomsResponse { rooms }
                    .encode_to_vec()
                    .into_response()
            }
        });
        let state = peers.clone();
        let on = available.clone();
        let participants = post(move |body: Bytes| {
            let state = state.clone();
            let on = on.clone();
            async move {
                if !on.load(std::sync::atomic::Ordering::SeqCst) {
                    return Status::SERVICE_UNAVAILABLE.into_response();
                }
                let request = livekit_protocol::ListParticipantsRequest::decode(body).unwrap();
                let participants = state
                    .lock()
                    .await
                    .get(&request.room)
                    .into_iter()
                    .flatten()
                    .map(|identity| livekit_protocol::ParticipantInfo {
                        identity: identity.clone(),
                        sid: format!("PA_{identity}"),
                        ..Default::default()
                    })
                    .collect();
                livekit_protocol::ListParticipantsResponse { participants }
                    .encode_to_vec()
                    .into_response()
            }
        });
        let app = Router::new()
            .route("/twirp/livekit.RoomService/ListRooms", rooms)
            .route("/twirp/livekit.RoomService/ListParticipants", participants);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            peers,
            available,
            task,
        }
    }
}

#[tokio::test]
async fn abandoned_answer_grace_requires_authoritative_absence_and_preserves_other_connections() {
    let media = MediaFixture::new().await;
    let t = Test::with_voice_at(Some(&media.url)).await;
    let bob = t.member("grace_bob").await;
    let carol = t.member("grace_carol").await;
    let mut invites = Vec::new();
    for user in [&bob, &carol] {
        let dm = t
            .post("/dms", &t.admin.token, json!({"member_ids":[user.user.id]}))
            .await;
        let channel = dm["id"].as_str().unwrap();
        let mut participants = vec![format!("{}:caller", t.admin.user.id)];
        if user.user.id == carol.user.id {
            participants.push(format!("{}:other-device", carol.user.id));
        }
        media
            .peers
            .lock()
            .await
            .insert(channel.into(), participants);
        let invite = t
            .post(
                &format!("/calls/{channel}/invite"),
                &t.admin.token,
                json!({}),
            )
            .await;
        t.post(&format!("/calls/{channel}/invite/accept"),&user.token,json!({"invitation_id":invite["id"],"answer_id":"d7c58c92-d1bf-4ba1-9f61-29a34999cb3b"})).await;
        invites.push(invite);
    }
    async fn read(t: &Test, id: &Value) -> Value {
        t.req(
            Method::GET,
            &format!("/calls/invitations/{}", id.as_str().unwrap()),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
    }
    assert_eq!(
        read(&t, &invites[1]["id"]).await["accepted"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    tokio::time::sleep(Duration::from_secs(21)).await;
    media
        .available
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        read(&t, &invites[0]["id"]).await["state"],
        "active",
        "LiveKit outage is unknown, never evidence of zero connections"
    );
    media
        .available
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let abandoned = read(&t, &invites[0]["id"]).await;
    assert_eq!(abandoned["state"], "ringing");
    assert_eq!(abandoned["accepted"], json!([]));
    assert_eq!(abandoned["declined_user_ids"], json!([bob.user.id]));
    let connected = read(&t, &invites[1]["id"]).await;
    assert_eq!(connected["state"], "active");
    assert_eq!(connected["accepted"].as_array().unwrap().len(), 1);
}

async fn next_call_event(socket: &mut realtime::Socket, kind: &str) -> Value {
    tokio::time::timeout(
        Duration::from_secs(if kind == "call_invite_expired" { 50 } else { 3 }),
        async {
            loop {
                match socket.next().await.unwrap().unwrap() {
                    Frame::Text(text) => {
                        let value: Value = serde_json::from_str(&text).unwrap();
                        if value["type"] == kind {
                            return value;
                        }
                    }
                    Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
                    _ => {}
                }
            }
        },
    )
    .await
    .expect("Expected lifecycle event")
}

#[tokio::test]
async fn cancel_answer_race_is_linear_and_stale_generations_cannot_change_replacements() {
    let t = Test::with_voice(true).await;
    let bob = t.member("race_bob").await;
    let outsider = t.member("race_outsider").await;
    let dm = t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let channel = dm["id"].as_str().unwrap();
    webhook(
        &t,
        channel,
        "participant_joined",
        &format!("{}:caller", t.admin.user.id),
        "PA_caller",
        "RM_race",
    )
    .await;
    let path = format!("/calls/{channel}/invite");
    let mut peer = realtime::socket(&t, &bob.token).await;
    let mut hidden = realtime::socket(&t, &outsider.token).await;
    next_call_event(&mut peer, "resync").await;
    next_call_event(&mut hidden, "resync").await;
    for _ in 0..4 {
        let invite = t.post(&path, &t.admin.token, json!({})).await;
        let key = json!({"invitation_id":invite["id"]});
        let answer = json!({"invitation_id":invite["id"],"answer_id":"d7c58c92-d1bf-4ba1-9f61-29a34999cb3b"});
        for action in ["cancel", "end"] {
            assert_eq!(
                t.req(Method::POST, &format!("{path}/{action}"), &bob.token)
                    .json(&key)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                403
            );
            assert_eq!(
                t.req(Method::POST, &format!("{path}/{action}"), &outsider.token)
                    .json(&key)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                404
            );
        }
        let (accept, cancel) = tokio::join!(
            t.req(Method::POST, &format!("{path}/accept"), &bob.token)
                .json(&answer)
                .send(),
            t.req(Method::POST, &format!("{path}/cancel"), &t.admin.token)
                .json(&key)
                .send()
        );
        let pair = (
            accept.unwrap().status().as_u16(),
            cancel.unwrap().status().as_u16(),
        );
        assert!(matches!(pair, (200, 409) | (409, 204)), "{pair:?}");
        next_call_event(
            &mut peer,
            if pair.0 == 200 {
                "call_invite_accepted"
            } else {
                "call_invite_cancelled"
            },
        )
        .await;
        if pair.0 == 200 {
            assert_eq!(t.req(Method::POST,&format!("{path}/decline"),&bob.token).json(&json!({"invitation_id":invite["id"],"from_user_id":invite["from_user_id"],"expires_at":invite["expires_at"]})).send().await.unwrap().status(),409);
        } else {
            assert_eq!(
                t.req(Method::POST, &format!("{path}/cancel"), &t.admin.token)
                    .json(&key)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                204
            );
        }
        assert_eq!(
            t.req(Method::POST, &format!("{path}/end"), &t.admin.token)
                .json(&key)
                .send()
                .await
                .unwrap()
                .status(),
            204
        );
        let next = t.post(&path, &t.admin.token, json!({})).await;
        assert_ne!(next["id"], invite["id"]);
        assert_eq!(
            t.req(Method::POST, &format!("{path}/accept"), &bob.token)
                .json(&answer)
                .send()
                .await
                .unwrap()
                .status(),
            409
        );
        assert_eq!(
            t.req(Method::POST, &format!("{path}/end"), &t.admin.token)
                .json(&key)
                .send()
                .await
                .unwrap()
                .status(),
            204
        );
        let unchanged = t
            .req(
                Method::GET,
                &format!("/calls/invitations/{}", next["id"].as_str().unwrap()),
                &bob.token,
            )
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(unchanged["state"], "ringing");
        assert_eq!(unchanged["accepted"], json!([]));
        assert_eq!(
            t.req(Method::POST, &format!("{path}/cancel"), &t.admin.token)
                .json(&json!({"invitation_id":next["id"]}))
                .send()
                .await
                .unwrap()
                .status(),
            204
        );
    }
    // No private ringing or lifecycle payload may reach an unrelated account.
    assert!(tokio::time::timeout(Duration::from_millis(150), async {
        loop {
            if let Some(Ok(Frame::Text(v))) = hidden.next().await {
                let v: Value = serde_json::from_str(&v).unwrap();
                if v["type"].as_str().unwrap_or("").starts_with("call_") {
                    return;
                }
            }
        }
    })
    .await
    .is_err());
}

#[tokio::test]
async fn restart_retains_answers_and_expiry_closes_only_pending_ringing() {
    let media = MediaFixture::new().await;
    let mut t = Test::with_voice_at(Some(&media.url)).await;
    let bob = t.member("restart_bob").await;
    let carol = t.member("restart_carol").await;
    let mut invitations = Vec::new();
    for users in [
        vec![bob.user.id.clone()],
        vec![bob.user.id.clone(), carol.user.id.clone()],
    ] {
        let dm = t
            .post("/dms", &t.admin.token, json!({"member_ids":users}))
            .await;
        let channel = dm["id"].as_str().unwrap();
        media.peers.lock().await.insert(
            channel.into(),
            vec![
                format!("{}:caller", t.admin.user.id),
                format!("{}:peer", bob.user.id),
            ],
        );
        invitations.push(
            t.post(
                &format!("/calls/{channel}/invite"),
                &t.admin.token,
                json!({}),
            )
            .await,
        );
    }
    let active = &invitations[1];
    let channel = active["channel_id"].as_str().unwrap();
    let answer =
        json!({"invitation_id":active["id"],"answer_id":"d7c58c92-d1bf-4ba1-9f61-29a34999cb3b"});
    t.post(
        &format!("/calls/{channel}/invite/accept"),
        &bob.token,
        answer.clone(),
    )
    .await;
    t.task.abort();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    t.url = format!("http://{}", listener.local_addr().unwrap());
    t.state = AppState::open(
        t.dir.join("den.db"),
        t.dir.join("uploads"),
        t.dir.join("bootstrap.key"),
        t.url.clone(),
        1024 * 1024,
    )
    .await
    .unwrap()
    .with_livekit(
        media.url.clone(),
        "test-key".into(),
        "test-secret-at-least-thirty-two-bytes".into(),
    );
    t.state.run_invitation_expiry();
    let app = den_server::router_with_web(t.state.clone(), t.dir.join("spa"));
    t.task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let mut peer = realtime::socket(&t, &bob.token).await;
    next_call_event(&mut peer, "resync").await;
    // A delayed old room callback after restart cannot prove the current room empty.
    webhook(
        &t,
        channel,
        "room_finished",
        "",
        "",
        "RM_stale_before_restart",
    )
    .await;
    let restored = t
        .post(
            &format!("/calls/{channel}/invite/accept"),
            &bob.token,
            answer.clone(),
        )
        .await;
    assert_eq!(restored["accepted"].as_array().unwrap().len(), 1);
    // Drive the actual persisted deadline, including the restarted timer and socket delivery.
    let mut expired = std::collections::HashSet::new();
    for _ in 0..2 {
        let event = next_call_event(&mut peer, "call_invite_expired").await;
        expired.insert(
            event["call"]["invitation"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert_eq!(expired.len(), 2);
    let states: Value = t
        .req(Method::GET, "/calls/invitations", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(states
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["invitation"]["id"] == invitations[0]["id"] && v["state"] == "expired"));
    assert!(states
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["invitation"]["id"] == active["id"] && v["state"] == "active"));
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/calls/{channel}/invite/accept"),
            &carol.token
        )
        .json(&answer)
        .send()
        .await
        .unwrap()
        .status(),
        409
    );
    assert_eq!(
        t.post(
            &format!("/calls/{channel}/invite/accept"),
            &bob.token,
            answer
        )
        .await["state"],
        "active"
    );
    assert!(tokio::time::timeout(
        Duration::from_millis(150),
        next_call_event(&mut peer, "call_invite_expired")
    )
    .await
    .is_err());
}
