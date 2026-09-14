use super::*;

async fn upload(t: &Test, token: &str, channel: &str, mime: &str, bytes: Vec<u8>) -> Upload {
    let row=t.post("/uploads",token,json!({"channel_id":channel,"filename":"image.bin","content_type":mime,"size":bytes.len()})).await;
    let id = row["id"].as_str().unwrap();
    let r = t
        .req(Method::PATCH, &format!("/uploads/{id}"), token)
        .header("Upload-Offset", 0)
        .body(bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    serde_json::from_value(
        t.post(&format!("/uploads/{id}/complete"), token, json!({}))
            .await,
    )
    .unwrap()
}
#[tokio::test]
async fn image_thumbnails_decode_to_bounded_png_and_enforce_private_access() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let cid = dm["id"].as_str().unwrap();
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1200,
        600,
        image::Rgba([128, 32, 64, 128]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let row = upload(&t, &alice.token, cid, "image/png", bytes.into_inner()).await;
    let path = row.thumbnail_url.unwrap();
    let response = t.req(Method::GET, &path, &bob.token).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/png");
    let png = image::load_from_memory(&response.bytes().await.unwrap()).unwrap();
    assert_eq!((png.width(), png.height()), (512, 256));
    assert_eq!(png.to_rgba8().get_pixel(0, 0).0, [128, 32, 64, 128]);
    assert_eq!(
        t.req(Method::GET, &path, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.http
            .get(format!("{}{path}", t.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let head = t.req(Method::HEAD, &path, &bob.token).send().await.unwrap();
    assert_eq!(head.status(), 200);
    assert!(head.bytes().await.unwrap().is_empty());
    // Pre-M2 uploads and lost derived files can be regenerated on demand.
    std::fs::remove_file(t.dir.join("uploads").join(format!("{}.thumb.png", row.id))).unwrap();
    sqlx::query("UPDATE uploads SET thumbnail_ready=0 WHERE id=?")
        .bind(&row.id)
        .execute(&t.state.db)
        .await
        .unwrap();
    assert_eq!(
        t.req(Method::GET, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let invalid = upload(
        &t,
        &alice.token,
        cid,
        "image/jpeg",
        b"not really a JPEG".to_vec(),
    )
    .await;
    assert!(invalid.complete);
    assert!(invalid.thumbnail_url.is_none());
    assert_eq!(
        t.req(
            Method::GET,
            &format!("/uploads/{}/thumbnail", invalid.id),
            &bob.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
    assert_eq!(
        t.req(
            Method::GET,
            &format!("/uploads/{}/file", invalid.id),
            &bob.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
}
#[tokio::test]
async fn spa_fallback_preserves_api_auth_and_missing_asset_errors() {
    let t = Test::new().await;
    let root = t.dir.join("spa");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let html = "<!doctype html><title>Den fixture</title><div id='app'></div>";
    std::fs::write(root.join("index.html"), html).unwrap();
    std::fs::write(root.join("assets/app.js"), "console.log('fixture');").unwrap();
    for path in ["/", "/inbox", "/chat/abc"] {
        let response = t.http.get(format!("{}{path}", t.url)).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().await.unwrap(), html);
    }
    let settings_page = t
        .http
        .get(format!("{}/settings", t.url))
        .header("Accept", "text/html")
        .send()
        .await
        .unwrap();
    assert_eq!(settings_page.status(), 200);
    assert_eq!(settings_page.headers()["cache-control"], "no-store");
    assert_eq!(settings_page.headers()["vary"], "Accept");
    assert_eq!(settings_page.text().await.unwrap(), html);
    let asset = t
        .http
        .get(format!("{}/assets/app.js", t.url))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.status(), 200);
    assert!(asset.headers()["content-type"]
        .to_str()
        .unwrap()
        .contains("javascript"));
    let head = t
        .http
        .head(format!("{}/inbox", t.url))
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(
        head.headers()["content-length"].to_str().unwrap(),
        html.len().to_string()
    );
    assert!(head.bytes().await.unwrap().is_empty());
    for path in [
        "/assets/missing.js",
        "/missing.png",
        "/users/misspelled",
        "/api/missing",
        "/objects/missing/typo",
    ] {
        assert_eq!(
            t.http
                .get(format!("{}{path}", t.url))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        t.http
            .get(format!("{}/channels", t.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        t.http
            .post(format!("{}/inbox", t.url))
            .send()
            .await
            .unwrap()
            .status(),
        405
    );
}
#[tokio::test]
async fn upgrading_m1_indexes_existing_messages_and_mentions() {
    let dir = std::env::temp_dir().join(format!("den-upgrade-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::write(
        dir.join("migrations/0001_chat.sql"),
        include_str!("../../migrations/0001_chat.sql"),
    )
    .unwrap();
    let db = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.join("den.db"))
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::migrate::Migrator::new(dir.join("migrations"))
        .await
        .unwrap()
        .run(&db)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO users(id,username,display_name,role) VALUES('a','alice','Alice','admin'),('b','bob','Bob','member'); INSERT INTO channels(id,name,kind) VALUES('c','general','text'); INSERT INTO messages(id,channel_id,author_id,content) VALUES('m','c','a','existing otter @bob');").execute(&db).await.unwrap();
    db.close().await;
    let state = AppState::open(
        dir.join("den.db"),
        dir.join("uploads"),
        dir.join("bootstrap.key"),
        "http://127.0.0.1:7000".into(),
        100000,
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM message_search WHERE message_search MATCH 'otter'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let mentioned: String =
        sqlx::query_scalar("SELECT user_id FROM message_mentions WHERE message_id='m'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(mentioned, "b");
    state.db.close().await;
    std::fs::remove_dir_all(dir).unwrap();
}
