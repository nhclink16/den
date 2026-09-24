use super::*;
use std::io::Cursor;

fn png(width: u32, shade: u8) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        width,
        10,
        image::Rgb([shade, 0, 0]),
    ))
    .write_to(&mut out, image::ImageFormat::Png)
    .unwrap();
    out.into_inner()
}

async fn upload(t: &Test, token: &str, bytes: Vec<u8>) -> BackgroundImage {
    let r = t
        .req(Method::PUT, "/users/me/background/image", token)
        .header("Content-Type", "image/png")
        .body(bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.json().await.unwrap()
}

async fn library(t: &Test, token: &str) -> Vec<BackgroundImage> {
    t.req(Method::GET, "/users/me/backgrounds", token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn past_uploads_stay_in_a_private_library_with_previews() {
    let t = Test::new().await;
    let alice = t.member("lib_alice").await;
    let bob = t.member("lib_bob").await;
    let first = upload(&t, &alice.token, png(40, 10)).await;
    let second = upload(&t, &alice.token, png(40, 20)).await;
    let again = upload(&t, &alice.token, png(40, 10)).await;
    assert_eq!(again.id, first.id, "the same picture is stored once");

    let listed = library(&t, &alice.token).await;
    assert_eq!(
        listed
            .iter()
            .map(|i| i.id.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        [first.id.as_str(), second.id.as_str()].into()
    );
    assert!(listed.iter().all(|i| i.uploaded_at > 0 && i.width == 40));

    let preview = t
        .req(
            Method::GET,
            &format!("/users/me/backgrounds/{}/preview", second.id),
            &alice.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(preview.status(), 200);
    assert_eq!(preview.headers()["content-type"], "image/jpeg");
    let original = t
        .req(
            Method::GET,
            &format!("/users/me/backgrounds/{}", second.id),
            &alice.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(original.bytes().await.unwrap().as_ref(), png(40, 20));

    // Libraries are per person: Bob neither sees nor fetches Alice's pictures.
    assert!(library(&t, &bob.token).await.is_empty());
    let peek = t
        .req(
            Method::GET,
            &format!("/users/me/backgrounds/{}", second.id),
            &bob.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(peek.status(), 404);
    for bad in ["../../den.db", "not-a-hash"] {
        let r = t
            .req(
                Method::GET,
                &format!("/users/me/backgrounds/{bad}"),
                &alice.token,
            )
            .send()
            .await
            .unwrap();
        // `../..` is resolved by the client, so this can land on another route
        // (`/users/{id}` has no GET). What matters is that no file comes back.
        assert!(
            matches!(r.status().as_u16(), 404 | 405),
            "{bad}: {}",
            r.status()
        );
    }

    // Deleting the picture in use also clears the selection.
    let mut value = json!(Appearance::default());
    value["background"] = json!({"source":{"type":"upload","id":second.id},"blur":0,"dim":0,"saturate":100,"scope":"app","fit":"cover"});
    t.req(Method::PUT, "/users/me/appearance", &alice.token)
        .json(&value)
        .send()
        .await
        .unwrap();
    let gone = t
        .req(
            Method::DELETE,
            &format!("/users/me/backgrounds/{}", second.id),
            &alice.token,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), 204);
    let appearance: Value = t
        .req(Method::GET, "/users/me/appearance", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(appearance["background"].is_null());
    assert_eq!(library(&t, &alice.token).await.len(), 1);
}

#[tokio::test]
async fn the_library_keeps_the_newest_24() {
    let t = Test::new().await;
    let alice = t.member("lib_many").await;
    let kept = upload(&t, &alice.token, png(12, 0)).await;
    let mut value = json!(Appearance::default());
    value["background"] = json!({"source":{"type":"upload","id":kept.id},"blur":0,"dim":0,"saturate":100,"scope":"app","fit":"cover"});
    t.req(Method::PUT, "/users/me/appearance", &alice.token)
        .json(&value)
        .send()
        .await
        .unwrap();
    // Uploading while an upload is selected moves the selection (the old single-image
    // meaning), so select a preset first and the library can grow past the pinned one.
    value["background"] = json!({"source":{"type":"builtin","name":"paper"},"blur":0,"dim":0,"saturate":100,"scope":"app","fit":"cover"});
    t.req(Method::PUT, "/users/me/appearance", &alice.token)
        .json(&value)
        .send()
        .await
        .unwrap();
    for shade in 1..=30u8 {
        upload(&t, &alice.token, png(12, shade)).await;
        // Distinct modification times keep "newest" unambiguous.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let listed = library(&t, &alice.token).await;
    assert_eq!(listed.len(), 24);
    assert!(
        listed.iter().all(|i| i.id != kept.id),
        "unselected old uploads age out"
    );
}

#[tokio::test]
async fn a_single_background_from_before_the_library_moves_into_it() {
    let t = Test::new().await;
    let alice = t.member("lib_legacy").await;
    let bytes = png(30, 90);
    let id = {
        use sha2::Digest;
        format!("{:x}", sha2::Sha256::digest(&bytes))
    };
    let root = t.state.uploads.join("backgrounds");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(&alice.user.id), &bytes).unwrap();
    let mut value = json!(Appearance::default());
    value["background"] = json!({"source":{"type":"upload","id":id},"blur":0,"dim":0,"saturate":100,"scope":"app","fit":"cover"});
    sqlx::query("INSERT INTO user_appearance(user_id, appearance) VALUES(?, ?) ON CONFLICT(user_id) DO UPDATE SET appearance=excluded.appearance")
        .bind(&alice.user.id)
        .bind(value.to_string())
        .execute(&t.state.db)
        .await
        .unwrap();

    t.state.migrate_backgrounds().await.unwrap();

    let listed = library(&t, &alice.token).await;
    assert_eq!(
        listed.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        [id.as_str()]
    );
    let appearance: Value = t
        .req(Method::GET, "/users/me/appearance", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        appearance["background"]["source"]["id"],
        id.as_str(),
        "still selected"
    );
    assert!(root.join(&alice.user.id).join(format!("{id}.jpg")).exists());
}

fn wallpaper(source: Value) -> Value {
    json!({"source":source,"blur":0,"dim":20,"saturate":100,"scope":"app","fit":"cover"})
}
async fn put_appearance(t: &Test, token: &str, body: &Value) -> Appearance {
    let r = t
        .req(Method::PUT, "/users/me/appearance", token)
        .json(body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{body}");
    r.json().await.unwrap()
}

#[tokio::test]
async fn the_sidebar_keeps_its_own_wallpaper_through_older_clients() {
    let t = Test::new().await;
    let alice = t.member("side_alice").await;
    let main = upload(&t, &alice.token, png(40, 30)).await;
    let side = upload(&t, &alice.token, png(40, 60)).await;
    let mut body = json!(Appearance::default());
    body["background"] = wallpaper(json!({"type":"upload","id":main.id}));
    body["sidebar_background"] = wallpaper(json!({"type":"upload","id":side.id}));
    let saved = put_appearance(&t, &alice.token, &body).await;
    assert!(saved.sidebar_background.is_some());

    // An app that predates the field saves a new theme without it: keep the sidebar.
    let mut older = json!(Appearance::default());
    older.as_object_mut().unwrap().remove("sidebar_background");
    older["dark_theme"] = json!("moss");
    older["background"] = body["background"].clone();
    let kept = put_appearance(&t, &alice.token, &older).await;
    assert_eq!(kept.dark_theme, "moss");
    assert_eq!(kept.sidebar_background, saved.sidebar_background);

    // Deleting the sidebar's image clears only the sidebar.
    let r = t
        .req(
            Method::DELETE,
            &format!("/users/me/backgrounds/{}", side.id),
            &alice.token,
        )
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let now: Appearance = t
        .req(Method::GET, "/users/me/appearance", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(now.sidebar_background.is_none());
    assert!(now.background.is_some(), "the main wallpaper is untouched");

    // A preset works for the sidebar too, and an explicit null clears it.
    body["sidebar_background"] = wallpaper(json!({"type":"builtin","name":"plaid"}));
    assert!(put_appearance(&t, &alice.token, &body)
        .await
        .sidebar_background
        .is_some());
    body["sidebar_background"] = Value::Null;
    assert!(put_appearance(&t, &alice.token, &body)
        .await
        .sidebar_background
        .is_none());
}
