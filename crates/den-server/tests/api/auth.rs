use super::*;

#[tokio::test]
async fn auth_consumes_invites_protects_cookies_and_revokes_credentials() {
    let t = Test::new().await;
    let a = &t.admin.token;
    assert!(!t.dir.join("bootstrap.key").exists());
    assert_eq!(
        t.http
            .get(format!("{}/users", t.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let invite = t
        .post("/invites", a, json!({"uses":1,"expires_in_hours":1}))
        .await;
    let body = json!({"username":"alice","password":"test-password-123","invite":invite["code"]});
    let alice: Session = serde_json::from_value(t.post("/auth/register", "", body).await).unwrap();
    let again = t
        .req(Method::POST, "/auth/register", "")
        .json(&json!({"username":"carol","password":"test-password-123","invite":invite["code"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 400);
    assert_eq!(
        t.req(Method::POST, "/invites", &alice.token)
            .json(&json!({"uses":1,"expires_in_hours":1}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::POST, "/auth/login", "")
            .json(&json!({"username":"alice","password":"wrong"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let login = t
        .req(Method::POST, "/auth/login", "")
        .json(&json!({"username":"alice","password":"test-password-123"}))
        .send()
        .await
        .unwrap();
    assert!(login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .contains("HttpOnly; SameSite=Strict"));
    let login: Session = login.json().await.unwrap();
    let cookie = format!("den_session={}", login.token);
    let write = || {
        t.http
            .post(format!("{}/tokens", t.url))
            .header("Cookie", &cookie)
            .json(&CreateToken {
                name: "cookie".into(),
                user_id: None,
            })
    };
    assert_eq!(write().send().await.unwrap().status(), 403);
    assert_eq!(
        write()
            .header("Origin", "https://evil.example")
            .header("X-CSRF-Token", &login.csrf_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        write()
            .header("Origin", &t.url)
            .header("X-CSRF-Token", &login.csrf_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let token = t
        .post("/tokens", &alice.token, json!({"name":"agent"}))
        .await;
    let secret = token["token"].as_str().unwrap();
    assert_eq!(
        t.req(Method::GET, "/users/me", secret)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/tokens/{}", token["credential"]["id"].as_str().unwrap()),
            &alice.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    assert_eq!(
        t.req(Method::GET, "/users/me", secret)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        t.req(Method::POST, "/auth/logout", &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        t.req(Method::GET, "/users/me", &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let hashes: Vec<String> =
        sqlx::query_scalar("SELECT secret_hash FROM sessions UNION SELECT secret_hash FROM tokens")
            .fetch_all(&t.state.db)
            .await
            .unwrap();
    assert!(hashes
        .iter()
        .all(|h| h != a && h != &login.token && h != secret));
}
#[tokio::test]
async fn login_throttles_and_openapi_contains_the_shared_contract() {
    let t = Test::new().await;
    // Invalid bootstrap attempts consume the same bounded throttle without hashing passwords.
    for _ in 0..9 {
        assert_eq!(t.req(Method::POST,"/auth/init","").json(&json!({"username":"other","password":"test-password-123","bootstrap_token":"wrong"})).send().await.unwrap().status(),409);
    }
    assert_eq!(
        t.req(Method::POST, "/auth/login", "")
            .json(&json!({"username":"admin","password":"test-password-123"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    let spec: Value = t
        .http
        .get(format!("{}/openapi.json", t.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(spec["components"]["schemas"]["Event"].is_object());
    assert!(spec["paths"]["/uploads/{id}"]["patch"].is_object());
    assert!(spec["paths"]["/channels/{id}/messages"]["post"]["requestBody"].is_object());
}

async fn join(t: &Test, code: &str, username: &str, display: Option<&str>) -> reqwest::Response {
    let mut body = json!({"username":username,"password":"test-password-123","invite":code});
    if let Some(display) = display {
        body["display_name"] = json!(display);
    }
    t.req(Method::POST, "/auth/register", "")
        .json(&body)
        .send()
        .await
        .unwrap()
}

// The charset itself is a pure function with its own unit test in auth.rs. This
// covers the parts that only show up end to end, and stays under the ten attempts
// a minute the rate limiter allows one address.
#[tokio::test]
async fn usernames_keep_their_case_and_display_names_are_free_text() {
    let t = Test::new().await;
    let invite = t
        .post(
            "/invites",
            &t.admin.token,
            json!({"uses":4,"expires_in_hours":1}),
        )
        .await;
    let code = invite["code"].as_str().unwrap().to_string();

    let andy = join(&t, &code, "Andy", Some("Andy 🎧")).await;
    assert_eq!(andy.status(), 200);
    let andy: Session = andy.json().await.unwrap();
    assert_eq!(andy.user.username, "Andy");
    assert_eq!(andy.user.display_name, "Andy 🎧");

    // Case cannot be used to impersonate: the column collates NOCASE, so the
    // lowercase spelling is the same account and registering it again conflicts.
    assert_eq!(join(&t, &code, "andy", None).await.status(), 409);
    assert_eq!(
        t.req(Method::POST, "/auth/login", "")
            .json(&json!({"username":"ANDY","password":"test-password-123"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    // A blank display name means the username stands in for one.
    let plain: Session = join(&t, &code, "bo_bby", Some("   "))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(plain.user.display_name, "bo_bby");
}

async fn ticket_socket(t: &Test, token: &str) -> bool {
    let ticket: WsTicket =
        serde_json::from_value(t.post("/auth/ws-ticket", token, json!({})).await).unwrap();
    connect_async(format!(
        "{}/ws?ticket={}",
        t.url.replace("http:", "ws:"),
        ticket.ticket
    ))
    .await
    .is_ok()
}

// Sessions last 90 days. Past that, the same secret must stop working as a
// bearer, as a cookie, for a ticket already handed out, and on a socket that
// was opened while it was still good.
#[tokio::test]
async fn an_expired_session_is_refused_everywhere_it_was_accepted() {
    let t = Test::new().await;
    let alice = t.member("expiring_alice").await;
    assert!(ticket_socket(&t, &alice.token).await);
    let ticket: WsTicket =
        serde_json::from_value(t.post("/auth/ws-ticket", &alice.token, json!({})).await).unwrap();
    let mut open = super::realtime::socket(&t, &alice.token).await;

    sqlx::query("UPDATE sessions SET expires_at=0 WHERE user_id=?")
        .bind(&alice.user.id)
        .execute(&t.state.db)
        .await
        .unwrap();

    for (method, path) in [
        (Method::GET, "/users/me"),
        (Method::GET, "/channels"),
        (Method::POST, "/auth/ws-ticket"),
    ] {
        assert_eq!(
            t.req(method, path, &alice.token)
                .send()
                .await
                .unwrap()
                .status(),
            401,
            "{path}"
        );
    }
    assert_eq!(
        t.http
            .get(format!("{}/users/me", t.url))
            .header("Cookie", format!("den_session={}", alice.token))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert!(connect_async(format!(
        "{}/ws?ticket={}",
        t.url.replace("http:", "ws:"),
        ticket.ticket
    ))
    .await
    .is_err());
    // The open socket rechecks its credential twice a second and hangs up.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match open.next().await {
                None | Some(Ok(Frame::Close(_))) | Some(Err(_)) => break,
                Some(Ok(Frame::Ping(v))) => {
                    let _ = open.send(Frame::Pong(v)).await;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("socket stayed open on an expired session");
}

// Tokens and bots belong to one person. Another member can neither see, revoke
// nor mint someone else's, and a bot is a member even when an admin owns it.
#[tokio::test]
async fn tokens_stay_with_their_owner_and_bots_never_act_as_admins() {
    let t = Test::new().await;
    let alice = t.member("token_alice").await;
    let bob = t.member("token_bob").await;
    let token = t
        .post("/tokens", &alice.token, json!({"name":"alice script"}))
        .await;
    let secret = token["token"].as_str().unwrap();
    let id = token["credential"]["id"].as_str().unwrap();

    let listed: Vec<Token> = t
        .req(Method::GET, "/tokens", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(listed
        .iter()
        .all(|t| t.id != id && t.user_id != alice.user.id));
    assert_eq!(
        t.req(Method::DELETE, &format!("/tokens/{id}"), &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    // Only bots can have tokens minted for them, and only by their owner.
    for target in [&alice.user.id, &t.admin.user.id] {
        assert_eq!(
            t.req(Method::POST, "/tokens", &bob.token)
                .json(&json!({"name":"steal","user_id":target}))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        t.req(Method::GET, "/users/me", secret)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    let bot: BotCreated = serde_json::from_value(
        t.post(
            "/bots",
            &t.admin.token,
            json!({"username":"admin_helper","display_name":"Helper"}),
        )
        .await,
    )
    .unwrap();
    assert_eq!(bot.user.role, Role::Member);
    let b = bot.credential.token.as_str();
    for (method, path, body) in [
        (
            Method::POST,
            "/invites",
            json!({"uses":1,"expires_in_hours":1}),
        ),
        (
            Method::POST,
            "/channels",
            json!({"name":"bots","position":0}),
        ),
        (
            Method::POST,
            "/bots",
            json!({"username":"grandchild","display_name":"Child"}),
        ),
        (Method::PUT, "/settings", json!({"instance_name":"Taken"})),
        (
            Method::PATCH,
            &format!("/users/{}", bob.user.id),
            json!({"display_name":"Renamed"}),
        ),
        (
            Method::DELETE,
            &format!("/users/{}", bob.user.id),
            json!({}),
        ),
    ] {
        assert_eq!(
            t.req(method, path, b)
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            403,
            "{path}"
        );
    }
    // A bot cannot sign in with a password, so a token is its only way in.
    assert_eq!(
        t.req(Method::POST, "/auth/login", "")
            .json(&json!({"username":"admin_helper","password":"test-password-123"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}

// Removal deletes credentials, but a sign-in or token request already past its
// checks can still write one a moment later. Such a credential must not work:
// a removed member's own, or one for a bot they own.
#[tokio::test]
async fn a_credential_written_after_removal_does_not_sign_anyone_in() {
    use sha2::{Digest, Sha256};
    let t = Test::new().await;
    let bob = t.member("late_bob").await;
    let bot: BotCreated = serde_json::from_value(
        t.post(
            "/bots",
            &bob.token,
            json!({"username":"late_bot","display_name":"Late"}),
        )
        .await,
    )
    .unwrap();
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/users/{}", bob.user.id),
            &t.admin.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    let digest = |secret: &str| format!("{:x}", Sha256::digest(secret.as_bytes()));
    sqlx::query("INSERT INTO sessions(id,user_id,secret_hash,csrf_hash,expires_at) VALUES('late-session',?,?,?,?)")
        .bind(&bob.user.id)
        .bind(digest("late-session-secret"))
        .bind(digest("late-csrf"))
        .bind(i64::MAX)
        .execute(&t.state.db)
        .await
        .unwrap();
    for (id, user, secret) in [
        ("late-token", &bob.user.id, "late-token-secret"),
        ("late-bot-token", &bot.user.id, "late-bot-secret"),
    ] {
        sqlx::query("INSERT INTO tokens(id,user_id,name,secret_hash) VALUES(?,?,'late',?)")
            .bind(id)
            .bind(user)
            .bind(digest(secret))
            .execute(&t.state.db)
            .await
            .unwrap();
    }
    for secret in [
        "late-session-secret",
        "late-token-secret",
        "late-bot-secret",
    ] {
        for (method, path) in [
            (Method::GET, "/users/me"),
            (Method::POST, "/auth/ws-ticket"),
        ] {
            assert_eq!(
                t.req(method, path, secret).send().await.unwrap().status(),
                401,
                "{path}"
            );
        }
    }
    assert_eq!(
        t.http
            .get(format!("{}/users/me", t.url))
            .header("Cookie", "den_session=late-session-secret")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}
