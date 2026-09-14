use super::*;

async fn register(t: &Test, session: &Session, token: &str) -> Value {
    t.post(
        "/devices",
        &session.token,
        json!({"platform":"ios","token":token,"app_version":"1.0"}),
    )
    .await
}

#[tokio::test]
async fn device_registration_is_scoped_idempotent_and_follows_account_switches() {
    let t = Test::new().await;
    let bob = t.member("device_bob").await;
    let receipt = register(&t, &t.admin, "aabb01").await;
    assert!(receipt.get("token").is_none());
    assert!(receipt.get("user_id").is_none());
    let repeated = register(&t, &t.admin, "AABB01").await;
    assert_eq!(receipt["id"], repeated["id"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM devices")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        1
    );
    let path = format!("/devices/{}", receipt["id"].as_str().unwrap());
    assert_eq!(
        t.req(Method::DELETE, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let switched = register(&t, &bob, "aabb01").await;
    assert_eq!(receipt["id"], switched["id"]);
    assert_eq!(
        t.req(Method::DELETE, &path, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.req(Method::DELETE, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        t.req(Method::DELETE, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn device_credentials_validate_input_and_logout_or_revocation_removes_only_their_devices() {
    let t = Test::new().await;
    assert_eq!(
        t.req(Method::POST, "/devices", "")
            .json(&json!({"platform":"ios","token":"aabb","app_version":"1"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for (platform, token, version) in [
        ("android", "aa", "1"),
        ("ios", "no-token", "1"),
        ("ios", "abc", "1"),
        ("ios", "", "1"),
        ("ios", "aa", ""),
    ] {
        let expected = if platform == "ios" { 400 } else { 422 };
        assert_eq!(
            t.req(Method::POST, "/devices", &t.admin.token)
                .json(&json!({"platform":platform,"token":token,"app_version":version}))
                .send()
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    register(&t, &t.admin, "aabb").await;
    let second: Session = serde_json::from_value(
        t.post(
            "/auth/login",
            "",
            json!({"username":"admin","password":"test-password-123"}),
        )
        .await,
    )
    .unwrap();
    register(&t, &second, "aacc").await;
    assert_eq!(
        t.req(Method::POST, "/auth/logout", &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT token FROM devices")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        "aacc"
    );
    let token = t
        .post("/tokens", &second.token, json!({"name":"device test"}))
        .await;
    let api_token = token["token"].as_str().unwrap();
    t.post(
        "/devices",
        api_token,
        json!({"platform":"ios","token":"aadd","app_version":"1"}),
    )
    .await;
    let id = token["credential"]["id"].as_str().unwrap();
    assert_eq!(
        t.req(Method::DELETE, &format!("/tokens/{id}"), &second.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM devices")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        1
    );
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
async fn socket(t: &Test, token: &str) -> Socket {
    let mut request = format!("{}/ws", t.url.replace("http", "ws"))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    while let Some(Ok(Frame::Text(text))) = socket.next().await {
        if matches!(
            serde_json::from_str::<Event>(&text).unwrap(),
            Event::Resync { .. }
        ) {
            break;
        }
    }
    socket
}
async fn next_invite(socket: &mut Socket) -> Option<Event> {
    tokio::time::timeout(Duration::from_millis(300), async {
        while let Some(frame) = socket.next().await {
            if let Frame::Text(text) = frame.unwrap() {
                let event: Event = serde_json::from_str(&text).unwrap();
                if matches!(event, Event::CallInvite { .. }) {
                    return Some(event);
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

#[tokio::test]
async fn invitations_require_dm_call_participation_and_only_reach_pending_recipients() {
    let t = Test::with_voice(true).await;
    let bob = t.member("invite_bob").await;
    let outsider = t.member("invite_outsider").await;
    let dm = t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let channel = dm["id"].as_str().unwrap();
    let path = format!("/calls/{channel}/invite");
    assert_eq!(
        t.req(Method::POST, &path, &outsider.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.req(Method::POST, &path, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/calls/{}/invite", t.general().await),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    t.state
        .calls
        .lock()
        .await
        .entry(channel.into())
        .or_default()
        .insert(format!("{}:phone", t.admin.user.id), "PA_test".into());
    let mut bob_socket = socket(&t, &bob.token).await;
    let mut caller_socket = socket(&t, &t.admin.token).await;
    let mut outsider_socket = socket(&t, &outsider.token).await;
    let invite = t.post(&path, &t.admin.token, json!({})).await;
    let repeated = t.post(&path, &t.admin.token, json!({})).await;
    assert_eq!(invite, repeated);
    let until = invite["expires_at"].as_i64().unwrap();
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!((44..=45).contains(&(until - time)));
    let event = Event::CallInvite {
        channel_id: channel.into(),
        from_user_id: t.admin.user.id.clone(),
        expires_at: until,
    };
    assert_eq!(next_invite(&mut bob_socket).await, Some(event.clone()));
    assert_eq!(next_invite(&mut bob_socket).await, None);
    assert_eq!(next_invite(&mut caller_socket).await, None);
    assert_eq!(next_invite(&mut outsider_socket).await, None);
    let decline = format!("{path}/decline");
    let decline_body = json!({"from_user_id":t.admin.user.id,"expires_at":until});
    assert_eq!(
        t.req(Method::POST, &decline, &outsider.token)
            .json(&decline_body)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.req(Method::POST, &decline, &t.admin.token)
            .json(&decline_body)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    for _ in 0..2 {
        assert_eq!(
            t.req(Method::POST, &decline, &bob.token)
                .json(&decline_body)
                .send()
                .await
                .unwrap()
                .status(),
            204
        );
    }
    t.state.events.send(event.clone()).unwrap();
    assert_eq!(next_invite(&mut bob_socket).await, None);
    sqlx::query("UPDATE call_invite_recipients SET declined=0")
        .execute(&t.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE call_invitations SET expires_at=0")
        .execute(&t.state.db)
        .await
        .unwrap();
    t.state.events.send(event).unwrap();
    assert_eq!(next_invite(&mut bob_socket).await, None);
    assert_eq!(
        t.req(Method::POST, &decline, &bob.token)
            .json(&decline_body)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn concurrent_callers_cannot_replace_a_live_invite_but_expiry_allows_a_new_one() {
    let t = Test::with_voice(true).await;
    let bob = t.member("race_bob").await;
    let recipient = t.member("race_recipient").await;
    let dm = t
        .post(
            "/dms",
            &t.admin.token,
            json!({"member_ids":[bob.user.id,recipient.user.id]}),
        )
        .await;
    let channel = dm["id"].as_str().unwrap();
    for user in [&t.admin.user.id, &bob.user.id] {
        t.state
            .calls
            .lock()
            .await
            .entry(channel.into())
            .or_default()
            .insert(format!("{user}:phone"), format!("PA_{user}"));
    }
    let path = format!("/calls/{channel}/invite");
    let (a, b) = tokio::join!(
        t.req(Method::POST, &path, &t.admin.token).send(),
        t.req(Method::POST, &path, &bob.token).send()
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(matches!(
        (a.status().as_u16(), b.status().as_u16()),
        (200, 409) | (409, 200)
    ));
    let (winner, loser) = if a.status() == 200 {
        (&t.admin, &bob)
    } else {
        (&bob, &t.admin)
    };
    let active = t.post(&path, &winner.token, json!({})).await;
    assert_eq!(active["from_user_id"], winner.user.id);
    sqlx::query("UPDATE call_invitations SET expires_at=0 WHERE channel_id=?")
        .bind(channel)
        .execute(&t.state.db)
        .await
        .unwrap();
    let replacement = t.post(&path, &loser.token, json!({})).await;
    assert_eq!(replacement["from_user_id"], loser.user.id);
    let stale = t
        .req(Method::POST, &format!("{path}/decline"), &recipient.token)
        .json(&json!({"from_user_id":active["from_user_id"],"expires_at":active["expires_at"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 404);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT declined FROM call_invite_recipients WHERE user_id=?")
            .bind(&recipient.user.id)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        0
    );
    t.state.cleanup().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM call_invitations")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn openapi_preserves_native_optional_ids_and_query_parameter_locations() {
    let t = Test::new().await;
    let schema: Value = t
        .http
        .get(format!("{}/openapi.json", t.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    for (path, query) in [
        ("/channels/{id}/messages", vec!["before", "after", "limit"]),
        (
            "/search/messages",
            vec!["q", "channel_id", "before", "limit"],
        ),
    ] {
        let parameters = schema["paths"][path]["get"]["parameters"]
            .as_array()
            .unwrap();
        for name in query {
            assert_eq!(
                parameters.iter().find(|p| p["name"] == name).unwrap()["in"],
                "query"
            );
        }
    }
    for (model, field) in [
        ("Channel", "category_id"),
        ("Message", "reply_to"),
        ("ChannelReadState", "last_read_id"),
        ("CreateMessage", "reply_to"),
        ("OpenTerminal", "channel_id"),
    ] {
        let field = &schema["components"]["schemas"][model]["properties"][field];
        assert_eq!(field["type"], json!(["string", "null"]));
        assert!(field.get("oneOf").is_none());
    }
    assert!(schema["paths"]["/devices"]["post"].is_object());
    assert!(schema["paths"]["/calls/{channel_id}/invite"]["post"].is_object());
}
