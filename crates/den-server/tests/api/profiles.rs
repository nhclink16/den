use super::*;

#[tokio::test]
async fn profile_patch_validates_clears_expires_and_broadcasts_to_members() {
    let t = Test::new().await;
    let alice = t.member("profile_alice").await;
    let bob = t.member("profile_bob").await;
    let path = "/users/me/profile";
    assert_eq!(
        t.req(Method::PATCH, path, "")
            .json(&json!({"bio":"no"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let mut sockets = Vec::new();
    for session in [&alice, &bob] {
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
    assert!(
        connect_async(format!("{}/ws", t.url.replace("http:", "ws:")))
            .await
            .unwrap_err()
            .to_string()
            .contains("401")
    );
    let value = json!({"display_name":"Alice Updated","bio":"Plain *text*, not rendered Markdown","accent":"#Aa12bF","status":{"emoji":"👩🏽‍💻","text":"Writing","expires_at":4102444800_i64}});
    assert_eq!(
        t.http
            .patch(format!("{}{path}", t.url))
            .header("Cookie", format!("den_session={}", alice.token))
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response = t
        .req(Method::PATCH, path, &alice.token)
        .json(&value)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let user: User = response.json().await.unwrap();
    assert_eq!(user.display_name, "Alice Updated");
    assert_eq!(user.accent.as_deref(), Some("#aa12bf"));
    assert_eq!(user.bio.as_deref(), value["bio"].as_str());
    assert_eq!(user.status.as_ref().unwrap().emoji.as_deref(), Some("👩🏽‍💻"));
    for socket in &mut sockets {
        tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(Ok(frame)) = socket.next().await {
                if let Frame::Text(text) = frame {
                    if let Ok(Event::UserUpdated { user: updated }) = serde_json::from_str(&text) {
                        assert_eq!(updated, user);
                        return;
                    }
                }
            }
            panic!("Missing full user_updated event");
        })
        .await
        .unwrap();
    }
    for invalid in [
        json!({"bio":"界".repeat(191)}),
        json!({"accent":"#12345g"}),
        json!({"status":{"text":"界".repeat(61)}}),
        json!({"status":{"emoji":"😀😃"}}),
        json!({"status":{"emoji":"ab"}}),
        json!({"status":{"emoji":""}}),
        json!({"display_name":" "}),
    ] {
        assert_eq!(
            t.req(Method::PATCH, path, &alice.token)
                .json(&invalid)
                .send()
                .await
                .unwrap()
                .status(),
            400,
            "{invalid}"
        );
    }
    assert_eq!(
        t.req(Method::PATCH, path, &alice.token)
            .json(&json!({"pronouns":"removed"}))
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    let boundary = json!({"bio":"界".repeat(190),"status":{"emoji":"🇨🇦","text":"界".repeat(60)}});
    let response = t
        .req(Method::PATCH, path, &alice.token)
        .json(&boundary)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bounded: User = response.json().await.unwrap();
    assert_eq!(bounded.bio.unwrap().chars().count(), 190);
    assert_eq!(bounded.display_name, user.display_name);
    assert_eq!(bounded.accent, user.accent);
    let response = t
        .req(Method::PATCH, path, &alice.token)
        .json(&json!({"bio":null,"accent":null,"status":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cleared: User = response.json().await.unwrap();
    assert_eq!(cleared.bio, None);
    assert_eq!(cleared.accent, None);
    assert_eq!(cleared.status, None);
    assert_eq!(cleared.display_name, user.display_name);
    let expired = json!({"emoji":"💤","text":"Expired","expires_at":1});
    sqlx::query("UPDATE users SET status=? WHERE id=?")
        .bind(expired.to_string())
        .bind(&alice.user.id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let listed: Vec<User> = t
        .req(Method::GET, "/users", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|u| u.id == alice.user.id)
            .unwrap()
            .status,
        None
    );
    let stored: Option<String> = sqlx::query_scalar("SELECT status FROM users WHERE id=?")
        .bind(&alice.user.id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert!(stored.is_none());
    let response = t
        .req(Method::PATCH, path, &alice.token)
        .json(&json!({"status":expired}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.json::<User>().await.unwrap().status, None);
    let me: User = t
        .req(Method::GET, "/users/me", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me.display_name, user.display_name);
    assert_eq!(me.status, None);
}
