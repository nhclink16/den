use super::*;

fn content_id_of(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backgrounds_are_private_bounded_and_portable() {
    use std::{io::Cursor, process::Command};
    let mut t = Test::new().await;
    let alice = t.admin.clone();
    let bob = t.member("background_bob").await;
    let path = "/users/me/background/image";
    assert_eq!(
        t.req(Method::GET, path, "").send().await.unwrap().status(),
        401
    );
    assert_eq!(
        t.req(Method::GET, path, &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    // Presets travel under their original names, which older iOS builds decode
    // strictly. The redesign's names are accepted and stored as the original.
    for (name, now) in [
        ("lamplight", "ember-sky"),
        ("paper", "grain"),
        ("aurora", "aurora"),
        ("dunes", "dunes"),
        ("clearing", "aurora"),
        ("contours", "dunes"),
        ("harbor", "harbor"),
        ("doorway", "harbor"),
        ("ember-sky", "ember-sky"),
        ("slate-mist", "slate-mist"),
        ("plaid", "slate-mist"),
        ("grain", "grain"),
        ("unknown", ""),
    ] {
        let mut preference = json!(Appearance::default());
        preference["background"] = json!({"source":{"type":"builtin","name":name},"blur":-1,"dim":999,"saturate":-1,"scope":"sidebar","fit":"tile"});
        let response = t
            .req(Method::PUT, "/users/me/appearance", &bob.token)
            .json(&preference)
            .send()
            .await
            .unwrap();
        if name == "unknown" {
            assert_eq!(response.status(), 422);
        } else {
            assert_eq!(response.status(), 200);
            let bounded: Value = response.json().await.unwrap();
            assert_eq!(bounded["background"]["blur"], 0);
            assert_eq!(bounded["background"]["dim"], 80);
            assert_eq!(bounded["background"]["saturate"], 50);
            assert_eq!(bounded["background"]["source"]["name"], now);
        }
    }
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(3, 2)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    let upload = t
        .req(Method::PUT, path, &alice.token)
        .header("Content-Type", "image/png")
        .body(png.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), 200);
    let metadata: BackgroundImage = upload.json().await.unwrap();
    assert_eq!(
        (metadata.width, metadata.height, metadata.size),
        (3, 2, png.len() as u64)
    );
    assert_eq!(metadata.content_type, "image/png");
    let fetched = t.req(Method::GET, path, &alice.token).send().await.unwrap();
    assert_eq!(fetched.status(), 200);
    assert_eq!(fetched.headers()["cache-control"], "private, max-age=3600");
    assert_eq!(fetched.headers()["vary"], "Authorization, Cookie");
    let etag = fetched.headers()["etag"].clone();
    assert_eq!(fetched.bytes().await.unwrap().as_ref(), png);
    assert_eq!(
        t.req(Method::GET, path, &alice.token)
            .header("If-None-Match", etag.clone())
            .send()
            .await
            .unwrap()
            .status(),
        304
    );
    assert_eq!(
        t.req(Method::GET, path, &bob.token)
            .header("If-None-Match", etag.clone())
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .header("Content-Type", "image/png")
            .body(vec![0; 16 * 1024 * 1024 + 1])
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .header("Content-Type", "image/png")
            .body("not an image")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .header("Content-Type", "image/jpeg")
            .body(png.clone())
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        t.req(Method::PUT, path, &alice.token)
            .header("Content-Type", "image/svg+xml")
            .body("<svg/>")
            .send()
            .await
            .unwrap()
            .status(),
        415
    );
    assert_eq!(
        t.http
            .put(format!("{}{path}", t.url))
            .header("Cookie", format!("den_session={}", alice.token))
            .header("Content-Type", "image/png")
            .body(png.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    // Wider than attachments allow is fine for a wallpaper; past 12,000 px is
    // refused with the size and the limit, not a generic decoding error.
    for (width, ok) in [(8193, true), (12_001, false)] {
        let mut wide = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(width, 1)
            .write_to(&mut wide, image::ImageFormat::Png)
            .unwrap();
        let r = t
            .req(Method::PUT, path, &alice.token)
            .header("Content-Type", "image/png")
            .body(wide.into_inner())
            .send()
            .await
            .unwrap();
        if ok {
            assert_eq!(r.status(), 200);
        } else {
            assert_eq!(r.status(), 400);
            let e: ApiError = r.json().await.unwrap();
            assert!(
                e.message.contains("12001×1") && e.message.contains("40 megapixels"),
                "{}",
                e.message
            );
        }
    }
    let mut value = json!(Appearance::default());
    value["contrast"] = json!(900);
    value["background"] = json!({"source":{"type":"upload","id":metadata.id},"blur":900,"dim":-20,"saturate":900,"scope":"app","fit":"cover"});
    let saved = t
        .req(Method::PUT, "/users/me/appearance", &alice.token)
        .json(&value)
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    let saved: Value = saved.json().await.unwrap();
    assert_eq!(saved["contrast"], 120);
    assert_eq!(saved["background"]["blur"], 40);
    assert_eq!(saved["background"]["dim"], 0);
    assert_eq!(saved["background"]["saturate"], 150);
    let mut foreign = value.clone();
    foreign["contrast"] = json!(-10);
    let missing = t
        .req(Method::PUT, "/users/me/appearance", &bob.token)
        .json(&foreign)
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 200);
    assert_eq!(missing.headers()["x-den-background-status"], "missing");
    let missing: Value = missing.json().await.unwrap();
    assert!(missing["background"].is_null());
    assert_eq!(missing["contrast"], 80);
    // Missing files do not prevent loading an otherwise valid preference.
    let file = t
        .state
        .uploads
        .join("backgrounds")
        .join(&alice.user.id)
        .join(&metadata.id);
    std::fs::remove_file(&file).unwrap();
    let missing = t
        .req(Method::GET, "/users/me/appearance", &alice.token)
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 200);
    assert_eq!(missing.headers()["x-den-background-status"], "missing");
    assert!(missing.json::<Value>().await.unwrap()["background"].is_null());
    // New uploads join the library and move an already-selected upload along.
    for format in [
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
        image::ImageFormat::Gif,
        image::ImageFormat::Png,
    ] {
        let mut image = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(4, 3)
            .write_to(&mut image, format)
            .unwrap();
        let response = t
            .req(Method::PUT, path, &alice.token)
            .header("Content-Type", format.to_mime_type())
            .body(image.into_inner())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let library: Vec<BackgroundImage> = t
        .req(Method::GET, "/users/me/backgrounds", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(library.len(), 5, "the 8193 px strip and four formats");
    let replacement = t
        .req(Method::GET, path, &alice.token)
        .header("If-None-Match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(replacement.status(), 200);
    let replacement = replacement.bytes().await.unwrap();
    // Exercise the real offline CLI, then serve the restored account's file.
    t.stop_for_export().await;
    let archive = t.dir.join("background.zip");
    let output = Command::new(env!("CARGO_BIN_EXE_den-server"))
        .args(["export", archive.to_str().unwrap()])
        .env("DEN_DB", t.dir.join("den.db"))
        .env("DEN_UPLOADS", &t.state.uploads)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let restored = t.dir.join("restored");
    let output = Command::new(env!("CARGO_BIN_EXE_den-server"))
        .args([
            "import",
            archive.to_str().unwrap(),
            "--into",
            restored.to_str().unwrap(),
            "--keep-credentials",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let selected = content_id_of(&replacement);
    assert_eq!(
        std::fs::read(
            restored
                .join("uploads/backgrounds")
                .join(&alice.user.id)
                .join(&selected)
        )
        .unwrap(),
        replacement
    );
    assert!(
        restored
            .join("uploads/backgrounds")
            .join(&alice.user.id)
            .join(format!("{selected}.jpg"))
            .exists(),
        "previews travel too"
    );
    let state = AppState::open(
        restored.join("den.db"),
        restored.join("uploads"),
        restored.join("bootstrap.key"),
        t.url.clone(),
        1024 * 1024,
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let restored_url = format!("http://{}", listener.local_addr().unwrap());
    let router = den_server::router(state.clone());
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let fetched = t
        .http
        .get(format!("{restored_url}{path}"))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap();
    assert_eq!(fetched.status(), 200);
    assert_eq!(fetched.bytes().await.unwrap(), replacement);
    let appearance: Value = t
        .http
        .get(format!("{restored_url}/users/me/appearance"))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(appearance["background"]["source"]["id"].is_string());
    assert_eq!(
        t.http
            .delete(format!("{restored_url}{path}"))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    // Only the image in use goes; the rest of the library stays.
    assert!(!restored
        .join("uploads/backgrounds")
        .join(&alice.user.id)
        .join(&selected)
        .exists());
    let left: Vec<BackgroundImage> = t
        .http
        .get(format!("{restored_url}/users/me/backgrounds"))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(left.len(), 4);
    let appearance: Value = t
        .http
        .get(format!("{restored_url}/users/me/appearance"))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(appearance["background"].is_null());
    task.abort();
    state.db.close().await;
}
