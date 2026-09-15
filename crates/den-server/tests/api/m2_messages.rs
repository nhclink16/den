use super::*;

async fn get<T: serde::de::DeserializeOwned>(t: &Test, path: &str, token: &str) -> T {
    let r = t.req(Method::GET, path, token).send().await.unwrap();
    assert_eq!(r.status(), 200, "{path}");
    r.json().await.unwrap()
}
async fn put(t: &Test, path: &str, token: &str, body: Value) -> Value {
    let r = t
        .req(Method::PUT, path, token)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{path}");
    r.json().await.unwrap()
}
#[tokio::test]
async fn replies_reactions_and_search_respect_membership_and_message_changes() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let public = t.general().await;
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let cid = dm["id"].as_str().unwrap();
    let path = format!("/channels/{cid}/messages");
    let parent = t
        .post(&path, &alice.token, json!({"content":"secret otter café"}))
        .await;
    let mid = parent["id"].as_str().unwrap();
    let reply = t
        .post(
            &path,
            &bob.token,
            json!({"content":"reply otter","reply_to":mid}),
        )
        .await;
    let fetched: Message = get(&t, &format!("/messages/{mid}"), &bob.token).await;
    assert_eq!(fetched.content, "secret otter café");
    assert_eq!(reply["reply_to"], mid);
    assert_eq!(
        t.req(Method::GET, &format!("/messages/{mid}"), &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let reaction = format!("/messages/{mid}/reactions");
    for _ in 0..2 {
        let result = put(&t, &reaction, &bob.token, json!({"emoji":"👍🏽"})).await;
        assert_eq!(result[0]["user_ids"].as_array().unwrap().len(), 1);
    }
    let result = put(&t, &reaction, &alice.token, json!({"emoji":"👍🏽"})).await;
    assert_eq!(result[0]["user_ids"].as_array().unwrap().len(), 2);
    assert_eq!(
        t.req(Method::PUT, &reaction, &t.admin.token)
            .json(&json!({"emoji":"👍🏽"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let result: Vec<Reaction> = t
        .req(Method::DELETE, &reaction, &bob.token)
        .json(&json!({"emoji":"👍🏽"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result[0].user_ids, vec![alice.user.id.clone()]);
    t.post(
        &format!("/channels/{public}/messages"),
        &t.admin.token,
        json!({"content":"public otter"}),
    )
    .await;
    let outside: Vec<Message> = get(&t, "/search/messages?q=otter", &t.admin.token).await;
    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].channel_id, public);
    let inside: Vec<Message> = get(
        &t,
        &format!("/search/messages?q=otter&channel_id={cid}&limit=1"),
        &bob.token,
    )
    .await;
    assert_eq!(inside[0].id, reply["id"]);
    let older: Vec<Message> = get(
        &t,
        &format!(
            "/search/messages?q=otter&channel_id={cid}&before={}",
            inside[0].id
        ),
        &bob.token,
    )
    .await;
    assert_eq!(older[0].id, mid);
    assert_eq!(
        t.req(
            Method::GET,
            &format!("/search/messages?q=otter&channel_id={cid}"),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
    let literal: Vec<Message> = get(
        &t,
        "/search/messages?q=otter%20OR%20nonexistent",
        &bob.token,
    )
    .await;
    assert!(literal.is_empty());
    assert_eq!(
        t.req(Method::PATCH, &format!("/messages/{mid}"), &alice.token)
            .json(&json!({"content":"renamed animal"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let old: Vec<Message> = get(
        &t,
        &format!("/search/messages?q=cafe&channel_id={cid}"),
        &bob.token,
    )
    .await;
    assert!(old.is_empty());
    assert_eq!(
        t.req(Method::DELETE, &format!("/messages/{mid}"), &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let reply: Message = get(
        &t,
        &format!("/messages/{}", reply["id"].as_str().unwrap()),
        &bob.token,
    )
    .await;
    assert!(reply.reply_to.is_none());
    let deleted: Vec<Message> = get(
        &t,
        &format!("/search/messages?q=renamed&channel_id={cid}"),
        &bob.token,
    )
    .await;
    assert!(deleted.is_empty());
}
#[tokio::test]
async fn read_markers_are_monotonic_and_notification_counts_follow_preferences() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let a = &t.admin.token;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    let prefs: NotificationPreferences =
        get(&t, "/users/me/notification-preferences", &bob.token).await;
    assert_eq!(prefs, NotificationPreferences::default());
    let first = t.post(&path, a, json!({"content":"ordinary"})).await;
    let mention = t
        .post(
            &path,
            a,
            json!({"content":"hello @bob twice @bob; address a@bob, `@bob`"}),
        )
        .await;
    assert_eq!(mention["mention_ids"], json!([bob.user.id]));
    t.post(
        &path,
        a,
        json!({"content":"email a@bob and code `@bob`\n```\n@bob\n```"}),
    )
    .await;
    t.post(&path, &bob.token, json!({"content":"my own post"}))
        .await;
    let states: Vec<ChannelReadState> = get(&t, "/users/me/read-state", &bob.token).await;
    let state = &states[0];
    assert_eq!(
        (
            state.unread_count,
            state.mention_count,
            state.notification_count
        ),
        (3, 1, 1)
    );
    let pref_path = "/users/me/notification-preferences";
    put(
        &t,
        pref_path,
        &bob.token,
        json!({"mentions":true,"dms":true,"subscribed_channel_ids":[cid]}),
    )
    .await;
    let states: Vec<ChannelReadState> = get(&t, "/users/me/read-state", &bob.token).await;
    assert_eq!(states[0].notification_count, 3);
    let read_path = format!("/channels/{cid}/read");
    let marker = put(
        &t,
        &read_path,
        &bob.token,
        json!({"message_id":mention["id"]}),
    )
    .await;
    assert_eq!(marker["unread_count"], 1);
    let older = put(
        &t,
        &read_path,
        &bob.token,
        json!({"message_id":first["id"]}),
    )
    .await;
    assert_eq!(older["last_read_id"], mention["id"]);
    t.req(
        Method::DELETE,
        &format!("/messages/{}", mention["id"].as_str().unwrap()),
        a,
    )
    .send()
    .await
    .unwrap();
    let after: Vec<ChannelReadState> = get(&t, "/users/me/read-state", &bob.token).await;
    assert_eq!(after[0].last_read_id.as_deref(), mention["id"].as_str());
    put(
        &t,
        pref_path,
        &bob.token,
        json!({"mentions":false,"dms":false,"subscribed_channel_ids":[]}),
    )
    .await;
    let muted: Vec<ChannelReadState> = get(&t, "/users/me/read-state", &bob.token).await;
    assert_eq!(muted[0].unread_count, 1);
    assert_eq!(muted[0].notification_count, 0);
    let alice = t.member("alice").await;
    let dm = t
        .post("/dms", a, json!({"member_ids":[alice.user.id]}))
        .await;
    let private = dm["id"].as_str().unwrap();
    assert_eq!(
        t.req(
            Method::PUT,
            &format!("/channels/{private}/read"),
            &bob.token
        )
        .json(&json!({"message_id":first["id"]}))
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
    assert_eq!(
        t.req(Method::PUT, pref_path, &bob.token)
            .json(&json!({"mentions":true,"dms":true,"subscribed_channel_ids":[private]}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let states: Vec<ChannelReadState> = get(&t, "/users/me/read-state", &bob.token).await;
    assert!(states.iter().all(|s| s.channel_id != private));
}

#[tokio::test]
async fn mentions_keep_dots_inside_names_and_drop_punctuation_after_them() {
    let t = Test::new().await;
    let a = &t.admin.token;
    let andy = t.member("an.dy").await;
    let path = format!("/channels/{}/messages", t.general().await);

    // The dot inside the name belongs to it. The one ending the sentence does not.
    let sentence = t
        .post(&path, a, json!({"content":"ping @an.dy. thanks"}))
        .await;
    assert_eq!(sentence["mention_ids"], json!([andy.user.id]));

    // A capitalised spelling still finds the same person.
    let shouted = t.post(&path, a, json!({"content":"@An.Dy again"})).await;
    assert_eq!(shouted["mention_ids"], json!([andy.user.id]));
}
