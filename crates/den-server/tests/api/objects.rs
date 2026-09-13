use super::*;

#[tokio::test]
async fn objects_enforce_access_merge_records_and_cascade() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let channel = dm["id"].as_str().unwrap();
    let o = t
        .post(
            &format!("/channels/{channel}/objects"),
            &alice.token,
            json!({"kind":"canvas","name":"raid","state":{"page:page":{"id":"page:page"}}}),
        )
        .await;
    let id = o["id"].as_str().unwrap();
    let path = format!("/objects/{id}");
    for (method, suffix, body) in [
        (Method::GET, "", json!({})),
        (Method::GET, "/summary", json!({})),
        (Method::POST, "/patch", json!({"base_version":0,"put":[]})),
        (Method::PATCH, "", json!({"name":"stolen"})),
    ] {
        assert_eq!(
            t.req(method, &format!("{path}{suffix}"), &t.admin.token)
                .json(&body)
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
            &format!("/channels/{channel}/objects"),
            &t.admin.token
        )
        .json(&json!({"kind":"canvas","name":"no"}))
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
    let patch = format!("{path}/patch");
    assert_eq!(
        t.post(
            &patch,
            &alice.token,
            json!({"base_version":0,"put":[{"id":"shape:a","x":1},{"id":"shape:b","x":2}]})
        )
        .await["version"],
        1
    );
    assert_eq!(
        t.post(
            &patch,
            &bob.token,
            json!({"base_version":0,"put":[{"id":"shape:a","x":3}],"remove":["shape:b"]})
        )
        .await["version"],
        2
    );
    let got: Object = t
        .req(Method::GET, &path, &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got.summary.version, 2);
    assert_eq!(got.state.len(), 2);
    assert_eq!(got.state["shape:a"]["x"], 3);
    let msg: Message = t
        .req(
            Method::GET,
            &format!("/messages/{}", o["message_id"].as_str().unwrap()),
            &bob.token,
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(msg.objects[0].id, id);
    assert!(msg.content.is_empty());
    assert_eq!(
        t.req(Method::POST, &patch, &bob.token)
            .json(&json!({"base_version":2,"put":[{"bad":true}]}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let large = "x".repeat(1024 * 1024);
    assert_eq!(
        t.req(Method::POST, &patch, &bob.token)
            .json(&json!({"base_version":2,"put":[{"id":"big","value":large}]}))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    // Full-state cap also applies when many individually legal patches accumulate.
    for i in 0..9 {
        let response = t.req(Method::POST,&patch,&bob.token).json(&json!({"base_version":0,"put":[{"id":format!("large:{i}"),"value":"x".repeat(1024*1024-200)}]})).send().await.unwrap();
        assert_eq!(response.status(), if i < 8 { 200 } else { 413 });
    }
    let renamed: ObjectSummary = t
        .req(Method::PATCH, &path, &bob.token)
        .json(&json!({"name":"new name"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(renamed.name, "new name");
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/messages/{}", msg.id),
            &alice.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    assert_eq!(
        t.req(Method::GET, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn only_admin_changes_canvas_switch_and_existing_objects_stay_readable() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let settings: Settings = t
        .req(Method::GET, "/settings", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(settings.canvas_enabled);
    let o = t
        .post(
            &format!("/channels/{cid}/objects"),
            &bob.token,
            json!({"kind":"canvas","name":""}),
        )
        .await;
    assert_eq!(o["name"], "Untitled canvas");
    assert_eq!(
        t.req(Method::PUT, "/settings", &bob.token)
            .json(&json!({"canvas_enabled":false}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::PUT, "/settings", &t.admin.token)
            .json(&json!({"canvas_enabled":false}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let r = t
        .req(
            Method::POST,
            &format!("/channels/{cid}/objects"),
            &bob.token,
        )
        .json(&json!({"kind":"canvas","name":"no"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "canvas_disabled");
    assert_eq!(
        t.req(
            Method::GET,
            &format!("/objects/{}", o["id"].as_str().unwrap()),
            &bob.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        t.req(Method::PUT, "/settings", &t.admin.token)
            .json(&json!({"canvas_enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    t.post(
        &format!("/channels/{cid}/objects"),
        &bob.token,
        json!({"kind":"canvas","name":"back"}),
    )
    .await;
}

#[tokio::test]
async fn instance_name_is_public_bounded_and_admin_only() {
    let t = Test::new().await;
    let bob = t.member("name_bob").await;
    let get = || t.req(Method::GET, "/settings", "");
    let initial: Settings = get().send().await.unwrap().json().await.unwrap();
    assert_eq!(initial.instance_name, "Den");
    for (token, status) in [("", 401), (bob.token.as_str(), 403)] {
        assert_eq!(
            t.req(Method::PUT, "/settings", token)
                .json(&json!({"instance_name":"Changed"}))
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    for name in ["".to_owned(), "   ".into(), "x".repeat(41), "a\nb".into()] {
        assert_eq!(
            t.req(Method::PUT, "/settings", &t.admin.token)
                .json(&json!({"instance_name":name}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    let name = "🌲".repeat(40);
    let changed: Settings = t
        .req(Method::PUT, "/settings", &t.admin.token)
        .json(&json!({"instance_name":format!("  {name}  ")}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(changed.instance_name, name);
    assert!(changed.canvas_enabled);
    let legacy: Settings = t
        .req(Method::PUT, "/settings", &t.admin.token)
        .json(&json!({"canvas_enabled":false}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(legacy.instance_name, name);
    assert!(!legacy.canvas_enabled);
    let persisted: Settings = get().send().await.unwrap().json().await.unwrap();
    assert_eq!(persisted, legacy);
}
