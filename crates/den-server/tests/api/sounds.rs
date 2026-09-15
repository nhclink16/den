use super::*;
use std::io::{Cursor, Read, Write};

async fn state(t: &Test, token: &str) -> Value {
    t.req(Method::GET, "/users/me/sounds", token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn put(t: &Test, token: &str, path: &str, value: Value) -> Value {
    t.req(Method::PUT, path, token)
        .json(&value)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
#[tokio::test]
async fn sound_storage_precedence_silence_missing_ids_and_admin_access() {
    let t = Test::new().await;
    let alice = t.member("sound_alice").await;
    let bob = t.member("sound_bob").await;
    let builtin = state(&t, &alice.token).await;
    assert_eq!(
        builtin["resolved"]["mention"]["sound"],
        json!({"type":"builtin","name":"mention"})
    );
    let pack = json!({"id":"server","name":"Server bells","sounds":{"mention":{"type":"builtin","name":"dm"}}});
    assert_eq!(
        t.req(Method::PUT, "/settings/sounds", &alice.token)
            .json(&pack)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    put(&t, &t.admin.token, "/settings/sounds", pack).await;
    assert_eq!(
        state(&t, &alice.token).await["resolved"]["mention"]["sound"]["name"],
        "dm"
    );
    let prefs = json!({"pack_id":"mine","custom_packs":[{"id":"mine","name":"My bells","sounds":{"mention":{"type":"builtin","name":"message"}}}],"overrides":{"dm":{"type":"silent"},"call_join":{"type":"upload","id":"missing"}},"master_volume":45,"volumes":{"message":20}});
    put(&t, &alice.token, "/users/me/sounds", prefs.clone()).await;
    let value = state(&t, &alice.token).await;
    assert_eq!(value["resolved"]["mention"]["sound"]["name"], "message");
    assert_eq!(value["resolved"]["dm"]["sound"]["type"], "silent");
    assert!(value["resolved"]["dm"]["url"].is_null());
    assert_eq!(value["resolved"]["call_join"]["sound"]["name"], "call_join");
    assert_eq!(value["preferences"]["master_volume"], 45);
    let mut prefs = prefs;
    prefs["overrides"]["mention"] = json!({"type":"builtin","name":"error"});
    assert_eq!(
        put(&t, &alice.token, "/users/me/sounds", prefs.clone()).await["resolved"]["mention"]
            ["sound"]["name"],
        "error"
    );
    prefs["overrides"]["mention"] = json!({"type":"upload","id":"unknown"});
    assert_eq!(
        put(&t, &alice.token, "/users/me/sounds", prefs).await["resolved"]["mention"]["sound"]
            ["name"],
        "message"
    );
    assert_eq!(
        state(&t, &bob.token).await["resolved"]["mention"]["sound"]["name"],
        "dm"
    );
    assert_eq!(
        t.req(Method::GET, "/users/me/sounds", "")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        t.http
            .put(format!("{}/users/me/sounds", t.url))
            .header("Cookie", format!("den_session={}", alice.token))
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}
fn wav(seconds: u32) -> Vec<u8> {
    let size = 8000 * 2 * seconds;
    let mut b = b"RIFF".to_vec();
    b.extend((36 + size).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(8000u32.to_le_bytes());
    b.extend(16000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(size.to_le_bytes());
    b.resize(44 + size as usize, 0);
    b
}
fn zip(pack: Value, entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    z.start_file("pack.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    z.write_all(&serde_json::to_vec(&pack).unwrap()).unwrap();
    for (name, data) in entries {
        z.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(&data).unwrap();
    }
    z.finish().unwrap().into_inner()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sound_upload_limits_zip_sharing_and_offline_roundtrip() {
    let mut t = Test::new().await;
    let alice = t.member("sound_designer").await;
    let bob = t.member("sound_listener").await;
    for (mime, body, status) in [
        ("audio/wav", vec![0; 512 * 1024 + 1], 413),
        ("audio/wav", wav(6), 400),
        ("audio/wav", b"not audio".to_vec(), 400),
        ("text/plain", wav(1), 415),
    ] {
        assert_eq!(
            t.req(Method::PUT, "/users/me/sounds/test", &alice.token)
                .header("content-type", mime)
                .body(body)
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    let audio = wav(1);
    assert_eq!(
        t.req(Method::PUT, "/users/me/sounds/chime", &alice.token)
            .header("content-type", "audio/wav")
            .body(audio.clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        t.req(Method::GET, "/users/me/sounds/chime", &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let pack = json!({"id":"bells","name":"Andy bells","sounds":{"mention":{"type":"upload","id":"chime"}}});
    let bundle = zip(pack.clone(), vec![("chime", audio.clone())]);
    let before = state(&t, &bob.token).await;
    for entries in [
        vec![("chime", audio.clone()), ("bad", b"bad".to_vec())],
        vec![("chime", audio.clone()), ("../escape", audio.clone())],
        vec![("chime", wav(6))],
    ] {
        assert_eq!(
            t.req(Method::PUT, "/users/me/sounds/import", &bob.token)
                .body(zip(pack.clone(), entries))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(state(&t, &bob.token).await, before);
        assert!(!t.dir.join("uploads/sounds").join(&bob.user.id).exists());
    }
    let channel = t.general().await;
    let upload=t.post("/uploads",&alice.token,json!({"channel_id":channel,"filename":"Bells.den-sounds.zip","content_type":"application/zip","size":bundle.len()})).await;
    let id = upload["id"].as_str().unwrap();
    t.req(Method::PATCH, &format!("/uploads/{id}"), &alice.token)
        .header("upload-offset", "0")
        .body(bundle)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    t.post(&format!("/uploads/{id}/complete"), &alice.token, json!({}))
        .await;
    t.post(
        &format!("/channels/{channel}/messages"),
        &alice.token,
        json!({"content":"Andy’s bells","upload_ids":[id]}),
    )
    .await;
    let preview: Value = t
        .req(Method::GET, &format!("/uploads/{id}/sounds"), &bob.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview, pack);
    assert_eq!(
        state(&t, &bob.token).await,
        before,
        "Audition must not install"
    );
    put(
        &t,
        &bob.token,
        "/users/me/sounds",
        json!({"overrides":{"dm":{"type":"silent"}}}),
    )
    .await;
    let installed = t
        .post(
            &format!("/uploads/{id}/sounds"),
            &bob.token,
            json!({"event":null}),
        )
        .await;
    assert_eq!(installed["resolved"]["dm"]["sound"]["type"], "silent");
    let path = installed["resolved"]["mention"]["url"].as_str().unwrap();
    assert_eq!(
        t.req(Method::GET, path, &bob.token)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
            .as_ref(),
        audio
    );
    let exported = t
        .req(Method::GET, "/users/me/sounds/export", &bob.token)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(&exported)).unwrap();
    let mut data = Vec::new();
    z.by_name("pack.json")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&data).unwrap()["sounds"]["dm"]["type"],
        "silent"
    );
    assert_eq!(
        t.req(Method::PUT, "/users/me/sounds/import", &alice.token)
            .body(exported.to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let admin_pack = json!({"id":"bells","name":"Server bells","sounds":{"mention":{"type":"upload","id":installed["resolved"]["mention"]["sound"]["id"]}}});
    // Import to admin, then use that same content-addressed audio as the server default.
    assert_eq!(
        t.req(Method::PUT, "/users/me/sounds/import", &t.admin.token)
            .body(exported.to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    put(&t, &t.admin.token, "/settings/sounds", admin_pack).await;
    t.stop_for_export().await;
    let archive = t.dir.join("backup.zip");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_den-server"))
        .args(["export", archive.to_str().unwrap()])
        .env("DEN_DB", t.dir.join("den.db"))
        .env("DEN_UPLOADS", t.dir.join("uploads"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let restored = t.dir.join("restored");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_den-server"))
        .args([
            "import",
            archive.to_str().unwrap(),
            "--into",
            restored.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sound_id = installed["resolved"]["mention"]["sound"]["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        std::fs::read(restored.join("uploads/sounds/server").join(sound_id)).unwrap(),
        audio
    );
    assert_eq!(
        std::fs::read(
            restored
                .join("uploads/sounds")
                .join(&bob.user.id)
                .join(sound_id)
        )
        .unwrap(),
        audio
    );
    let restored_db =
        sqlx::SqlitePool::connect(&format!("sqlite:{}", restored.join("den.db").display()))
            .await
            .unwrap();
    let json: String = sqlx::query_scalar("SELECT preferences FROM user_sounds WHERE user_id=?")
        .bind(&bob.user.id)
        .fetch_one(&restored_db)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&json).unwrap()["overrides"]["dm"]["type"],
        "silent"
    );
    restored_db.close().await;
}
