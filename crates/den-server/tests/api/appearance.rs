use super::*;

#[tokio::test]
async fn split_appearance_migrates_validates_persists_and_stays_private() {
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
        light_theme: "paper".into(),
        dark_theme: theme.id.clone(),
        custom_themes: vec![theme.clone()],
        ..Appearance::default()
    });
    // An existing paired preference migrates both choices and preserves custom colors.
    sqlx::query("INSERT INTO user_appearance VALUES(?,?)")
        .bind(&alice.user.id)
        .bind(json!({"mode":"dark","theme":theme.id,"custom_themes":[theme]}).to_string())
        .execute(&t.state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_appearance VALUES(?,?)")
        .bind(&bob.user.id)
        .bind(json!({"mode":"system","custom_themes":[]}).to_string())
        .execute(&t.state.db)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../migrations/0013_split_theme_choice.sql"))
        .execute(&t.state.db)
        .await
        .unwrap();
    let migrated: Appearance = t
        .req(Method::GET, path, &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(migrated.light_theme, theme.id);
    assert_eq!(migrated.dark_theme, theme.id);
    assert_eq!(migrated.custom_themes, vec![theme]);
    assert_eq!(migrated.contrast, 100);
    assert_eq!(migrated.background, None);
    let mut sockets = vec![];
    for session in [&alice, &alice, &bob] {
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
    for socket in &mut sockets[..2] {
        tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(Ok(frame)) = socket.next().await {
                if let Frame::Text(text) = frame {
                    let event: Value = serde_json::from_str(&text).unwrap();
                    if event["type"] == "appearance_updated" {
                        assert_eq!(event["appearance"], value);
                        return;
                    }
                }
            }
            panic!("Owner WebSocket closed before appearance_updated");
        })
        .await
        .unwrap();
    }
    let leaked = tokio::time::timeout(Duration::from_millis(200), async {
        while let Some(Ok(frame)) = sockets[2].next().await {
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
        json!({"mode":"system","light_theme":"den-light","dark_theme":"den","custom_themes":[]}),
        json!({"mode":"dark","light_theme":"den","dark_theme":"missing","custom_themes":[]}),
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
