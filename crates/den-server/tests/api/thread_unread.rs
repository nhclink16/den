use super::realtime::socket;
use super::*;

async fn status(t: &Test, method: Method, path: &str, token: &str, body: Value) -> StatusCode {
    t.req(method, path, token)
        .json(&body)
        .send()
        .await
        .unwrap()
        .status()
}
fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}
async fn get<T: serde::de::DeserializeOwned>(t: &Test, path: &str, token: &str) -> T {
    let r = t.req(Method::GET, path, token).send().await.unwrap();
    assert_eq!(r.status(), 200, "{path}");
    r.json().await.unwrap()
}
/// This user's counts for one channel, as the server reports them.
async fn channel(t: &Test, token: &str, id: &str) -> ChannelReadState {
    let all: Vec<ChannelReadState> = get(t, "/users/me/read-state", token).await;
    all.into_iter()
        .find(|s| s.channel_id == id)
        .expect("channel")
}
async fn thread(t: &Test, token: &str, id: &str) -> ThreadReadState {
    get::<ThreadView>(t, &format!("/threads/{id}"), token)
        .await
        .read_state
}
async fn read_thread(t: &Test, token: &str, thread: &str, marker: &str) -> StatusCode {
    status(
        t,
        Method::PUT,
        &format!("/threads/{thread}/read"),
        token,
        json!({ "message_id": marker }),
    )
    .await
}
async fn read_channel(
    t: &Test,
    token: &str,
    channel: &str,
    marker: &str,
    roots: bool,
) -> StatusCode {
    status(
        t,
        Method::PUT,
        &format!("/channels/{channel}/read"),
        token,
        json!({"message_id": marker, "roots_only": roots}),
    )
    .await
}

#[tokio::test]
async fn reading_one_conversation_never_acknowledges_another() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    let a_root = id(&t
        .post(&path, &t.admin.token, json!({"content":"thread A"}))
        .await);
    let b_root = id(&t
        .post(&path, &t.admin.token, json!({"content":"thread B"}))
        .await);
    let a = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"a1","reply_to":a_root}),
        )
        .await;
    let b = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"b1","reply_to":b_root}),
        )
        .await;
    let (a_thread, b_thread) = (
        a["thread_id"].as_str().unwrap().to_string(),
        b["thread_id"].as_str().unwrap().to_string(),
    );
    // Bob follows both so they are relevant to him without a DM or subscription.
    for th in [&a_thread, &b_thread] {
        assert_eq!(
            status(
                &t,
                Method::PUT,
                &format!("/threads/{th}/follow"),
                &bob.token,
                json!({"following": true})
            )
            .await,
            200
        );
    }
    let later = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"a2","reply_to":a_root}),
        )
        .await;
    // B gets a reply after the Follow too. Without this both conversations cannot
    // be unread at once, and "reading A cleared B" would pass unnoticed.
    let b_later = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"b2","reply_to":b_root}),
        )
        .await;
    let newest_room = id(&t
        .post(&path, &t.admin.token, json!({"content":"room news"}))
        .await);
    t.post(&path, &bob.token, json!({"content":"mine"})).await;

    // Reading the newest room message clears the main conversation and leaves both
    // collapsed threads exactly as they were. Bob's own message never counted.
    assert_eq!(
        read_channel(&t, &bob.token, &cid, &newest_room, true).await,
        200
    );
    assert_eq!(thread(&t, &bob.token, &a_thread).await.unread_count, 1);
    assert_eq!(thread(&t, &bob.token, &b_thread).await.unread_count, 1);
    assert_eq!(
        channel(&t, &bob.token, &cid).await.unread_count,
        2,
        "the room read left both collapsed conversations unread"
    );

    // Reading thread A clears A alone. B is untouched and the total drops by one.
    assert_eq!(
        read_thread(&t, &bob.token, &a_thread, &id(&later)).await,
        200
    );
    assert_eq!(thread(&t, &bob.token, &a_thread).await.unread_count, 0);
    assert_eq!(
        thread(&t, &bob.token, &b_thread).await.unread_count,
        1,
        "reading one conversation must not acknowledge the other"
    );
    assert_eq!(channel(&t, &bob.token, &cid).await.unread_count, 1);

    // Reading B in turn clears the rest, so the aggregate is exactly its parts.
    assert_eq!(
        read_thread(&t, &bob.token, &b_thread, &id(&b_later)).await,
        200
    );
    assert_eq!(thread(&t, &bob.token, &b_thread).await.unread_count, 0);
    assert_eq!(channel(&t, &bob.token, &cid).await.unread_count, 0);

    // A task post is the one way to get a genuinely empty thread: a root with no
    // replies yet. Bob follows it while it is still empty, so his position is its
    // root and the thread is relevant to him.
    let job = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"deploy","task_id":"run-1"}),
        )
        .await;
    let empty = job["thread"]["id"].as_str().unwrap().to_string();
    let empty_root = id(&job);
    assert_eq!(
        status(
            &t,
            Method::PUT,
            &format!("/threads/{empty}/follow"),
            &bob.token,
            json!({"following": true})
        )
        .await,
        200
    );
    // Opening is a load and then a read at what was displayed. A reply arriving
    // between those two requests must survive the watermark, not be swallowed by it.
    let crossed = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"progress","task_id":"run-1"}),
        )
        .await;
    assert_eq!(read_thread(&t, &bob.token, &empty, &empty_root).await, 200);
    assert_eq!(
        thread(&t, &bob.token, &empty).await.unread_count,
        1,
        "the reply that crossed the empty-thread watermark must stay unread"
    );
    assert!(thread(&t, &bob.token, &empty).await.last_read_id.unwrap() < id(&crossed));

    // A captured flat Mark all read behaves the same way: it stops at the marker it
    // was given rather than at whatever the tail happens to be when it runs.
    let captured = id(&t
        .post(&path, &t.admin.token, json!({"content":"marker"}))
        .await);
    let crossing = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"later","task_id":"run-1"}),
        )
        .await;
    assert_eq!(
        read_channel(&t, &bob.token, &cid, &captured, false).await,
        200
    );
    assert_eq!(
        thread(&t, &bob.token, &empty).await.unread_count,
        1,
        "the flat read stopped at the captured marker, not the current tail"
    );
    assert!(thread(&t, &bob.token, &empty).await.last_read_id.unwrap() < id(&crossing));
}

#[tokio::test]
async fn affiliation_decides_relevance_without_moving_anybody_position() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let carol = t.member("carol").await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let first = t
        .post(&path, &bob.token, json!({"content":"r1","reply_to":root}))
        .await;
    let th = first["thread_id"].as_str().unwrap().to_string();

    // Replying enrolled bob at the previous tail, so his own reply does not count
    // and the root author sees the reply as new.
    assert!(thread(&t, &bob.token, &th).await.following);
    assert_eq!(thread(&t, &bob.token, &th).await.unread_count, 0);
    assert_eq!(thread(&t, &t.admin.token, &th).await.unread_count, 1);

    // Carol is not in the conversation at all: no row, no relevance, zero counts.
    let before = thread(&t, &carol.token, &th).await;
    assert!(!before.following && before.unread_count == 0 && before.last_read_id.is_none());

    let second = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"r2","thread_id":th}),
        )
        .await;
    // Posting does not advance an existing cursor: a CLI send is not evidence of
    // having read the messages in between.
    let held = thread(&t, &bob.token, &th).await.last_read_id;
    t.post(&path, &bob.token, json!({"content":"r3","thread_id":th}))
        .await;
    assert_eq!(thread(&t, &bob.token, &th).await.last_read_id, held);
    assert_eq!(
        thread(&t, &bob.token, &th).await.unread_count,
        1,
        "r2 is still unread"
    );

    // A passive read advances the position and must not follow.
    assert_eq!(read_thread(&t, &carol.token, &th, &id(&second)).await, 200);
    let read = thread(&t, &carol.token, &th).await;
    assert!(!read.following, "reading is not joining");
    assert_eq!(read.last_read_id.unwrap(), id(&second));
    // Explicit Follow starts now rather than replaying history.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            &format!("/threads/{th}/follow"),
            &carol.token,
            json!({"following": true})
        )
        .await,
        200
    );
    let followed = thread(&t, &carol.token, &th).await;
    assert!(followed.following && followed.unread_count == 0);
    // Unfollowing stops it counting but keeps where she had read to.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            &format!("/threads/{th}/follow"),
            &carol.token,
            json!({"following": false})
        )
        .await,
        200
    );
    let unfollowed = thread(&t, &carol.token, &th).await;
    assert!(!unfollowed.following && unfollowed.last_read_id.is_some());
    // Reading AFTER an explicit Unfollow must leave it unfollowed. Inspecting a
    // conversation is not a request to rejoin the one you just left.
    let newest = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"r4","thread_id":th}),
        )
        .await;
    assert_eq!(read_thread(&t, &carol.token, &th, &id(&newest)).await, 200);
    let after_read = thread(&t, &carol.token, &th).await;
    assert!(
        !after_read.following,
        "a passive read after Unfollow must not re-follow"
    );
    assert_eq!(after_read.last_read_id.unwrap(), id(&newest));

    // Resolving changes no position, and unread resolved threads still show up in
    // the unread view and still sum into the channel badge.
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{th}"),
            &t.admin.token,
            json!({"resolved": true})
        )
        .await,
        200
    );
    let admin_unread = thread(&t, &t.admin.token, &th).await.unread_count;
    assert!(admin_unread > 0, "resolving does not clear unread");
    let listed: Vec<ThreadView> = get(
        &t,
        &format!("/channels/{cid}/threads?unread_only=true"),
        &t.admin.token,
    )
    .await;
    assert!(
        listed.iter().any(|v| v.thread.id == th),
        "an unread resolved thread must be reachable without paging history"
    );
    assert_eq!(
        channel(&t, &t.admin.token, &cid).await.unread_count,
        listed
            .iter()
            .map(|v| v.read_state.unread_count)
            .sum::<i64>(),
        "the unread view and the channel badge use one predicate"
    );
}

#[tokio::test]
async fn subscribing_starts_now_and_never_erases_existing_unread() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    let quiet_root = id(&t
        .post(&path, &t.admin.token, json!({"content":"old"}))
        .await);
    let quiet = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"old reply","reply_to":quiet_root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    let joined_root = id(&t
        .post(&path, &t.admin.token, json!({"content":"joined"}))
        .await);
    let joined = t
        .post(
            &path,
            &bob.token,
            json!({"content":"mine","reply_to":joined_root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    t.post(
        &path,
        &t.admin.token,
        json!({"content":"after","thread_id":joined}),
    )
    .await;
    let held = thread(&t, &bob.token, &joined).await;
    assert_eq!(held.unread_count, 1);

    // A thread where bob already has a following=0 row from an earlier flat Mark all
    // read. Its saved position is real and older than the tail, so the subscription
    // must advance it with a NULL-safe maximum rather than ignore the existing row.
    let stale_root = id(&t
        .post(&path, &t.admin.token, json!({"content":"stale"}))
        .await);
    let stale = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"s1","reply_to":stale_root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    let flat_marker = id(&t
        .post(&path, &t.admin.token, json!({"content":"flat marker"}))
        .await);
    assert_eq!(
        read_channel(&t, &bob.token, &cid, &flat_marker, false).await,
        200
    );
    let stale_before = thread(&t, &bob.token, &stale).await;
    assert!(!stale_before.following && stale_before.last_read_id.is_some());
    t.post(
        &path,
        &t.admin.token,
        json!({"content":"s2","thread_id":stale}),
    )
    .await;
    // A resolved thread bob is not in, created AFTER the flat read above so that read
    // cannot have acknowledged it. Its reply is genuinely unread-if-relevant, which is
    // what makes the post-subscription assertion mean something: if the subscription
    // SQL skipped resolved threads, no row would appear at all.
    let done_root = id(&t
        .post(&path, &t.admin.token, json!({"content":"finished"}))
        .await);
    let done_reply = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"last word","reply_to":done_root}),
        )
        .await;
    let done = done_reply["thread_id"].as_str().unwrap().to_string();
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{done}"),
            &t.admin.token,
            json!({"resolved": true})
        )
        .await,
        200
    );
    // Bob has no row for it at all before subscribing.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM thread_read_state WHERE user_id=? AND thread_id=?"
        )
        .bind(&bob.user.id)
        .bind(&done)
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0,
        "no saved position for a resolved thread bob has never touched"
    );

    // That flat read acknowledged every thread in the channel, including the two set
    // up above. Give each fresh activity so they are back in the state this scenario
    // is actually about before the subscription happens.
    t.post(
        &path,
        &t.admin.token,
        json!({"content":"more history","thread_id":quiet}),
    )
    .await;
    t.post(
        &path,
        &t.admin.token,
        json!({"content":"still talking","thread_id":joined}),
    )
    .await;
    assert_eq!(
        thread(&t, &bob.token, &joined).await.unread_count,
        1,
        "the followed thread is unread again before subscribing"
    );

    // A DM whose replies are already relevant through membership alone. Subscribing
    // to it adds no relevance, so advancing its threads would destroy real unread.
    let dm = id(&t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await);
    let dm_path = format!("/channels/{dm}/messages");
    let dm_root = id(&t
        .post(&dm_path, &t.admin.token, json!({"content":"ours"}))
        .await);
    let dm_thread = t
        .post(
            &dm_path,
            &t.admin.token,
            json!({"content":"unread","reply_to":dm_root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    let dm_before = thread(&t, &bob.token, &dm_thread).await;
    assert_eq!(dm_before.unread_count, 1);

    // Subscribing starts previously irrelevant threads at their current tails, and
    // leaves a thread bob already follows exactly as it was.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            "/users/me/notification-preferences",
            &bob.token,
            json!({"mentions":true,"dms":true,"subscribed_channel_ids":[cid, dm]})
        )
        .await,
        200
    );
    assert_eq!(
        thread(&t, &bob.token, &quiet).await.unread_count,
        0,
        "subscribing is not a request to be told about history"
    );
    assert_eq!(
        thread(&t, &bob.token, &joined).await.unread_count,
        1,
        "a followed thread keeps its real unread"
    );
    // The resolved thread was materialized: a row now exists, reaching its actual
    // reply tail, and subscribing did not enrol bob as a follower.
    let done_after = thread(&t, &bob.token, &done).await;
    assert_eq!(
        done_after.last_read_id.as_deref(),
        Some(id(&done_reply).as_str()),
        "a resolved previously irrelevant thread starts at its real reply tail"
    );
    assert!(
        !done_after.following,
        "materializing a position is not joining the conversation"
    );
    assert_eq!(
        done_after.unread_count, 0,
        "and so it contributes nothing from before the subscription"
    );
    let stale_after = thread(&t, &bob.token, &stale).await;
    assert_eq!(
        stale_after.unread_count, 0,
        "an existing following=0 row advances rather than being ignored"
    );
    assert!(
        stale_after.last_read_id > stale_before.last_read_id,
        "its old flat position moved forward to the current tail"
    );
    let dm_after = thread(&t, &bob.token, &dm_thread).await;
    assert_eq!(
        dm_after.unread_count, 1,
        "a DM was already relevant, so subscribing must not erase its unread"
    );
    assert_eq!(
        dm_after.last_read_id, dm_before.last_read_id,
        "and must not move its position"
    );

    // Toggling an unrelated switch must not reset any position.
    let after_new = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"newer","thread_id":quiet}),
        )
        .await;
    assert_eq!(thread(&t, &bob.token, &quiet).await.unread_count, 1);
    assert_eq!(
        status(
            &t,
            Method::PUT,
            "/users/me/notification-preferences",
            &bob.token,
            json!({"mentions":false,"dms":true,"subscribed_channel_ids":[cid]})
        )
        .await,
        200
    );
    assert_eq!(
        thread(&t, &bob.token, &quiet).await.unread_count,
        1,
        "flipping mentions must not acknowledge anything"
    );
    assert!(thread(&t, &bob.token, &quiet).await.last_read_id.unwrap() < id(&after_new));

    // Unsubscribing removes relevance, so the count goes to zero while the saved
    // position survives for when it matters again.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            "/users/me/notification-preferences",
            &bob.token,
            json!({"mentions":false,"dms":true,"subscribed_channel_ids":[]})
        )
        .await,
        200
    );
    let dropped = thread(&t, &bob.token, &quiet).await;
    assert_eq!(dropped.unread_count, 0);
    assert!(
        dropped.last_read_id.is_some(),
        "position survives losing relevance"
    );
}

#[tokio::test]
async fn a_thread_mention_notifies_after_the_room_was_read_and_only_once() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    // A DM bob also subscribes to, so a mention there qualifies three ways at once:
    // mention, DM membership and channel subscription. It must still be one alert.
    let cid = id(&t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await);
    assert_eq!(
        status(
            &t,
            Method::PUT,
            "/users/me/notification-preferences",
            &bob.token,
            json!({"mentions":true,"dms":true,"subscribed_channel_ids":[cid]})
        )
        .await,
        200
    );
    let path = format!("/channels/{cid}/messages");
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let reply = t
        .post(&path, &bob.token, json!({"content":"r1","reply_to":root}))
        .await;
    let th = reply["thread_id"].as_str().unwrap().to_string();
    // Bob reads the whole room flat, so his channel cursor is past everything.
    let tail = id(&t
        .post(&path, &t.admin.token, json!({"content":"room"}))
        .await);
    assert_eq!(read_channel(&t, &bob.token, &cid, &tail, false).await, 200);
    assert_eq!(channel(&t, &bob.token, &cid).await.unread_count, 0);

    let mut watcher = socket(&t, &bob.token).await;
    loop {
        if let Frame::Text(v) = watcher.next().await.unwrap().unwrap() {
            if matches!(
                serde_json::from_str::<Event>(&v).unwrap(),
                Event::Resync { .. }
            ) {
                break;
            }
        }
    }
    // The mention arrives after bob has already read the room flat. It is newer than
    // his channel cursor, so this is positive delivery rather than a reproduction of
    // the old channel-cursor gate: IDs are monotonic and send holds the write lock
    // through notification, so a mention can never be older than the cursor.
    let mention = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"@bob look","thread_id":th}),
        )
        .await;
    // The alert is emitted after the message event, so the barrier has to be a
    // later message: reaching it proves any notification already arrived.
    let barrier = t
        .post(&path, &t.admin.token, json!({"content":"barrier"}))
        .await;
    let mut alerts = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), watcher.next())
            .await
            .expect("event")
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                Event::Notification {
                    message, reason, ..
                } if message.id == id(&mention) => {
                    assert_eq!(message.thread_id.as_deref(), Some(th.as_str()));
                    alerts.push(reason);
                }
                Event::MessageCreated(m) if m.id == id(&barrier) => break,
                _ => {}
            },
            Frame::Ping(v) => watcher.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    // Three eligible reasons, one alert, and mention takes precedence over both.
    assert_eq!(
        alerts,
        vec![NotificationReason::Mention],
        "a message qualifying several ways must alert exactly once, as a mention"
    );
    assert_eq!(thread(&t, &bob.token, &th).await.mention_count, 1);
    assert_eq!(
        thread(&t, &bob.token, &th).await.notification_count,
        1,
        "overlapping reasons still count once"
    );
    assert_eq!(channel(&t, &bob.token, &cid).await.mention_count, 1);
}

#[tokio::test]
async fn a_subscriber_with_no_saved_row_still_receives_its_states() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    // Bob subscribes before the thread exists, so he never gets a row of his own.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            "/users/me/notification-preferences",
            &bob.token,
            json!({"mentions":true,"dms":true,"subscribed_channel_ids":[cid]})
        )
        .await,
        200
    );
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let mut watcher = socket(&t, &bob.token).await;
    loop {
        if let Frame::Text(v) = watcher.next().await.unwrap().unwrap() {
            if matches!(
                serde_json::from_str::<Event>(&v).unwrap(),
                Event::Resync { .. }
            ) {
                break;
            }
        }
    }
    let reply = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"r1","reply_to":root}),
        )
        .await;
    let th = reply["thread_id"].as_str().unwrap().to_string();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM thread_read_state WHERE user_id=? AND thread_id=?"
        )
        .bind(&bob.user.id)
        .bind(&th)
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0,
        "no row was created for a subscriber who did nothing"
    );

    // Creation, then deletion of the only reply. Both must reach him with matching
    // thread and channel counts, and deletion must reach zero rather than losing
    // the thread handle along with the message.
    let created = collect(&mut watcher, &t, &path, &th).await;
    assert_eq!(
        created.0, 1,
        "reply creation reaches a subscriber who has no saved row"
    );
    // Editing that reply to add a mention refreshes the counts a client holds, and
    // must NOT alert: the existing policy is that edits never acquire a new push,
    // and the mention indexer also runs for edits and startup backfill.
    let edited = edit_and_watch(
        &mut watcher,
        &t,
        &path,
        &th,
        &id(&reply),
        "@bob now you are named",
    )
    .await;
    assert_eq!(edited.mentions, 1, "the edit refreshed the mention count");
    assert!(!edited.alerted, "an edit must not raise a new notification");
    // Taking the mention back out refreshes it down again, still silently.
    let reverted = edit_and_watch(
        &mut watcher,
        &t,
        &path,
        &th,
        &id(&reply),
        "never mind, no name here",
    )
    .await;
    assert_eq!(
        reverted.mentions, 0,
        "removing a mention refreshes downward"
    );
    assert!(!reverted.alerted, "and still raises nothing");

    assert_eq!(
        t.req(
            Method::DELETE,
            &format!("/messages/{}", id(&reply)),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    let deleted = collect(&mut watcher, &t, &path, &th).await;
    assert_eq!(
        deleted.0, 0,
        "deleting the last reply emits the new zero count rather than losing the thread"
    );
    // The aggregate this subscriber was pushed agrees with what the server will
    // tell him if he asks. Counting barriers would only test the fixture.
    assert_eq!(
        deleted.1,
        channel(&t, &bob.token, &cid).await.unread_count,
        "the channel total on the wire matches the one over HTTP"
    );
    assert_eq!(
        thread(&t, &bob.token, &th).await.unread_count,
        0,
        "and HTTP agrees the emptied conversation is at zero"
    );
}

/// Drain to a fresh room-message barrier, returning the last (thread, channel)
/// unread counts seen for this user on the way.
async fn collect(
    socket: &mut super::realtime::Socket,
    t: &Test,
    path: &str,
    thread: &str,
) -> (i64, i64) {
    let marker = t
        .post(path, &t.admin.token, json!({"content":"barrier"}))
        .await;
    let mut th = -1;
    let mut ch;
    // The barrier's own state events follow its MessageCreated, so seeing the
    // message is not far enough: stop on the read state that message produced, or
    // the totals collected here would be one message behind what HTTP reports.
    let mut arrived = false;
    loop {
        match tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("event")
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                Event::ThreadReadStateUpdated { state, .. } if state.thread_id == thread => {
                    th = state.unread_count
                }
                Event::ReadStateUpdated { state, .. } => {
                    ch = state.unread_count;
                    if arrived {
                        break;
                    }
                }
                Event::MessageCreated(m) if m.id == id(&marker) => arrived = true,
                _ => {}
            },
            Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    (th, ch)
}

/// Drain both of one user's sockets to a shared barrier, returning the last state
/// each saw for the two keys under test. Returning whole states rather than one
/// number keeps the caller free to assert whichever field its scenario is about.
async fn both(
    a: &mut super::realtime::Socket,
    b: &mut super::realtime::Socket,
    t: &Test,
    barrier_path: &str,
    thread: &str,
    channel: &str,
) -> Vec<(Option<ThreadReadState>, Option<ChannelReadState>)> {
    let marker = t
        .post(barrier_path, &t.admin.token, json!({"content":"barrier"}))
        .await;
    let mut out = Vec::new();
    for socket in [a, b] {
        let (mut th, mut ch) = (None, None);
        // Stop on the read state the barrier itself produced, so everything the
        // action under test emitted has certainly been drained first.
        let mut arrived = false;
        loop {
            match tokio::time::timeout(Duration::from_secs(5), socket.next())
                .await
                .expect("event")
                .unwrap()
                .unwrap()
            {
                Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                    Event::ThreadReadStateUpdated { state, .. } if state.thread_id == thread => {
                        th = Some(state)
                    }
                    Event::ReadStateUpdated { state, .. } => {
                        if state.channel_id == channel {
                            ch = Some(state);
                        }
                        if arrived {
                            break;
                        }
                    }
                    Event::MessageCreated(m) if m.id == id(&marker) => arrived = true,
                    _ => {}
                },
                Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
                _ => {}
            }
        }
        out.push((th, ch));
    }
    out
}

#[tokio::test]
async fn a_preference_change_refreshes_every_conversation_it_silences() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let public = t.general().await;
    let public_path = format!("/channels/{public}/messages");
    let dm = id(&t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await);
    let dm_path = format!("/channels/{dm}/messages");
    let root = id(&t
        .post(&dm_path, &t.admin.token, json!({"content":"ours"}))
        .await);
    t.post(
        &dm_path,
        &t.admin.token,
        json!({"content":"unread reply","reply_to":root}),
    )
    .await;
    let th = sqlx::query_scalar::<_, String>("SELECT id FROM threads WHERE root_message_id=?")
        .bind(&root)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    // A DM reply bob has not read: relevant through membership, and notifying
    // because the dms preference is on by default.
    let before = thread(&t, &bob.token, &th).await;
    assert_eq!(before.unread_count, 1);
    assert_eq!(before.notification_count, 1);

    let mut one = socket(&t, &bob.token).await;
    let mut two = socket(&t, &bob.token).await;
    for s in [&mut one, &mut two] {
        loop {
            if let Frame::Text(v) = s.next().await.unwrap().unwrap() {
                if matches!(
                    serde_json::from_str::<Event>(&v).unwrap(),
                    Event::Resync { .. }
                ) {
                    break;
                }
            }
        }
    }
    // Only the dms switch moves; the subscription set is untouched. That silences
    // this conversation, so every device holding its counts has to be told.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            "/users/me/notification-preferences",
            &bob.token,
            json!({"mentions":true,"dms":false,"subscribed_channel_ids":[]})
        )
        .await,
        200
    );
    let seen = both(&mut one, &mut two, &t, &public_path, &th, &dm).await;
    for (i, (th_state, ch_state)) in seen.iter().enumerate() {
        let th_state = th_state
            .as_ref()
            .unwrap_or_else(|| panic!("socket {i} never received the silenced thread's state"));
        assert_eq!(
            th_state.notification_count, 0,
            "socket {i} still shows the silenced thread as notifying"
        );
        assert_eq!(
            ch_state
                .as_ref()
                .unwrap_or_else(|| panic!("socket {i} never received the channel state"))
                .notification_count,
            0,
            "socket {i} channel notification count"
        );
    }
    // Silencing changes what notifies, never what is unread or where anyone read to.
    let after = thread(&t, &bob.token, &th).await;
    assert_eq!(after.notification_count, 0, "HTTP agrees with the events");
    assert_eq!(after.unread_count, before.unread_count, "still unread");
    assert_eq!(
        after.last_read_id, before.last_read_id,
        "position untouched"
    );
}

/// The real upgrade path: run migrations up to 19, populate the database the way a
/// live phase-2a server would have left it, then run 0020 through the actual
/// migrator and inspect what it did. The traps this has to prove are a saved NULL
/// position (SQLite's max(NULL,x) is NULL, which would blank it), a later saved
/// position that must win, a resolved thread that must still be covered, and
/// affiliations that must survive untouched.
#[tokio::test]
async fn the_read_position_migration_carries_flat_knowledge_forward() {
    let t = Test::new().await;
    let path = t.dir.join("upgrade.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let db = sqlx::SqlitePool::connect_with(options.clone())
        .await
        .unwrap();
    let before = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(sqlx::migrate!().iter().take(19).cloned().collect()),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    assert_eq!(before.iter().last().unwrap().version, 19);
    before.run(&db).await.unwrap();
    for sql in [
        "INSERT INTO users(id,username,display_name,role) VALUES('u1','ada','Ada','admin'),('u2','bo','Bo','member'),('u3','cy','Cy','member')",
        "INSERT INTO channels(id,name,kind,position) VALUES('c1','general','text',0)",
        "INSERT INTO messages(id,channel_id,author_id,content) VALUES('m100','c1','u1','open root'),('m200','c1','u1','resolved root')",
        "INSERT INTO threads(id,channel_id,root_message_id,title,created_by) VALUES('t1','c1','m100','open','u1'),('t2','c1','m200','resolved','u1')",
        "UPDATE threads SET resolved_at='2026-09-15T10:00:00.000Z',resolved_by='u1' WHERE id='t2'",
        "INSERT INTO messages(id,channel_id,author_id,content,thread_id) VALUES('m300','c1','u2','r1','t1'),('m400','c1','u2','r2','t2')",
        // Three shapes of flat reader: ada has read to m350, bo further back, cy ahead.
        "INSERT INTO read_state(user_id,channel_id,last_read_id) VALUES('u1','c1','m350'),('u2','c1','m150'),('u3','c1','m500')",
        // ada follows t1 with NO saved position, and has an already-later position
        // in the resolved t2 while not following it.
        "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following) VALUES('u1','t1',NULL,1),('u1','t2','m900',0)",
        // cy follows t1 from further back than the channel marker.
        "INSERT INTO thread_read_state(user_id,thread_id,last_read_id,following) VALUES('u3','t1','m050',1)",
        // bo has no thread rows at all.
    ] {
        sqlx::query(sql).execute(&db).await.unwrap();
    }
    db.close().await;

    let db = sqlx::SqlitePool::connect_with(options).await.unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    let rows = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
        "SELECT user_id,thread_id,last_read_id,following FROM thread_read_state ORDER BY user_id,thread_id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    let at = |user: &str, thread: &str| {
        rows.iter()
            .find(|r| r.0 == user && r.1 == thread)
            .map(|r| (r.2.clone(), r.3))
    };
    // A saved NULL takes the channel marker rather than being blanked, and keeps
    // the affiliation it already had.
    assert_eq!(at("u1", "t1"), Some((Some("m350".into()), true)));
    // A later saved position wins over the channel marker.
    assert_eq!(at("u1", "t2"), Some((Some("m900".into()), false)));
    // A reader with no rows gets every thread of the channel, resolved included,
    // and those new rows are explicitly not following.
    assert_eq!(at("u2", "t1"), Some((Some("m150".into()), false)));
    assert_eq!(at("u2", "t2"), Some((Some("m150".into()), false)));
    // An earlier saved position advances, and following survives.
    assert_eq!(at("u3", "t1"), Some((Some("m500".into()), true)));
    assert_eq!(at("u3", "t2"), Some((Some("m500".into()), false)));
    // The channel cursor itself is untouched: its meaning changes, its value does not.
    let channel = sqlx::query_as::<_, (String, String)>(
        "SELECT user_id,last_read_id FROM read_state ORDER BY user_id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        channel,
        vec![
            ("u1".to_string(), "m350".to_string()),
            ("u2".to_string(), "m150".to_string()),
            ("u3".to_string(), "m500".to_string())
        ]
    );
    // The messages themselves are untouched: same rows, same ids, same content, same
    // thread placement. A read-position migration has no business rewriting history.
    let messages = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id,content,thread_id FROM messages ORDER BY id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        messages,
        vec![
            ("m100".to_string(), "open root".to_string(), None),
            ("m200".to_string(), "resolved root".to_string(), None),
            ("m300".to_string(), "r1".to_string(), Some("t1".to_string())),
            ("m400".to_string(), "r2".to_string(), Some("t2".to_string())),
        ]
    );
    db.close().await;
}

/// An old client sends the pre-threads body with no roots_only key at all. That is
/// the shape that actually appears during a mixed-version rollout, and it has to
/// keep meaning a flat acknowledgement of both scopes.
#[tokio::test]
async fn an_old_client_body_without_roots_only_still_reads_flat() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let reply = t
        .post(&path, &bob.token, json!({"content":"r1","reply_to":root}))
        .await;
    let th = reply["thread_id"].as_str().unwrap().to_string();
    let newer = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"r2","thread_id":th}),
        )
        .await;
    assert_eq!(thread(&t, &bob.token, &th).await.unread_count, 1);

    // The literal old body: message_id and nothing else.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            &format!("/channels/{cid}/read"),
            &bob.token,
            json!({ "message_id": id(&newer) })
        )
        .await,
        200
    );
    assert_eq!(
        thread(&t, &bob.token, &th).await.unread_count,
        0,
        "an omitted roots_only is a flat read and acknowledges the thread too"
    );
    assert_eq!(channel(&t, &bob.token, &cid).await.unread_count, 0);

    // Now a further reply lands in the thread and a fresh room message arrives. The
    // new client reads the room at that room marker with roots_only=true, which
    // leaves the collapsed reply alone.
    let later = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"r3","thread_id":th}),
        )
        .await;
    let room = id(&t
        .post(&path, &t.admin.token, json!({"content":"room"}))
        .await);
    assert_eq!(read_channel(&t, &bob.token, &cid, &room, true).await, 200);
    assert_eq!(
        thread(&t, &bob.token, &th).await.unread_count,
        1,
        "roots_only left the hidden reply unread"
    );
    assert!(thread(&t, &bob.token, &th).await.last_read_id.unwrap() < id(&later));
}

#[tokio::test]
async fn two_devices_converge_and_a_nonmember_learns_nothing() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let outsider = t.member("outsider").await;
    let public = t.general().await;
    let public_path = format!("/channels/{public}/messages");
    let dm = id(&t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await);
    let dm_path = format!("/channels/{dm}/messages");
    // The root stays unread throughout. It is what proves the two scopes are
    // separate: reading the conversation must not acknowledge the room around it.
    let dm_root = id(&t
        .post(&dm_path, &t.admin.token, json!({"content":"ours"}))
        .await);
    let dm_reply = t
        .post(
            &dm_path,
            &t.admin.token,
            json!({"content":"in the thread","reply_to":dm_root}),
        )
        .await;
    let private = dm_reply["thread_id"].as_str().unwrap().to_string();

    // A nonmember gets nothing from any read-model route, and cannot tell a
    // forbidden conversation from a missing one.
    for (method, route, body) in [
        (Method::GET, format!("/channels/{dm}/threads"), json!({})),
        (Method::GET, format!("/threads/{private}"), json!({})),
        (
            Method::PUT,
            format!("/threads/{private}/read"),
            json!({ "message_id": dm_root }),
        ),
        (
            Method::PUT,
            format!("/threads/{private}/follow"),
            json!({"following": true}),
        ),
    ] {
        assert_eq!(
            status(&t, method, &route, &outsider.token, body).await,
            404,
            "{route} leaked to a nonmember"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM thread_read_state WHERE user_id=? AND thread_id=?"
        )
        .bind(&outsider.user.id)
        .bind(&private)
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0,
        "a refused read or follow must not create a row"
    );

    // Every socket connects BEFORE the read, so the outsider's window actually
    // contains private activity to have leaked.
    let mut one = socket(&t, &bob.token).await;
    let mut two = socket(&t, &bob.token).await;
    let mut stranger = socket(&t, &outsider.token).await;
    for s in [&mut one, &mut two, &mut stranger] {
        loop {
            if let Frame::Text(v) = s.next().await.unwrap().unwrap() {
                if matches!(
                    serde_json::from_str::<Event>(&v).unwrap(),
                    Event::Resync { .. }
                ) {
                    break;
                }
            }
        }
    }

    // One device reads the private conversation. The barrier is in the public
    // channel, so draining cannot itself change the DM totals being asserted.
    assert_eq!(
        read_thread(&t, &bob.token, &private, &id(&dm_reply)).await,
        200
    );
    let seen = both(&mut one, &mut two, &t, &public_path, &private, &dm).await;
    let http_thread = thread(&t, &bob.token, &private).await;
    let http_channel = channel(&t, &bob.token, &dm).await;
    for (i, (th_state, ch_state)) in seen.iter().enumerate() {
        let th_state = th_state
            .as_ref()
            .unwrap_or_else(|| panic!("device {i} never saw the conversation it read"));
        let ch_state = ch_state
            .as_ref()
            .unwrap_or_else(|| panic!("device {i} never saw the channel total"));
        assert_eq!(th_state.unread_count, 0, "device {i} thread unread");
        assert_eq!(
            th_state.notification_count, 0,
            "device {i} thread notification count"
        );
        assert_eq!(
            th_state.last_read_id.as_deref(),
            Some(id(&dm_reply).as_str()),
            "device {i} position"
        );
        // The DM root is still unread, and a DM notifies by default, so the channel
        // is 1 while the conversation is 0. That difference is the whole point.
        assert_eq!(ch_state.unread_count, 1, "device {i} channel unread");
        assert_eq!(
            ch_state.notification_count, 1,
            "device {i} channel notification count"
        );
        // Both devices and HTTP tell the same story.
        assert_eq!(th_state, &http_thread, "device {i} disagrees with HTTP");
        assert_eq!(ch_state, &http_channel, "device {i} channel disagrees");
    }

    // The outsider watched that whole exchange and must have learned nothing of it.
    let marker = t
        .post(&public_path, &t.admin.token, json!({"content":"public"}))
        .await;
    let mut leaked = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stranger.next())
            .await
            .expect("event")
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                Event::ThreadReadStateUpdated { user_id, state } => {
                    leaked.push((user_id, state.thread_id))
                }
                Event::ReadStateUpdated { user_id, state } => {
                    leaked.push((user_id, state.channel_id))
                }
                Event::MessageCreated(m) if m.id == id(&marker) => break,
                _ => {}
            },
            Frame::Ping(v) => stranger.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    assert!(
        leaked.iter().all(|(u, _)| u == &outsider.user.id),
        "somebody else's personal read state reached an outsider: {leaked:?}"
    );
    assert!(
        leaked.iter().all(|(_, key)| key != &private && key != &dm),
        "a private conversation was named to an outsider: {leaked:?}"
    );
}

struct Edited {
    mentions: i64,
    alerted: bool,
}
/// Edit one message and drain to a fresh barrier, reporting the thread's refreshed
/// mention count and whether any notification was raised on the way.
async fn edit_and_watch(
    socket: &mut super::realtime::Socket,
    t: &Test,
    path: &str,
    thread: &str,
    message: &str,
    content: &str,
) -> Edited {
    assert_eq!(
        t.req(
            Method::PATCH,
            &format!("/messages/{message}"),
            &t.admin.token
        )
        .json(&json!({ "content": content }))
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    let marker = t
        .post(path, &t.admin.token, json!({"content":"barrier"}))
        .await;
    let (mut mentions, mut alerted) = (-1, false);
    loop {
        match tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("event")
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                Event::ThreadReadStateUpdated { state, .. } if state.thread_id == thread => {
                    mentions = state.mention_count
                }
                // Only an alert for the edited message counts. The barrier this
                // helper posts is an ordinary new message and may notify legitimately.
                Event::Notification { message: m, .. } if m.id == message => alerted = true,
                Event::MessageCreated(m) if m.id == id(&marker) => break,
                _ => {}
            },
            Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    Edited { mentions, alerted }
}
