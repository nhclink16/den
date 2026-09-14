use super::*;

#[tokio::test]
async fn native_tickets_upgrade_once_and_follow_session_revocation() {
    let t = Test::new().await;
    let instance: Instance = t
        .http
        .get(format!("{}/instance", t.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(instance.instance_name, "Den");
    assert_eq!(instance.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        t.http
            .post(format!("{}/auth/ws-ticket", t.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let ticket: WsTicket =
        serde_json::from_value(t.post("/auth/ws-ticket", &t.admin.token, json!({})).await).unwrap();
    assert_eq!(ticket.expires_in, 30);
    let url = format!(
        "{}/ws?ticket={}",
        t.url.replace("http", "ws"),
        ticket.ticket
    );
    let (mut socket, _) = connect_async(&url).await.unwrap();
    assert!(connect_async(&url).await.is_err());
    socket.close(None).await.unwrap();
    let ticket = t.post("/auth/ws-ticket", &t.admin.token, json!({})).await;
    t.req(Method::POST, "/auth/logout", &t.admin.token)
        .send()
        .await
        .unwrap();
    assert!(connect_async(format!(
        "{}/ws?ticket={}",
        t.url.replace("http", "ws"),
        ticket["ticket"].as_str().unwrap()
    ))
    .await
    .is_err());
}

#[tokio::test]
async fn cookie_ticket_minting_retains_csrf_protection() {
    let t = Test::new().await;
    let req = || {
        t.http
            .post(format!("{}/auth/ws-ticket", t.url))
            .header("cookie", format!("den_session={}", t.admin.token))
    };
    assert_eq!(req().send().await.unwrap().status(), 403);
    assert_eq!(
        req()
            .header("origin", &t.url)
            .header("x-csrf-token", &t.admin.csrf_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}
