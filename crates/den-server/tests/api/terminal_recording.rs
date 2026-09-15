use super::*;

#[tokio::test]
async fn recording_is_opt_in_discardable_remembered_and_private() {
    let t = Test::new().await;
    let bob = t.member("recording_bob").await;
    let enrollment = t.post("/hosts/enroll", &t.admin.token, json!({})).await;
    let host = t.post("/hosts/login", "", json!({"code":enrollment["code"].as_str().unwrap().rsplit_once('#').unwrap().1,"name":"recording-host"})).await;
    let host_id = host["host_id"].as_str().unwrap();
    let mut request = format!("{}/hosts/ws", t.url.replace("http", "ws"))
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", host["token"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Frame::Binary(
            serde_json::to_vec(&json!({"type":"hello","direct_url":null}))
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(40)).await;
    let terminal = t
        .post(
            &format!("/hosts/{host_id}/sessions"),
            &t.admin.token,
            json!({}),
        )
        .await;
    let id = terminal["id"].as_str().unwrap();
    assert_eq!(terminal["state"]["terminal"]["recording_enabled"], false);
    let record = format!("/sessions/{id}/recording");
    let prefs = format!("/users/me/hosts/{host_id}/recording");
    for (enabled, expected_file) in [(None, false), (Some(true), true), (Some(false), false)] {
        if let Some(enabled) = enabled {
            t.post(&record, &t.admin.token, json!({"enabled":enabled}))
                .await;
        }
        socket
            .send(Frame::Binary(
                serde_json::to_vec(&HostFrame::Output {
                    session_id: id.into(),
                    bytes: b"output\r\n".to_vec(),
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(Ok(Frame::Binary(b))) = socket.next().await {
                    if matches!(
                        serde_json::from_slice::<HostFrame>(&b).unwrap(),
                        HostFrame::Ack { .. }
                    ) {
                        break;
                    }
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            t.dir.join(format!("uploads/{id}.recording")).exists(),
            expected_file
        );
    }
    assert_eq!(
        t.req(Method::POST, &record, &bob.token)
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        t.req(Method::PUT, &prefs, &bob.token)
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    t.req(Method::DELETE, &format!("/sessions/{id}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let ended = t
        .req(Method::GET, &format!("/objects/{id}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert!(ended["state"]["terminal"]["recording_upload_id"].is_null());
    assert_eq!(
        t.req(Method::POST, &record, &t.admin.token)
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    t.req(Method::PUT, &prefs, &t.admin.token)
        .json(&json!({"enabled":true}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let remembered = t
        .req(Method::GET, &prefs, &t.admin.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(remembered["enabled"], true);
    let terminal = t
        .post(
            &format!("/hosts/{host_id}/sessions"),
            &t.admin.token,
            json!({}),
        )
        .await;
    assert_eq!(terminal["state"]["terminal"]["recording_enabled"], true);
    let id = terminal["id"].as_str().unwrap();
    socket
        .send(Frame::Binary(
            serde_json::to_vec(&HostFrame::Output {
                session_id: id.into(),
                bytes: b"keep this recording\r\n".to_vec(),
            })
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(Ok(Frame::Binary(b))) = socket.next().await {
                if matches!(
                    serde_json::from_slice::<HostFrame>(&b).unwrap(),
                    HostFrame::Ack { .. }
                ) {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    t.req(Method::DELETE, &format!("/sessions/{id}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let ended = t
        .req(Method::GET, &format!("/objects/{id}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let upload = ended["state"]["terminal"]["recording_upload_id"]
        .as_str()
        .unwrap();
    // Completed recordings from before this field existed remain readable.
    sqlx::query(
        "UPDATE objects SET state=json_remove(state,'$.terminal.recording_enabled') WHERE id=?",
    )
    .bind(id)
    .execute(&t.state.db)
    .await
    .unwrap();
    let file = format!("/uploads/{upload}/file");
    assert_eq!(
        t.req(Method::GET, &file, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert!(!t
        .req(Method::GET, &file, &t.admin.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .is_empty());
}
