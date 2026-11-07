use super::*;

#[tokio::test]
async fn admins_rename_members_and_nobody_else_can() {
    let t = Test::new().await;
    let bob = t.member("rename_bob").await;
    let carol = t.member("rename_carol").await;
    let path = format!("/users/{}", bob.user.id);
    let patch = |token: &str, body: Value| t.req(Method::PATCH, &path, token).json(&body).send();

    assert_eq!(
        patch(&carol.token, json!({"display_name":"Bobby"}))
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        patch(&bob.token, json!({"username":"boss"}))
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        patch(&t.admin.token, json!({"username":"-bad"}))
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        patch(&t.admin.token, json!({"username":"rename_carol"}))
            .await
            .unwrap()
            .status(),
        409
    );

    // A display name that just echoed the username follows the new username.
    let renamed: User = patch(&t.admin.token, json!({"username":"robert"}))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        (renamed.username.as_str(), renamed.display_name.as_str()),
        ("robert", "robert")
    );
    let named: User = patch(&t.admin.token, json!({"display_name":" Bob "}))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        (named.username.as_str(), named.display_name.as_str()),
        ("robert", "Bob")
    );
    let login = t
        .req(Method::POST, "/auth/login", "")
        .json(&json!({"username":"robert","password":"test-password-123"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    assert_eq!(
        t.req(Method::PATCH, "/users/nobody", &t.admin.token)
            .json(&json!({"display_name":"x"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn a_removed_member_is_signed_out_everywhere_but_keeps_their_messages() {
    let t = Test::new().await;
    let bob = t.member("removed_bob").await;
    let carol = t.member("removed_carol").await;
    let general = t.general().await;
    t.post(
        &format!("/channels/{general}/messages"),
        &bob.token,
        json!({"content":"still here after I go"}),
    )
    .await;
    let token = t
        .post("/tokens", &bob.token, json!({"name":"script"}))
        .await;
    let path = format!("/users/{}", bob.user.id);

    assert_eq!(
        t.req(Method::DELETE, &path, &carol.token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/users/{}", t.admin.user.id),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    let removed: User = t
        .req(Method::DELETE, &path, &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(removed.removed);

    for credential in [bob.token.as_str(), token["token"].as_str().unwrap()] {
        assert_eq!(
            t.req(Method::GET, "/users/me", credential)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    assert_eq!(
        t.req(Method::POST, "/auth/login", "")
            .json(&json!({"username":"removed_bob","password":"test-password-123"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let messages: Vec<Message> = t
        .req(
            Method::GET,
            &format!("/channels/{general}/messages"),
            &carol.token,
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(messages.iter().any(|m| m.author_id == bob.user.id));
    let users: Vec<User> = t
        .req(Method::GET, "/users", &carol.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(users.iter().any(|u| u.id == bob.user.id && u.removed));
}

async fn bot_token(t: &Test, owner: &str, username: &str) -> String {
    let created: BotCreated = serde_json::from_value(
        t.post(
            "/bots",
            owner,
            json!({"username":username,"display_name":"Helper"}),
        )
        .await,
    )
    .unwrap();
    created.credential.token
}

// A bot's token sits in its owner's scripts. Removing the owner must not leave
// them a way back in through it; another member's bot is untouched.
#[tokio::test]
async fn removing_a_member_revokes_their_bots_tokens() {
    let t = Test::new().await;
    let bob = t.member("bot_owner_bob").await;
    let carol = t.member("bot_owner_carol").await;
    let general = t.general().await;
    let bobs_bot = bot_token(&t, &bob.token, "bobs_helper").await;
    let carols_bot = bot_token(&t, &carol.token, "carols_helper").await;
    let posts = |token: &str| {
        t.req(
            Method::POST,
            &format!("/channels/{general}/messages"),
            token,
        )
        .json(&json!({"content":"still posting"}))
        .send()
    };
    assert_eq!(posts(&bobs_bot).await.unwrap().status(), 200);

    let removed = t
        .req(
            Method::DELETE,
            &format!("/users/{}", bob.user.id),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), 200);

    for (method, path) in [
        (Method::GET, "/users/me"),
        (Method::GET, &format!("/channels/{general}/messages")),
        (Method::POST, "/auth/ws-ticket"),
    ] {
        assert_eq!(
            t.req(method, path, &bobs_bot)
                .send()
                .await
                .unwrap()
                .status(),
            401,
            "{path}"
        );
    }
    assert_eq!(posts(&bobs_bot).await.unwrap().status(), 401);
    assert_eq!(posts(&carols_bot).await.unwrap().status(), 200);
}
