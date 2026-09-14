use super::*;

#[tokio::test]
async fn appearance_validates_persists_and_stays_private() {
    let t = Test::new().await;
    let alice = t.member("appearance_alice").await;
    let bob = t.member("appearance_bob").await;
    let path = "/users/me/appearance";
    assert_eq!(
        t.req(Method::GET, path, "").send().await.unwrap().status(),
        401
    );
    let initial: Value = t
        .req(Method::GET, path, &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(initial, json!(Appearance::default()));
    let mut theme = builtin_themes()[3].clone();
    theme.id = "custom-tide".into();
    theme.name = "My tide".into();
    theme.dark.accent = "#abcdef".into();
    let mut value = json!(Appearance {
        mode: AppearanceMode::Dark,
        theme: theme.id.clone(),
        custom_themes: vec![theme],
        ..Appearance::default()
    });
    let mut sockets = vec![];
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
    assert_eq!(
        t.http
            .put(format!("{}{path}", t.url))
            .header("Cookie", format!("den_session={}", alice.token))
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(Ok(frame)) = sockets[0].next().await {
            if let Frame::Text(text) = frame {
                let event: Value = serde_json::from_str(&text).unwrap();
                if event["type"] == "appearance_updated" {
                    assert_eq!(event["appearance"], value);
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    let leaked = tokio::time::timeout(Duration::from_millis(200), async {
        while let Some(Ok(frame)) = sockets[1].next().await {
            if let Frame::Text(text) = frame {
                if serde_json::from_str::<Value>(&text).unwrap()["type"] == "appearance_updated" {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(!leaked);
    let stored: Value = t
        .req(Method::GET, path, &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stored, value);
    let other: Value = t
        .req(Method::GET, path, &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(other, initial);
    let good = value.clone();
    for (field, bad) in [
        ("dark", json!({"accent":"red"})),
        (
            "fonts",
            json!({"display":"x; background:red","body":"Inter","mono":"Inter"}),
        ),
        ("name", json!("x".repeat(41))),
        ("radius", json!("huge")),
    ] {
        value = good.clone();
        value["custom_themes"][0][field] = bad;
        assert_eq!(
            t.req(Method::PUT, path, &alice.token)
                .json(&value)
                .send()
                .await
                .unwrap()
                .status(),
            if matches!(field, "dark" | "radius") {
                422
            } else {
                400
            },
            "{field}"
        );
    }
    for bad in [
        json!({"mode":"system","theme":"den-light","custom_themes":[]}),
        json!({"mode":"dark","theme":"missing","custom_themes":[]}),
    ] {
        assert_eq!(
            t.req(Method::PUT, path, &alice.token)
                .json(&bad)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    value = good.clone();
    value["custom_themes"] = json!(vec![good["custom_themes"][0].clone(); 13]);
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    value = good.clone();
    value["custom_themes"][0]["extra"] = json!(true);
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    let persisted: String =
        sqlx::query_scalar("SELECT appearance FROM user_appearance WHERE user_id=?")
            .bind(&alice.user.id)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&persisted).unwrap(), good);
}
