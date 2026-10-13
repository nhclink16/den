use super::*;

#[tokio::test]
async fn channel_crud_dm_privacy_messages_and_cursor_pagination() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let a = &t.admin.token;
    let category = t
        .post("/categories", a, json!({"name":"Games","position":1}))
        .await;
    let channel = t
        .post(
            "/channels",
            a,
            json!({"name":"gaming","category_id":category["id"],"position":1}),
        )
        .await;
    let cid = channel["id"].as_str().unwrap();
    assert_eq!(
        t.req(Method::POST, "/channels", &alice.token)
            .json(&json!({"name":"nope","position":0}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::PUT, &format!("/channels/{cid}"), a)
            .json(&json!({"name":"games","position":2}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let private = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let dm = private["id"].as_str().unwrap();
    let same = t
        .post("/dms", &bob.token, json!({"member_ids":[alice.user.id]}))
        .await;
    assert_eq!(private["id"], same["id"]);
    assert_eq!(
        t.req(Method::GET, &format!("/channels/{dm}/messages"), a)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let visible: Vec<Channel> = t
        .req(Method::GET, "/channels", a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(visible.iter().all(|c| c.id != dm));
    let path = format!("/channels/{cid}/messages");
    let mut ids = Vec::new();
    for text in ["one", "two", "three"] {
        ids.push(
            t.post(&path, &alice.token, json!({"content":text})).await["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    assert!(ids.windows(2).all(|w| w[0] < w[1]));
    let latest: Vec<Message> = t
        .req(Method::GET, &format!("{path}?limit=2"), a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        latest
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        ["two", "three"]
    );
    let before: Vec<Message> = t
        .req(Method::GET, &format!("{path}?before={}&limit=2", ids[1]), a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].content, "one");
    let after: Vec<Message> = t
        .req(Method::GET, &format!("{path}?after={}&limit=1", ids[0]), a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after[0].content, "two");
    let target = format!("/messages/{}", ids[0]);
    assert_eq!(
        t.req(Method::PATCH, &target, &bob.token)
            .json(&json!({"content":"stolen"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let edited: Message = t
        .req(Method::PATCH, &target, &alice.token)
        .json(&json!({"content":"edited"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(edited.edited_at.is_some());
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/channels/{dm}/messages"),
            &alice.token
        )
        .json(&json!({"content":"cross-channel reply","reply_to":ids[0]}))
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    assert_eq!(
        t.req(Method::DELETE, &target, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::DELETE, &target, a)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/categories/{}", category["id"].as_str().unwrap()),
            a
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    assert_eq!(
        t.req(Method::DELETE, &format!("/channels/{cid}"), a)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
}

#[tokio::test]
async fn room_list_previews_the_newest_room_message_and_never_a_private_one() {
    let t = Test::new().await;
    let alice = t.member("preview_alice").await;
    let bob = t.member("preview_bob").await;
    let carol = t.member("preview_carol").await;
    let general = t.general().await;
    let list = |token: String| {
        let t = &t;
        async move {
            t.req(Method::GET, "/channels", &token)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        }
    };
    let last = |body: &str, id: &str| {
        let channels: Vec<Channel> = serde_json::from_str(body).unwrap();
        channels
            .into_iter()
            .find(|c| c.id == id)
            .map(|c| c.last_message)
    };
    assert_eq!(last(&list(alice.token.clone()).await, &general), Some(None));

    let path = format!("/channels/{general}/messages");
    let root = t
        .post(
            &path,
            &alice.token,
            json!({"content":"who's driving saturday?"}),
        )
        .await;
    // A reply lives in its thread, so the room's preview stays on the root.
    t.post(
        &path,
        &bob.token,
        json!({"content":"me","reply_to":root["id"]}),
    )
    .await;
    let preview = last(&list(carol.token.clone()).await, &general)
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            preview.id.as_str(),
            preview.author_id.as_str(),
            preview.content.as_str()
        ),
        (
            root["id"].as_str().unwrap(),
            alice.user.id.as_str(),
            "who's driving saturday?"
        )
    );
    // Long text is cut by characters, not bytes.
    let long = "é".repeat(300);
    let newest = t.post(&path, &bob.token, json!({ "content": long })).await;
    let preview = last(&list(carol.token.clone()).await, &general)
        .unwrap()
        .unwrap();
    assert_eq!(preview.content, "é".repeat(200));
    // Deleting the newest message brings the one before it back.
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/messages/{}", newest["id"].as_str().unwrap()),
            &bob.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    let preview = last(&list(carol.token.clone()).await, &general)
        .unwrap()
        .unwrap();
    assert_eq!(preview.id, root["id"].as_str().unwrap());

    // A DM's newest message reaches its members and nobody else, admins included.
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let dm = dm["id"].as_str().unwrap();
    t.post(
        &format!("/channels/{dm}/messages"),
        &bob.token,
        json!({"content":"the surprise is a puppy"}),
    )
    .await;
    for member in [&alice, &bob] {
        let preview = last(&list(member.token.clone()).await, dm)
            .unwrap()
            .unwrap();
        assert_eq!(preview.content, "the surprise is a puppy");
    }
    for outsider in [carol.token.clone(), t.admin.token.clone()] {
        let body = list(outsider).await;
        assert_eq!(last(&body, dm), None);
        assert!(!body.contains("puppy"), "{body}");
    }
}

#[tokio::test]
async fn only_admins_set_and_clear_a_room_icon() {
    let t = Test::new().await;
    let alice = t.member("icon_alice").await;
    let general = t.general().await;
    let icon = |token: String, id: String, body: Value| {
        let t = &t;
        async move {
            t.req(Method::PUT, &format!("/channels/{id}/icon"), &token)
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };
    let listed = |token: String| {
        let t = &t;
        let general = general.clone();
        async move {
            let channels: Vec<Value> = t
                .req(Method::GET, "/channels", &token)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            channels
                .into_iter()
                .find(|c| c["id"] == general.as_str())
                .unwrap()["icon"]
                .clone()
        }
    };
    // Rooms from before icons existed read as no icon.
    assert_eq!(listed(alice.token.clone()).await, Value::Null);

    let a = t.admin.token.clone();
    let r = icon(a.clone(), general.clone(), json!({"icon":"flame"})).await;
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.json::<Channel>().await.unwrap().icon.as_deref(),
        Some("flame")
    );
    assert_eq!(listed(alice.token.clone()).await, "flame");

    assert_eq!(
        icon(
            alice.token.clone(),
            general.clone(),
            json!({"icon":"skull"})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        icon(alice.token.clone(), general.clone(), json!({"icon":null}))
            .await
            .status(),
        403
    );
    for bad in ["Flame", "fire!", "", "gamepad--2", "-x", &"a".repeat(65)] {
        assert_eq!(
            icon(a.clone(), general.clone(), json!({ "icon": bad }))
                .await
                .status(),
            400,
            "{bad}"
        );
    }
    let dm = t
        .post("/dms", &a, json!({"member_ids":[alice.user.id]}))
        .await;
    assert_eq!(
        icon(
            a.clone(),
            dm["id"].as_str().unwrap().into(),
            json!({"icon":"flame"})
        )
        .await
        .status(),
        404
    );
    // Renaming keeps the icon; only an explicit null clears it.
    assert_eq!(
        t.req(Method::PUT, &format!("/channels/{general}"), &a)
            .json(&json!({"name":"lobby","position":0}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(listed(alice.token.clone()).await, "flame");
    assert_eq!(
        icon(a.clone(), general.clone(), json!({"icon":"gamepad-2"}))
            .await
            .status(),
        200
    );
    assert_eq!(listed(alice.token.clone()).await, "gamepad-2");
    assert_eq!(
        icon(a.clone(), general.clone(), json!({"icon":null}))
            .await
            .status(),
        200
    );
    assert_eq!(listed(alice.token.clone()).await, Value::Null);
}
