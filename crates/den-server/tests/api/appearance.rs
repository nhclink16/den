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
    // Bob saved something before Neutral existed, so he keeps Den.
    let mut expected = initial.clone();
    expected["light_theme"] = json!("den");
    expected["dark_theme"] = json!("den");
    assert_eq!(other, expected);
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

#[tokio::test]
async fn neutral_is_only_the_default_and_the_accent_survives_older_clients() {
    let t = Test::new().await;
    let fresh = t.member("appearance_fresh").await;
    let chose = t.member("appearance_chose").await;
    let path = "/users/me/appearance";
    let get = |token: String| {
        let t = &t;
        async move {
            t.req(Method::GET, path, &token)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let put = |token: String, body: Value| {
        let t = &t;
        async move {
            t.req(Method::PUT, path, &token)
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };
    // Nothing saved: the new default. A row written before the accent existed
    // still reads, and keeps the theme it names.
    let fresh_value = get(fresh.token.clone()).await;
    assert_eq!(fresh_value["light_theme"], "neutral");
    assert_eq!(fresh_value["dark_theme"], "neutral");
    assert_eq!(fresh_value["accent"], Value::Null);
    sqlx::query("INSERT INTO user_appearance VALUES(?,?)")
        .bind(&chose.user.id)
        .bind(
            json!({"mode":"dark","light_theme":"den","dark_theme":"den","custom_themes":[],"contrast":100,"background":null})
                .to_string(),
        )
        .execute(&t.state.db)
        .await
        .unwrap();
    let old = get(chose.token.clone()).await;
    assert_eq!(old["dark_theme"], "den");
    assert_eq!(old["accent"], Value::Null);
    assert_eq!(old["room_icons"], "icons");
    assert_eq!(old["bubble_shape"], "corners");
    assert_eq!(old["bubble_colors"], "everyone");
    assert_eq!(old["avatar_shape"], "theme");
    assert_eq!(old["presence_style"], "dot");

    let mut body = old.clone();
    body["accent"] = json!("green");
    body["room_icons"] = json!("hash");
    body["bubble_shape"] = json!("none");
    body["bubble_colors"] = json!("plain");
    body["avatar_shape"] = json!("round");
    body["presence_style"] = json!("ring");
    assert_eq!(put(chose.token.clone(), body.clone()).await.status(), 200);
    // An app that predates the accent saves without it: the choice stays.
    let mut older = body.clone();
    older.as_object_mut().unwrap().remove("accent");
    older.as_object_mut().unwrap().remove("room_icons");
    older.as_object_mut().unwrap().remove("bubble_shape");
    older.as_object_mut().unwrap().remove("bubble_colors");
    older.as_object_mut().unwrap().remove("avatar_shape");
    older.as_object_mut().unwrap().remove("presence_style");
    older["mode"] = json!("light");
    assert_eq!(put(chose.token.clone(), older).await.status(), 200);
    let kept = get(chose.token.clone()).await;
    assert_eq!(
        (
            kept["accent"].clone(),
            kept["mode"].clone(),
            kept["room_icons"].clone(),
            kept["bubble_shape"].clone(),
            kept["bubble_colors"].clone(),
            kept["avatar_shape"].clone(),
            kept["presence_style"].clone()
        ),
        (
            json!("green"),
            json!("light"),
            json!("hash"),
            json!("none"),
            json!("plain"),
            json!("round"),
            json!("ring")
        )
    );
    // A newer app that sends them changes them.
    body["bubble_shape"] = json!("sharp");
    body["bubble_colors"] = json!("mine");
    body["avatar_shape"] = json!("theme");
    body["presence_style"] = json!("off");
    assert_eq!(put(chose.token.clone(), body.clone()).await.status(), 200);
    let changed = get(chose.token.clone()).await;
    assert_eq!(
        (
            changed["bubble_shape"].clone(),
            changed["bubble_colors"].clone(),
            changed["avatar_shape"].clone(),
            changed["presence_style"].clone()
        ),
        (json!("sharp"), json!("mine"), json!("theme"), json!("off"))
    );
    for (field, junk) in [
        ("room_icons", "squares"),
        ("bubble_shape", "blob"),
        ("bubble_colors", "rainbow"),
        ("avatar_shape", "hexagon"),
        ("presence_style", "glow"),
    ] {
        let mut bad = body.clone();
        bad[field] = json!(junk);
        assert_eq!(put(chose.token.clone(), bad).await.status(), 422, "{field}");
    }
    body["room_icons"] = json!("none");
    for (accent, status) in [
        (json!("#12ab34"), 200),
        (json!(null), 200),
        (json!("purple"), 400),
        (json!("#12ab3"), 400),
        (json!("12ab34f"), 400),
    ] {
        body["accent"] = accent.clone();
        assert_eq!(
            put(chose.token.clone(), body.clone()).await.status(),
            status,
            "{accent}"
        );
    }
    assert_eq!(get(chose.token.clone()).await["accent"], Value::Null);

    // A custom theme someone had already called "neutral" keeps its colours and
    // stays chosen, under an ID that no longer clashes, and saves again.
    let mut mine = builtin_themes()[1].clone();
    mine.id = "neutral".into();
    mine.name = "My neutral".into();
    sqlx::query("UPDATE user_appearance SET appearance=? WHERE user_id=?")
        .bind(
            json!({"mode":"dark","light_theme":"den","dark_theme":"neutral","custom_themes":[mine],"contrast":100,"background":null})
                .to_string(),
        )
        .bind(&chose.user.id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let renamed = get(chose.token.clone()).await;
    assert_eq!(renamed["dark_theme"], "neutral 1");
    assert_eq!(renamed["custom_themes"][0]["id"], "neutral 1");
    assert_eq!(renamed["custom_themes"][0]["name"], "My neutral");
    assert_eq!(put(chose.token.clone(), renamed).await.status(), 200);
}

#[tokio::test]
async fn a_save_keeps_every_field_it_does_not_name() {
    // Two devices saving at once each send only what they changed, so neither
    // writes its older copy of the other's setting back.
    let t = Test::new().await;
    let ann = t.member("appearance_partial").await;
    let path = "/users/me/appearance";
    let put = |body: Value| {
        let t = &t;
        let token = ann.token.clone();
        async move {
            t.req(Method::PUT, path, &token)
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };
    let mut theme = builtin_themes()[1].clone();
    theme.id = "plum".into();
    theme.name = "Plum".into();
    let res = put(json!({"custom_themes":[theme],"dark_theme":"plum"})).await;
    assert_eq!(res.status(), 200);
    assert_eq!(put(json!({"mode":"light"})).await.status(), 200);
    let res = put(json!({"accent":"green","contrast":110})).await;
    assert_eq!(res.status(), 200);
    let saved = res.json::<Value>().await.unwrap();
    assert_eq!(
        (
            saved["mode"].clone(),
            saved["dark_theme"].clone(),
            saved["light_theme"].clone(),
            saved["custom_themes"][0]["id"].clone(),
            saved["accent"].clone(),
            saved["contrast"].clone()
        ),
        (
            json!("light"),
            json!("plum"),
            json!("neutral"),
            json!("plum"),
            json!("green"),
            json!(110)
        )
    );
    // The result still has to be valid as a whole: a theme in use can't go.
    assert_eq!(put(json!({"custom_themes":[]})).await.status(), 400);
    // And a field that is there must be a real value.
    assert_eq!(put(json!({"mode":null})).await.status(), 422);
    assert_eq!(put(json!(["mode"])).await.status(), 422);
    let after = t
        .req(Method::GET, path, &ann.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(after, saved);
}
