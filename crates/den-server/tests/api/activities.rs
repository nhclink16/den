use super::realtime::{socket, Socket};
use super::*;

async fn activity_event(socket: &mut Socket, user: &str) -> Vec<Activity> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Frame::Text(v) => {
                    if let Event::ActivityUpdated {
                        user_id,
                        activities,
                    } = serde_json::from_str(&v).unwrap()
                    {
                        if user_id == user {
                            return activities;
                        }
                    }
                }
                Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
                _ => {}
            }
        }
    })
    .await
    .expect("an activity_updated event for this user")
}

async fn put(t: &Test, token: &str, path: &str, body: Value) -> reqwest::Response {
    t.req(Method::PUT, path, token)
        .json(&body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn own_activity_reaches_everyone_and_presence() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let mut watcher = socket(&t, &bob.token).await;

    let r = put(
        &t,
        &alice.token,
        "/users/me/activities/desktop",
        json!({"kind":"playing","name":"  Minecraft ","details":"on the den"}),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    let mine: UserActivities = r.json().await.unwrap();
    assert_eq!(mine.user_id, alice.user.id);
    assert_eq!(mine.activities[0].name, "Minecraft", "names are trimmed");

    let seen = activity_event(&mut watcher, &alice.user.id).await;
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].slot.as_str(), seen[0].kind),
        ("desktop", ActivityKind::Playing)
    );

    let presence: PresenceState = t
        .req(Method::GET, "/presence", &bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let listed = presence
        .activities
        .iter()
        .find(|u| u.user_id == alice.user.id)
        .expect("in presence");
    assert_eq!(listed.activities[0].details.as_deref(), Some("on the den"));

    // A second source stacks beside the first rather than replacing it.
    put(
        &t,
        &alice.token,
        "/users/me/activities/cli",
        json!({"kind":"working","name":"Backups"}),
    )
    .await;
    assert_eq!(activity_event(&mut watcher, &alice.user.id).await.len(), 2);

    let r = t
        .req(Method::DELETE, "/users/me/activities/desktop", &alice.token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let left = activity_event(&mut watcher, &alice.user.id).await;
    assert_eq!(
        left.iter().map(|a| a.slot.as_str()).collect::<Vec<_>>(),
        ["cli"]
    );
}

#[tokio::test]
async fn only_you_or_an_admin_can_set_your_activity() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let path = format!("/users/{}/activities/minecraft", alice.user.id);
    let body = json!({"kind":"playing","name":"Minecraft"});

    assert_eq!(
        put(&t, &bob.token, &path, body.clone()).await.status(),
        StatusCode::FORBIDDEN
    );
    let r = put(&t, &t.admin.token, &path, body.clone()).await;
    assert_eq!(
        r.status(),
        StatusCode::OK,
        "the Minecraft relay runs as an admin"
    );
    let set: UserActivities = r.json().await.unwrap();
    assert_eq!(set.user_id, alice.user.id);

    assert_eq!(
        t.req(Method::DELETE, &path, &bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let nobody = put(
        &t,
        &t.admin.token,
        "/users/01NOBODY/activities/minecraft",
        body,
    )
    .await;
    assert_eq!(nobody.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn slots_and_text_are_checked() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let ok = json!({"kind":"using","name":"Blender"});
    for slot in ["spotify", "Desktop", "a%20b"] {
        let r = put(
            &t,
            &alice.token,
            &format!("/users/me/activities/{slot}"),
            ok.clone(),
        )
        .await;
        assert_eq!(r.status(), StatusCode::BAD_REQUEST, "slot {slot}");
    }
    for body in [
        json!({"kind":"using","name":""}),
        json!({"kind":"using","name":"two\nlines"}),
        json!({"kind":"using","name":"x","ttl_seconds":5}),
        json!({"kind":"dancing","name":"x"}),
    ] {
        let r = put(
            &t,
            &alice.token,
            "/users/me/activities/desktop",
            body.clone(),
        )
        .await;
        assert!(r.status().is_client_error(), "{body}");
    }
}

#[tokio::test]
async fn refreshing_keeps_the_start_and_expiry_removes_it() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let mut watcher = socket(&t, &t.admin.token).await;
    let body = json!({"kind":"playing","name":"Minecraft","ttl_seconds":10});
    let first: UserActivities = put(
        &t,
        &alice.token,
        "/users/me/activities/desktop",
        body.clone(),
    )
    .await
    .json()
    .await
    .unwrap();
    activity_event(&mut watcher, &alice.user.id).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let again: UserActivities = put(&t, &alice.token, "/users/me/activities/desktop", body)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(
        again.activities[0].started_at, first.activities[0].started_at,
        "a refresh is not a new session"
    );
    assert!(again.activities[0].expires_at > first.activities[0].expires_at);

    // Pretend the source went quiet past its deadline.
    t.state
        .expire_activities_at(again.activities[0].expires_at + 1);
    assert!(activity_event(&mut watcher, &alice.user.id)
        .await
        .is_empty());
    let all: Vec<UserActivities> = t
        .req(Method::GET, "/activities", &alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(all.iter().all(|u| u.user_id != alice.user.id));
}
