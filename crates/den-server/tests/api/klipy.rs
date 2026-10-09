use super::*;
use std::io::Cursor;

fn picture(name: &str) -> Value {
    json!({
        "url": format!("https://static.klipy.com/ii/abc/{name}.webp"),
        "still_url": format!("https://static.klipy.com/ii/abc/{name}.jpg"),
        "width": 498,
        "height": 280,
    })
}
/// Where a picture URL sends the browser. KLIPY's terms forbid Den from
/// fetching the GIF, so these must be redirects, never bytes.
async fn location(t: &Test, path: &str, token: &str) -> String {
    let r = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(format!("{}{path}", t.url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 302, "{path}");
    r.headers()["location"].to_str().unwrap().into()
}

#[tokio::test]
async fn klipy_gifs_are_kept_as_links_that_redirect_to_klipy() {
    let t = Test::new().await;
    let bob = t.member("klipy_bob").await;
    assert_eq!(
        t.req(Method::GET, "/klipy", "")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // Only KLIPY's own media links, so a picture can never point elsewhere.
    for url in [
        "https://evil.example/x.webp",
        "https://static.klipy.com.evil.example/x.webp",
        "http://static.klipy.com/x.webp",
        "https://static.klipy.com/x.webp\r\nSet-Cookie: a=b",
    ] {
        let mut body = picture("x");
        body["url"] = json!(url);
        let r = t
            .req(Method::PUT, "/users/me/avatar/klipy", &t.admin.token)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400, "{url}");
    }

    // A profile picture: animated for people who hover it, KLIPY's still otherwise.
    let user: User = t
        .req(Method::PUT, "/users/me/avatar/klipy", &t.admin.token)
        .json(&picture("hi"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = user.avatar_url.unwrap();
    assert!(url.contains("?v=a_"), "{url}");
    assert_eq!(
        location(&t, &url, &bob.token).await,
        "https://static.klipy.com/ii/abc/hi.webp"
    );
    assert_eq!(
        location(&t, &format!("{url}&still=1"), &bob.token).await,
        "https://static.klipy.com/ii/abc/hi.jpg"
    );
    // Uploading a picture afterwards replaces the link.
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(20, 20)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let user: User = t
        .req(Method::PUT, "/users/me/avatar", &t.admin.token)
        .header("Content-Type", "image/png")
        .body(png.into_inner())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r = t
        .req(Method::GET, &user.avatar_url.unwrap(), &bob.token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.headers()["content-type"], "image/png");

    // A wallpaper joins the private library and can be chosen like an upload.
    let r = t
        .req(Method::POST, "/users/me/backgrounds/klipy", &bob.token)
        .json(&picture("wall"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let saved: BackgroundImage = r.json().await.unwrap();
    assert_eq!((saved.width, saved.height), (498, 280));
    assert_eq!(saved.content_type, "image/webp");
    let path = format!("/users/me/backgrounds/{}", saved.id);
    assert_eq!(
        location(&t, &path, &bob.token).await,
        "https://static.klipy.com/ii/abc/wall.webp"
    );
    for still in [format!("{path}?still=1"), format!("{path}/preview")] {
        assert_eq!(
            location(&t, &still, &bob.token).await,
            "https://static.klipy.com/ii/abc/wall.jpg"
        );
    }
    let mut body = json!(Appearance::default());
    body["background"] = json!({"source":{"type":"upload","id":saved.id},"blur":0,"dim":20,"saturate":100,"scope":"app","fit":"cover"});
    let kept: Appearance = t
        .req(Method::PUT, "/users/me/appearance", &bob.token)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(kept.background.is_some());
    assert_eq!(
        t.req(Method::GET, &path, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        404,
        "another account's library stays private"
    );
}
