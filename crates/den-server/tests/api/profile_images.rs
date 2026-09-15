use super::*;
use std::{io::Cursor, process::Command};

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut image = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(width, height)
        .write_to(&mut image, image::ImageFormat::Png)
        .unwrap();
    image.into_inner()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_images_preserve_gifs_enforce_ownership_and_roundtrip_archives() {
    let mut t = Test::new().await;
    let bob = t.member("avatar_bob").await;
    let avatar = png(20, 20);
    let put = t
        .req(Method::PUT, "/users/me/avatar", &t.admin.token)
        .header("Content-Type", "image/png")
        .body(avatar.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);
    let user: User = put.json().await.unwrap();
    let url = user.avatar_url.unwrap();
    assert!(url.starts_with(&format!("/users/{}/avatar?v=", t.admin.user.id)));
    let fetched = t.req(Method::GET, &url, &bob.token).send().await.unwrap();
    assert_eq!(fetched.status(), 200);
    assert_eq!(fetched.headers()["cache-control"], "private, max-age=86400");
    let etag = fetched.headers()["etag"].clone();
    assert!(url.ends_with(etag.to_str().unwrap().trim_matches('"')));
    let derived = fetched.bytes().await.unwrap();
    let decoded = image::load_from_memory(&derived).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (256, 256));
    assert_eq!(
        t.req(Method::GET, &url, &bob.token)
            .header("If-None-Match", etag)
            .send()
            .await
            .unwrap()
            .status(),
        304
    );
    assert_eq!(
        t.req(Method::GET, &url, "").send().await.unwrap().status(),
        401
    );
    assert_eq!(
        std::fs::read(
            t.state
                .uploads
                .join("profiles")
                .join(&t.admin.user.id)
                .join("avatar")
        )
        .unwrap(),
        avatar
    );
    for (path, limit) in [
        ("/users/me/avatar", 4 * 1024 * 1024),
        ("/users/me/banner", 8 * 1024 * 1024),
    ] {
        assert_eq!(
            t.req(Method::PUT, path, &t.admin.token)
                .header("Content-Type", "image/png")
                .body(vec![0; limit + 1])
                .send()
                .await
                .unwrap()
                .status(),
            413
        );
        assert_eq!(
            t.req(Method::PUT, path, &t.admin.token)
                .header("Content-Type", "image/png")
                .body("not an image")
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        t.req(Method::PUT, "/users/me/avatar", &t.admin.token)
            .header("Content-Type", "image/png")
            .body(png(20, 10))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        t.req(Method::PUT, "/users/me/avatar", &t.admin.token)
            .header("Content-Type", "image/png")
            .body(png(4097, 1))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let banner = png(1600, 400);
    let put = t
        .req(Method::PUT, "/users/me/banner", &t.admin.token)
        .header("Content-Type", "image/png")
        .body(banner.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);
    let banner_url = put.json::<User>().await.unwrap().banner_url.unwrap();
    let fetched = t
        .req(Method::GET, &banner_url, &bob.token)
        .send()
        .await
        .unwrap();
    assert_eq!(fetched.status(), 200);
    let banner_preview = fetched.bytes().await.unwrap();
    let decoded = image::load_from_memory(&banner_preview).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (1200, 300));
    let mut gif = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
        encoder
            .set_repeat(image::codecs::gif::Repeat::Infinite)
            .unwrap();
        for color in [[255, 0, 0, 255], [0, 255, 0, 255]] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    image::RgbaImage::from_pixel(2, 2, image::Rgba(color)),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(100, 1),
                ))
                .unwrap();
        }
    }
    let put = t
        .req(Method::PUT, "/users/me/avatar", &t.admin.token)
        .header("Content-Type", "image/gif")
        .body(gif.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);
    let gif_url = put.json::<User>().await.unwrap().avatar_url.unwrap();
    assert_ne!(gif_url, url);
    let fetched = t
        .req(Method::GET, &gif_url, &bob.token)
        .send()
        .await
        .unwrap();
    assert_eq!(fetched.headers()["content-type"], "image/gif");
    let bytes = fetched.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), gif);
    use image::AnimationDecoder;
    assert_eq!(
        image::codecs::gif::GifDecoder::new(Cursor::new(&bytes))
            .unwrap()
            .into_frames()
            .collect_frames()
            .unwrap()
            .len(),
        2
    );
    assert!(!t
        .state
        .uploads
        .join("profiles")
        .join(&t.admin.user.id)
        .join("avatar.png")
        .exists());
    let bot = t
        .post(
            "/bots",
            &t.admin.token,
            json!({"username":"profile_bot","display_name":"Profile bot"}),
        )
        .await;
    let bot_id = bot["user"]["id"].as_str().unwrap();
    let bot_path = format!("/users/{bot_id}/avatar");
    assert_eq!(
        t.req(Method::PUT, &bot_path, &bob.token)
            .header("Content-Type", "image/png")
            .body(avatar.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(
            Method::PUT,
            &format!("/users/{}/avatar", bob.user.id),
            &t.admin.token
        )
        .header("Content-Type", "image/png")
        .body(avatar.clone())
        .send()
        .await
        .unwrap()
        .status(),
        403
    );
    assert_eq!(
        t.req(Method::PUT, &bot_path, &t.admin.token)
            .header("Content-Type", "image/png")
            .body(avatar.clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        t.req(Method::GET, &bot_path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    t.stop_for_export().await;
    let archive = t.dir.join("profiles.zip");
    let result = Command::new(env!("CARGO_BIN_EXE_den-server"))
        .args(["export", archive.to_str().unwrap()])
        .env("DEN_DB", t.dir.join("den.db"))
        .env("DEN_UPLOADS", &t.state.uploads)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let restored = t.dir.join("restored");
    let result = Command::new(env!("CARGO_BIN_EXE_den-server"))
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
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let root = restored.join("uploads/profiles").join(&t.admin.user.id);
    assert_eq!(std::fs::read(root.join("avatar")).unwrap(), gif);
    assert_eq!(std::fs::read(root.join("banner")).unwrap(), banner);
    assert_eq!(
        std::fs::read(root.join("banner.png")).unwrap(),
        banner_preview
    );
    assert_eq!(
        std::fs::read(
            restored
                .join("uploads/profiles")
                .join(bot_id)
                .join("avatar")
        )
        .unwrap(),
        avatar
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
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = den_server::router(state.clone());
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let fetched = t
        .http
        .get(format!("{origin}{gif_url}"))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap();
    assert_eq!(fetched.status(), 200);
    assert_eq!(fetched.bytes().await.unwrap(), gif);
    for (kind, url) in [("avatar", gif_url), ("banner", banner_url)] {
        let response = t
            .http
            .delete(format!("{origin}/users/me/{kind}"))
            .bearer_auth(&t.admin.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(response.json::<Value>().await.unwrap()[format!("{kind}_url")].is_null());
        assert_eq!(
            t.http
                .get(format!("{origin}{url}"))
                .bearer_auth(&bob.token)
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
        assert!(!root.join(kind).exists());
        assert!(!root.join(format!("{kind}.png")).exists());
    }
    task.abort();
    state.db.close().await;
}
