use super::*;

#[tokio::test]
async fn review_holdout_other_member_cannot_append() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let upload = t.post("/uploads", &alice.token,
        json!({"channel_id":channel,"filename":"a.bin","content_type":"application/octet-stream","size":10})).await;
    let path = format!("/uploads/{}", upload["id"].as_str().unwrap());
    let response = t.req(Method::PATCH, &path, &bob.token)
        .header("Upload-Offset", 0).body("12345").send().await.unwrap();
    assert_eq!(response.status(), 403, "only the owner may append upload bytes");
}

#[tokio::test]
async fn review_holdout_resume_discards_excess_crash_bytes() {
    let t = Test::new().await;
    let channel = t.general().await;
    let upload = t.post("/uploads", &t.admin.token,
        json!({"channel_id":channel,"filename":"a.bin","content_type":"application/octet-stream","size":10})).await;
    let id = upload["id"].as_str().unwrap();
    let path = format!("/uploads/{id}");
    let response = t.req(Method::PATCH, &path, &t.admin.token)
        .header("Upload-Offset", 0).body("01234").send().await.unwrap();
    assert_eq!(response.status(), 200);
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true)
            .open(t.dir.join("uploads").join(format!("{id}.part"))).unwrap();
        // Uncommitted bytes need not match the length of the next retry chunk.
        f.write_all(b"uncommitted-excess-crash-bytes").unwrap();
    }
    let response = t.req(Method::PATCH, &path, &t.admin.token)
        .header("Upload-Offset", 5).body("56789").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let response = t.req(Method::POST, &format!("{path}/complete"), &t.admin.token)
        .json(&json!({})).send().await.unwrap();
    assert_eq!(response.status(), 200, "resumed upload must finalize after replacing crash bytes");
    let response = t.req(Method::GET, &format!("{path}/file"), &t.admin.token).send().await.unwrap();
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"0123456789");
}
