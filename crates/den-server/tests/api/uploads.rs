use super::*;

#[tokio::test]
async fn upload_resume_survives_uncommitted_bytes_and_serves_authenticated_ranges() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let dm = t
        .post("/dms", &alice.token, json!({"member_ids":[bob.user.id]}))
        .await;
    let cid = dm["id"].as_str().unwrap();
    let upload = t
        .post(
            "/uploads",
            &alice.token,
            json!({"channel_id":cid,"filename":"clip.mp4","content_type":"video/mp4","size":10}),
        )
        .await;
    let uid = upload["id"].as_str().unwrap();
    let path = format!("/uploads/{uid}");
    assert_eq!(
        t.req(Method::POST, &format!("{path}/complete"), &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        t.req(Method::PATCH, &path, &bob.token)
            .header("Upload-Offset", 0)
            .body("01234")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::PATCH, &path, &alice.token)
            .header("Upload-Offset", 0)
            .body("01234")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Simulate a process dying after file write but before SQLite offset commit.
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(t.dir.join("uploads").join(format!("{uid}.part")))
            .unwrap();
        f.write_all(b"crash").unwrap();
    }
    let status: Upload = t
        .req(Method::GET, &path, &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status.offset, 5);
    assert_eq!(
        t.req(Method::PATCH, &path, &alice.token)
            .header("Upload-Offset", 0)
            .body("01234")
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        t.req(Method::PATCH, &path, &alice.token)
            .header("Upload-Offset", 5)
            .body("567890")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        t.req(Method::PATCH, &path, &alice.token)
            .header("Upload-Offset", 5)
            .body("56789")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Recover finalization after rename but before the complete flag commits.
    std::fs::rename(
        t.dir.join("uploads").join(format!("{uid}.part")),
        t.dir.join("uploads").join(uid),
    )
    .unwrap();
    let completed = t
        .post(&format!("{path}/complete"), &alice.token, json!({}))
        .await;
    assert_eq!(completed["complete"], true);
    assert_eq!(
        t.post(&format!("{path}/complete"), &alice.token, json!({}))
            .await["complete"],
        true
    );
    let file = format!("{path}/file");
    assert_eq!(
        t.req(Method::GET, &file, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.http
            .get(format!("{}{file}", t.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let head = t.req(Method::HEAD, &file, &bob.token).send().await.unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-length"], "10");
    assert!(head.bytes().await.unwrap().is_empty());
    for (range, expected, content_range) in [
        ("bytes=2-5", "2345", "bytes 2-5/10"),
        ("bytes=-3", "789", "bytes 7-9/10"),
        ("bytes=7-", "789", "bytes 7-9/10"),
    ] {
        let r = t
            .req(Method::GET, &file, &bob.token)
            .header("Range", range)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 206);
        assert_eq!(r.headers()["content-range"], content_range);
        assert_eq!(r.headers()["content-type"], "video/mp4");
        assert_eq!(r.text().await.unwrap(), expected);
    }
    assert_eq!(
        t.req(Method::GET, &file, &bob.token)
            .header("Range", "bytes=20-30")
            .send()
            .await
            .unwrap()
            .status(),
        416
    );
    let msg = t
        .post(
            &format!("/channels/{cid}/messages"),
            &alice.token,
            json!({"content":"clip","upload_ids":[uid]}),
        )
        .await;
    assert_eq!(msg["attachments"][0]["id"], uid);
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/channels/{cid}/messages"),
            &alice.token
        )
        .json(&json!({"content":"reuse","upload_ids":[uid]}))
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    let stale = t
        .post(
            "/uploads",
            &alice.token,
            json!({"channel_id":cid,"filename":"stale","content_type":"text/plain","size":10}),
        )
        .await;
    let stale = stale["id"].as_str().unwrap();
    sqlx::query("UPDATE uploads SET touched_at=0 WHERE id=?")
        .bind(stale)
        .execute(&t.state.db)
        .await
        .unwrap();
    t.state.cleanup().await.unwrap();
    assert!(!t.dir.join("uploads").join(format!("{stale}.part")).exists());
}

#[tokio::test]
async fn upload_deletion_requires_owner_or_admin_and_no_live_references() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let channels: Vec<Channel> = t
        .req(Method::GET, "/channels", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cid = &channels.iter().find(|c| c.name == "general").unwrap().id;
    let upload = t.post("/uploads", &alice.token, json!({"channel_id":cid,"filename":"smoke.bin","content_type":"application/octet-stream","size":1})).await;
    let id = upload["id"].as_str().unwrap();
    let path = format!("/uploads/{id}");
    assert_eq!(
        t.req(Method::DELETE, &path, "")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        t.req(Method::DELETE, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        t.req(Method::PATCH, &path, &alice.token)
            .header("Upload-Offset", 0)
            .body("x")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    t.post(&format!("{path}/complete"), &alice.token, json!({}))
        .await;
    let message = t
        .post(
            &format!("/channels/{cid}/messages"),
            &alice.token,
            json!({"content":"smoke","upload_ids":[id]}),
        )
        .await;
    assert_eq!(
        t.req(Method::DELETE, &path, &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/messages/{}", message["id"].as_str().unwrap()),
            &alice.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    let object = t.post(&format!("/channels/{cid}/objects"), &alice.token, json!({"kind":"canvas","name":"smoke","state":{"asset":{"id":"asset","src":format!("/uploads/{id}/file")}}})).await;
    assert_eq!(
        t.req(Method::DELETE, &path, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/messages/{}", object["message_id"].as_str().unwrap()),
            &alice.token
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    sqlx::query("INSERT INTO terminal_recordings(upload_id,session_id) VALUES(?, 'ended-smoke')")
        .bind(id)
        .execute(&t.state.db)
        .await
        .unwrap();
    assert_eq!(
        t.req(Method::DELETE, &path, &t.admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert!(!t.dir.join("uploads").join(id).exists());
    assert_eq!(
        t.req(Method::GET, &path, &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM terminal_recordings WHERE upload_id=?")
            .bind(id)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    let pending = t.post("/uploads", &alice.token, json!({"channel_id":cid,"filename":"pending.bin","content_type":"application/octet-stream","size":1})).await;
    let id = pending["id"].as_str().unwrap();
    assert_eq!(
        t.req(Method::DELETE, &format!("/uploads/{id}"), &alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert!(!t.dir.join("uploads").join(format!("{id}.part")).exists());
}

async fn rejected(
    t: &Test,
    token: &str,
    body: serde_json::Value,
) -> (StatusCode, String, String) {
    let r = t
        .req(Method::POST, "/uploads", token)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = r.status();
    let err: Value = r.json().await.unwrap();
    (
        status,
        err["error"].as_str().unwrap_or_default().into(),
        err["message"].as_str().unwrap_or_default().into(),
    )
}

fn begin(cid: &str, filename: &str, size: i64) -> Value {
    json!({"channel_id":cid,"filename":filename,"content_type":"application/octet-stream","size":size})
}

#[tokio::test]
async fn upload_begin_rejects_bad_sizes_and_filenames_without_admitting_them() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let cid = t.general().await;
    let max = 1024 * 1024; // Test harness max_upload bytes.
    for size in [0, -3, max + 1] {
        let (status, code, message) = rejected(&t, &alice.token, begin(&cid, "big.bin", size)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "size {size}");
        assert_eq!(code, "upload_size");
        assert_eq!(message, format!("Size must be 1-{max} bytes"));
    }
    let names = vec![
        "".to_string(),
        "dir/a.bin".to_string(),
        "dir\\a.bin".to_string(),
        "bad\u{1}.bin".to_string(),
        "x".repeat(256),
    ];
    for filename in names {
        let (status, code, message) =
            rejected(&t, &alice.token, begin(&cid, &filename, 1)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "filename {filename:?}");
        assert_eq!(code, "invalid_request");
        assert_eq!(message, "Invalid filename");
    }
    // Rejected admission must not leave rows or files behind.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM uploads")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(std::fs::read_dir(t.dir.join("uploads")).unwrap().count(), 0);
}

#[tokio::test]
async fn upload_begin_caps_five_pending_per_owner_and_completion_frees_a_slot() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let mut first = String::new();
    for i in 0..5 {
        let up = t
            .post(
                "/uploads",
                &alice.token,
                begin(&cid, &format!("p{i}.bin"), 10),
            )
            .await;
        if i == 0 {
            first = up["id"].as_str().unwrap().into();
        }
    }
    let (status, code, message) = rejected(&t, &alice.token, begin(&cid, "six.bin", 10)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code, "conflict");
    assert_eq!(message, "Finish pending uploads first; maximum five");
    // The rejected sixth upload created no row and no file.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM uploads WHERE owner_id=?")
            .bind(&alice.user.id)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        5
    );
    assert_eq!(std::fs::read_dir(t.dir.join("uploads")).unwrap().count(), 5);
    // Each owner has an independent allowance.
    assert_eq!(
        t.req(Method::POST, "/uploads", &bob.token)
            .json(&begin(&cid, "bob.bin", 10))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Completing one of alice's uploads frees her sixth slot.
    assert_eq!(
        t.req(Method::PATCH, &format!("/uploads/{first}"), &alice.token)
            .header("Upload-Offset", 0)
            .body("0123456789")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    t.post(&format!("/uploads/{first}/complete"), &alice.token, json!({}))
        .await;
    assert_eq!(
        t.req(Method::POST, "/uploads", &alice.token)
            .json(&begin(&cid, "six.bin", 10))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}
