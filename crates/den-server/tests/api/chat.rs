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
