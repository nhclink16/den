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
