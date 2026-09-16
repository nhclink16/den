use super::realtime::socket;
use super::*;

/// Status code of a request, so a rejection is asserted by its code rather than by
/// `post`'s blanket 200.
async fn status(t: &Test, method: Method, path: &str, token: &str, body: Value) -> StatusCode {
    t.req(method, path, token)
        .json(&body)
        .send()
        .await
        .unwrap()
        .status()
}
async fn ids(t: &Test, path: &str, token: &str) -> Vec<String> {
    let v: Vec<Message> = t
        .req(Method::GET, path, token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    v.into_iter().map(|m| m.id).collect()
}
fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}
/// Shut the server down, release the database, and serve the same directory again
/// from a fresh AppState. A fact that survives this is stored, not remembered. The
/// original server and pool are gone afterwards, so this goes last in a test and
/// every later assertion goes through the returned URL.
async fn restart(t: &mut Test) -> (String, tokio::task::JoinHandle<()>) {
    t.stop_for_export().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let state = AppState::open(
        t.dir.join("den.db"),
        t.dir.join("uploads"),
        t.dir.join("bootstrap.key"),
        url.clone(),
        1024 * 1024,
    )
    .await
    .unwrap();
    let app = den_server::router_with_web(state, t.dir.join("spa"));
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    (url, task)
}

/// A reply resolves its destination and inserts inside one transaction, while
/// every authenticated request writes through profile expiry outside the message
/// write lock. Both have to proceed: a reply must not fail because ordinary reads
/// were in flight. Real HTTP and real authentication, a short concurrent burst,
/// no sleeps and no direct SQL.
#[tokio::test]
async fn authenticated_reads_do_not_interrupt_thread_replies() {
    const PAIRS: usize = 32;
    const READS: usize = 128;
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let replies = async {
        for pair in 0..PAIRS {
            let root = id(&t
                .post_phase(
                    "root",
                    &path,
                    &t.admin.token,
                    json!({"content":"why is the build red"}),
                )
                .await);
            let (one, two) = tokio::join!(
                t.post_phase(
                    "first-reply-a",
                    &path,
                    &t.admin.token,
                    json!({"content":"a","reply_to":root})
                ),
                t.post_phase(
                    "first-reply-b",
                    &path,
                    &bob.token,
                    json!({"content":"b","reply_to":root})
                ),
            );
            let thread = one["thread_id"].as_str().unwrap();
            assert_eq!(
                two["thread_id"].as_str().unwrap(),
                thread,
                "pair {pair} split one root across two threads"
            );
        }
    };
    let admin_reads = async {
        for _ in 0..READS {
            let status = t
                .req(Method::GET, "/users/me", &t.admin.token)
                .send()
                .await
                .unwrap()
                .status();
            assert_eq!(status, StatusCode::OK, "GET /users/me");
        }
    };
    let bob_reads = async {
        for _ in 0..READS {
            let status = t
                .req(Method::GET, "/users/me", &bob.token)
                .send()
                .await
                .unwrap()
                .status();
            assert_eq!(status, StatusCode::OK, "GET /users/me");
        }
    };
    tokio::time::timeout(Duration::from_secs(15), async {
        tokio::join!(replies, admin_reads, bob_reads);
    })
    .await
    .expect("replies and authenticated reads did not finish within 15s");
}

#[tokio::test]
async fn a_first_reply_opens_one_thread_and_replies_never_nest() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let root = id(&t
        .post_phase(
            "root",
            &path,
            &t.admin.token,
            json!({"content":"why is the build red"}),
        )
        .await);
    // Two first replies at once. Whichever lands second must join the thread the
    // first one opened rather than starting a second conversation on the same root.
    let (one, two) = tokio::join!(
        t.post_phase(
            "first-reply-a",
            &path,
            &t.admin.token,
            json!({"content":"a","reply_to":root})
        ),
        t.post_phase(
            "first-reply-b",
            &path,
            &bob.token,
            json!({"content":"b","reply_to":root})
        ),
    );
    let thread = one["thread_id"].as_str().unwrap().to_string();
    assert_eq!(two["thread_id"].as_str().unwrap(), thread);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM threads WHERE root_message_id=?")
            .bind(&root)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        1
    );
    // Replying to a reply stays in the same thread. There is no second level.
    let deep = t
        .post_phase(
            "nested-reply",
            &path,
            &bob.token,
            json!({"content":"c","reply_to":id(&one)}),
        )
        .await;
    assert_eq!(deep["thread_id"].as_str().unwrap(), thread);
    // The root itself stays in the main conversation and carries the summary.
    let fetched: Message = t
        .req(Method::GET, &format!("/messages/{root}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(fetched.thread_id.is_none());
    let summary = fetched.thread.unwrap();
    assert_eq!(summary.id, thread);
    assert_eq!(summary.reply_count, 3);
    assert_eq!(summary.title, "why is the build red");
    assert_eq!(summary.last_reply_id.unwrap(), id(&deep));

    let replies = vec![id(&one), id(&two), id(&deep)];
    let mut flat = replies.clone();
    flat.push(root.clone());
    flat.sort();
    assert_eq!(ids(&t, &path, &t.admin.token).await, flat);
    assert_eq!(
        ids(&t, &format!("{path}?roots_only=true"), &t.admin.token).await,
        vec![root.clone()]
    );
    let mut sorted = replies.clone();
    sorted.sort();
    assert_eq!(
        ids(&t, &format!("/threads/{thread}/messages"), &t.admin.token).await,
        sorted
    );
    // An object-only root is named by its own card. The reply that opens the thread
    // has text of its own, and that text must not be allowed to name the root.
    let card = t
        .post_phase(
            "canvas-card",
            &format!("/channels/{channel}/objects"),
            &t.admin.token,
            json!({"kind":"canvas","name":"Sprint board","state":{}}),
        )
        .await;
    let card_root = card["message_id"].as_str().unwrap().to_string();
    let card_reply = t
        .post_phase(
            "canvas-reply",
            &path,
            &bob.token,
            json!({"content":"moved the last column","reply_to":card_root}),
        )
        .await;
    let card_thread = card_reply["thread_id"].as_str().unwrap().to_string();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM threads WHERE id=?")
            .bind(&card_thread)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        "Sprint board"
    );
    // Paging a thread follows the same before/after/limit rules as the channel list,
    // with the same stable ascending page order.
    assert_eq!(
        ids(
            &t,
            &format!("/threads/{thread}/messages?limit=1"),
            &t.admin.token
        )
        .await,
        vec![sorted[2].clone()]
    );
    assert_eq!(
        ids(
            &t,
            &format!("/threads/{thread}/messages?before={}", sorted[2]),
            &t.admin.token
        )
        .await,
        vec![sorted[0].clone(), sorted[1].clone()]
    );
    assert_eq!(
        ids(
            &t,
            &format!("/threads/{thread}/messages?after={}", sorted[0]),
            &t.admin.token
        )
        .await,
        vec![sorted[1].clone(), sorted[2].clone()]
    );
    assert_eq!(
        ids(
            &t,
            &format!("/threads/{thread}/messages?before={}&limit=1", sorted[2]),
            &t.admin.token
        )
        .await,
        vec![sorted[1].clone()]
    );
    // roots_only is not a narrower view of a thread; it is the wrong endpoint.
    assert_eq!(
        status(
            &t,
            Method::GET,
            &format!("/threads/{thread}/messages?roots_only=true"),
            &t.admin.token,
            json!({})
        )
        .await,
        400
    );
}

#[tokio::test]
async fn refused_placement_leaves_no_rows_and_private_threads_stay_private() {
    let t = Test::new().await;
    let alice = t.member("alice").await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let other = id(&t
        .post(
            "/channels",
            &t.admin.token,
            json!({"name":"other","category_id":null,"position":9}),
        )
        .await);
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"root"}))
        .await);
    let elsewhere = id(&t
        .post(
            &format!("/channels/{other}/messages"),
            &t.admin.token,
            json!({"content":"far away"}),
        )
        .await);
    let stray = id(&t
        .post(
            &format!("/channels/{other}/messages"),
            &t.admin.token,
            json!({"content":"first","reply_to":elsewhere}),
        )
        .await);
    let stray_thread = sqlx::query_scalar::<_, String>("SELECT thread_id FROM messages WHERE id=?")
        .bind(&stray)
        .fetch_one(&t.state.db)
        .await
        .unwrap();

    let before = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM threads")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    // An unattachable upload must not leave the thread its reply would have opened,
    // nor a task mapping that would capture the job's later posts.
    assert_eq!(
        status(
            &t,
            Method::POST,
            &path,
            &t.admin.token,
            json!({"content":"x","reply_to":root,"task_id":"doomed-run","upload_ids":[ulid::Ulid::new().to_string()]})
        )
        .await,
        400
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM thread_tasks WHERE task_id='doomed-run'"
        )
        .fetch_one(&t.state.db)
        .await
        .unwrap(),
        0
    );
    // A thread from another channel is not a destination here.
    assert_eq!(
        status(
            &t,
            Method::POST,
            &path,
            &t.admin.token,
            json!({"content":"x","thread_id":stray_thread})
        )
        .await,
        400
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM threads")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM messages WHERE content='x'")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        0
    );

    // A DM thread is reachable by its members and by nobody else, over REST and WS.
    let dm = id(&t
        .post(
            "/dms",
            &alice.token,
            json!({"member_ids":[t.admin.user.id]}),
        )
        .await);
    let dm_path = format!("/channels/{dm}/messages");
    let dm_root = id(&t
        .post(&dm_path, &alice.token, json!({"content":"just us"}))
        .await);
    let mut watcher = socket(&t, &bob.token).await;
    let dm_reply = t
        .post(
            &dm_path,
            &alice.token,
            json!({"content":"still just us","reply_to":dm_root}),
        )
        .await;
    let dm_thread = dm_reply["thread_id"].as_str().unwrap().to_string();
    for (method, path) in [
        (Method::GET, format!("/threads/{dm_thread}/messages")),
        (Method::PATCH, format!("/threads/{dm_thread}")),
    ] {
        assert_eq!(
            status(&t, method, &path, &bob.token, json!({"title":"mine now"})).await,
            404
        );
    }
    assert_eq!(
        status(
            &t,
            Method::POST,
            &dm_path,
            &bob.token,
            json!({"content":"hello?","thread_id":dm_thread})
        )
        .await,
        404
    );
    // Post in a room the outsider can see: reaching it proves the socket was live
    // and had every chance to deliver the DM thread's metadata first.
    let seen = t
        .post(
            &format!("/channels/{channel}/messages"),
            &t.admin.token,
            json!({"content":"public marker"}),
        )
        .await;
    let mut leaked = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(3), watcher.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                Event::MessageCreated(m) if m.id == id(&seen) => break,
                Event::ThreadUpdated { thread } => leaked.push(thread.id),
                _ => {}
            },
            Frame::Ping(v) => watcher.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    assert!(leaked.is_empty(), "leaked thread metadata: {leaked:?}");
}

#[tokio::test]
async fn task_identity_groups_work_and_survives_a_restart() {
    let mut t = Test::new().await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    // Two jobs from one identity are independent; nothing is inferred from the bot.
    let first = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"job one","task_id":"run-1"}),
        )
        .await;
    let second = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"job two","task_id":"run-2"}),
        )
        .await;
    let one = first["thread"]["id"].as_str().unwrap().to_string();
    let two = second["thread"]["id"].as_str().unwrap().to_string();
    assert_ne!(one, two);
    // A task's first post is a room root, so the room still shows the job started.
    assert!(first["thread_id"].is_null());
    assert_eq!(
        ids(&t, &format!("{path}?roots_only=true"), &t.admin.token).await,
        vec![id(&first), id(&second)]
    );
    // Later posts carrying the same ID join without repeating any destination.
    let progress = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"step","task_id":"run-1"}),
        )
        .await;
    assert_eq!(progress["thread_id"].as_str().unwrap(), one);

    // An established mapping cannot be redirected into another conversation.
    assert_eq!(
        status(
            &t,
            Method::POST,
            &path,
            &t.admin.token,
            json!({"content":"hijack","task_id":"run-1","thread_id":two})
        )
        .await,
        409
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM messages WHERE content='hijack'")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        0
    );
    // A post with no task context at all is an ordinary room message.
    let plain = t
        .post(&path, &t.admin.token, json!({"content":"just talking"}))
        .await;
    assert!(plain["thread_id"].is_null() && plain["thread"].is_null());

    // The mapping lives in the database, not in the running server: after a real
    // shutdown a replacement resumes the same thread.
    let (url, task) = restart(&mut t).await;
    let resumed: Value = t
        .http
        .post(format!("{url}{path}"))
        .bearer_auth(&t.admin.token)
        .json(&json!({"content":"after restart","task_id":"run-1"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resumed["thread_id"].as_str().unwrap(), one);
    task.abort();
}

#[tokio::test]
async fn resolving_is_explicit_reversible_and_blocks_new_content() {
    let mut t = Test::new().await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let root = id(&t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"   \n  first   line  \nsecond line"}),
        )
        .await);
    let reply = t
        .post(&path, &bob.token, json!({"content":"a","reply_to":root}))
        .await;
    let thread = reply["thread_id"].as_str().unwrap().to_string();
    let spare = id(&t
        .post(&path, &bob.token, json!({"content":"b","reply_to":root}))
        .await);
    // A task thread with no replies yet: its root is still undeletable, and its
    // established ID must not be able to reopen it once resolved.
    let job = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"deploy it","task_id":"run-9"}),
        )
        .await;
    let job_root = id(&job);
    let job_thread = job["thread"]["id"].as_str().unwrap().to_string();
    assert_eq!(job["thread"]["reply_count"], 0);
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{job_root}"),
            &t.admin.token,
            json!({})
        )
        .await,
        409
    );
    // The title is the first nonempty root line with its whitespace collapsed,
    // skipping leading blank lines, and editing the root never rewrites that
    // snapshot.
    let resolved: ThreadSummary = t
        .req(Method::PATCH, &format!("/threads/{thread}"), &bob.token)
        .json(&json!({"resolved":true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resolved.title, "first line");
    assert_eq!(resolved.resolved_by.unwrap(), bob.user.id);
    let at = resolved.resolved_at.unwrap();

    // Assert the edit itself was accepted, so a rejected request cannot pass as a
    // preserved snapshot title.
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/messages/{root}"),
            &t.admin.token,
            json!({"content":"totally different now"})
        )
        .await,
        200
    );
    // Resolving again is idempotent: the original actor and time are preserved.
    let again: ThreadSummary = t
        .req(Method::PATCH, &format!("/threads/{thread}"), &t.admin.token)
        .json(&json!({"resolved":true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(again.title, "first line");
    assert_eq!(again.resolved_at.unwrap(), at);
    assert_eq!(again.resolved_by.unwrap(), bob.user.id);

    // New content is refused; a delayed agent update cannot undo the resolution.
    for body in [
        json!({"content":"late","thread_id":thread}),
        json!({"content":"late","reply_to":root}),
    ] {
        assert_eq!(
            status(&t, Method::POST, &path, &t.admin.token, body).await,
            409
        );
    }
    // The same for a job: resolving ends it, and a straggling post carrying the
    // established task ID is a conflict rather than a silent reopen.
    t.req(
        Method::PATCH,
        &format!("/threads/{job_thread}"),
        &t.admin.token,
    )
    .json(&json!({"resolved":true}))
    .send()
    .await
    .unwrap();
    assert_eq!(
        status(
            &t,
            Method::POST,
            &path,
            &t.admin.token,
            json!({"content":"straggler","task_id":"run-9"})
        )
        .await,
        409
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM messages WHERE content='straggler'")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        0
    );
    // Content and read rows survive; the replies are all still there.
    assert_eq!(
        ids(&t, &format!("/threads/{thread}/messages"), &bob.token)
            .await
            .len(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM thread_read_state WHERE thread_id=?")
            .bind(&thread)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        2
    );
    // Reopening is explicit, and then the conversation accepts content again.
    let open: ThreadSummary = t
        .req(Method::PATCH, &format!("/threads/{thread}"), &t.admin.token)
        .json(&json!({"resolved":false,"title":"  renamed  "}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(open.resolved_at.is_none() && open.resolved_by.is_none());
    assert_eq!(open.title, "renamed");
    t.post(
        &path,
        &t.admin.token,
        json!({"content":"late","thread_id":thread}),
    )
    .await;

    // A root cannot be deleted out from under its conversation; a reply can.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{root}"),
            &t.admin.token,
            json!({})
        )
        .await,
        409
    );
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{spare}"),
            &bob.token,
            json!({})
        )
        .await,
        204
    );

    // Resolution is stored, not timed. Shut the server down entirely, serve the
    // same directory again, and the job is still resolved and still refusing its
    // own task ID. No passage of time changed anything.
    let (url, task) = restart(&mut t).await;
    let job_after: Message = t
        .http
        .get(format!("{url}/messages/{job_root}"))
        .bearer_auth(&t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let after = job_after.thread.unwrap();
    assert!(after.resolved_at.is_some() && after.resolved_by.is_some());
    assert_eq!(
        t.http
            .post(format!("{url}{path}"))
            .bearer_auth(&t.admin.token)
            .json(&json!({"content":"straggler","task_id":"run-9"}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    // The thread reopened before the restart is still open and still accepts work.
    assert_eq!(
        t.http
            .post(format!("{url}{path}"))
            .bearer_auth(&t.admin.token)
            .json(&json!({"content":"after restart","thread_id":thread}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    task.abort();
}

#[tokio::test]
async fn posting_joins_a_thread_without_acknowledging_what_it_did_not_read() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let carol = t.member("carol").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"root"}))
        .await);
    let first = t
        .post(&path, &bob.token, json!({"content":"a","reply_to":root}))
        .await;
    let thread = first["thread_id"].as_str().unwrap().to_string();
    let position = |user: String, thread: String, db: sqlx::SqlitePool| async move {
        sqlx::query_as::<_, (Option<String>, bool)>(
            "SELECT last_read_id,following FROM thread_read_state WHERE user_id=? AND thread_id=?",
        )
        .bind(user)
        .bind(thread)
        .fetch_optional(&db)
        .await
        .unwrap()
    };
    // Opening a thread creates it for its root author and its first participant,
    // both starting at the root: the first reply is new activity for the author.
    assert_eq!(
        position(t.admin.user.id.clone(), thread.clone(), t.state.db.clone())
            .await
            .unwrap(),
        (Some(root.clone()), true)
    );
    assert_eq!(
        position(bob.user.id.clone(), thread.clone(), t.state.db.clone())
            .await
            .unwrap(),
        (Some(root.clone()), true)
    );
    // Carol is not in the conversation until something brings her in.
    assert!(
        position(carol.user.id.clone(), thread.clone(), t.state.db.clone())
            .await
            .is_none()
    );

    t.post(
        &path,
        &t.admin.token,
        json!({"content":"b","thread_id":thread}),
    )
    .await;
    // Bob unfollows, then more arrives, then bob posts. Posting re-follows him but
    // must not move his position: a CLI send is not evidence he read anything.
    sqlx::query("UPDATE thread_read_state SET following=0 WHERE user_id=? AND thread_id=?")
        .bind(&bob.user.id)
        .bind(&thread)
        .execute(&t.state.db)
        .await
        .unwrap();
    t.post(
        &path,
        &t.admin.token,
        json!({"content":"c","thread_id":thread}),
    )
    .await;
    t.post(
        &path,
        &bob.token,
        json!({"content":"@carol look","thread_id":thread}),
    )
    .await;
    assert_eq!(
        position(bob.user.id.clone(), thread.clone(), t.state.db.clone())
            .await
            .unwrap(),
        (Some(root.clone()), true),
        "posting moved an existing follower's read position"
    );
    // A mention on a new message joins carol just before it, so the mention itself
    // is unread without the whole backlog becoming new activity.
    let carol_start = position(carol.user.id.clone(), thread.clone(), t.state.db.clone())
        .await
        .unwrap();
    assert!(carol_start.1);
    let before_mention = sqlx::query_scalar::<_, String>(
        "SELECT max(id) FROM messages WHERE thread_id=? AND content='c'",
    )
    .bind(&thread)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(carol_start.0.unwrap(), before_mention);

    // Editing a message to add a mention is not somebody joining: the indexer runs
    // for edits and for startup backfill too, and neither is an act of joining.
    let edited = id(&t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"quiet","thread_id":thread}),
        )
        .await);
    let dave = t.member("dave").await;
    t.req(
        Method::PATCH,
        &format!("/messages/{edited}"),
        &t.admin.token,
    )
    .json(&json!({"content":"@dave quiet"}))
    .send()
    .await
    .unwrap();
    assert!(position(dave.user.id, thread, t.state.db.clone())
        .await
        .is_none());
}

#[tokio::test]
async fn pre_thread_quotes_stay_history_and_keep_their_old_deletion_rule() {
    let t = Test::new().await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    // Seed rows the way the database looked before 0019: a quote whose reply_to
    // points at an older message, and no thread anywhere. Both IDs come from the
    // server's own monotonic generator, so the asserted order is deterministic
    // rather than two ULIDs minted in the same millisecond.
    let older = id(&t
        .post(&path, &t.admin.token, json!({"content":"old topic"}))
        .await);
    let quote = id(&t
        .post(&path, &t.admin.token, json!({"content":"old quote"}))
        .await);
    sqlx::query("UPDATE messages SET reply_to=? WHERE id=?")
        .bind(&older)
        .bind(&quote)
        .execute(&t.state.db)
        .await
        .unwrap();
    // The migration reinterprets nothing: an old quote is main-conversation history
    // and stays in the roots-only view.
    assert_eq!(
        ids(&t, &format!("{path}?roots_only=true"), &t.admin.token).await,
        vec![older.clone(), quote.clone()]
    );

    // Replying to that old quote now roots a thread at the quote itself. Its own
    // reply_to chain is history and is never walked back to `older`.
    let fresh = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"new","reply_to":quote}),
        )
        .await;
    let thread = fresh["thread_id"].as_str().unwrap().to_string();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT root_message_id FROM threads WHERE id=?")
            .bind(&thread)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        quote
    );

    // `older` roots nothing, so the pre-thread rule still applies to it: deleting it
    // succeeds and nulls the quote's reply_to, without touching the new thread.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{older}"),
            &t.admin.token,
            json!({})
        )
        .await,
        204
    );
    let quoted: Message = t
        .req(Method::GET, &format!("/messages/{quote}"), &t.admin.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(quoted.reply_to.is_none());
    assert_eq!(quoted.thread.unwrap().id, thread);
    assert_eq!(
        ids(&t, &format!("/threads/{thread}/messages"), &t.admin.token).await,
        vec![id(&fresh)]
    );
}

/// Drain until `predicate` matches, collecting every ThreadUpdated seen on the way.
/// A later known event is the barrier: reaching it proves anything the server would
/// have sent earlier has already arrived, so absence needs no sleep.
async fn drain(
    socket: &mut super::realtime::Socket,
    predicate: impl Fn(&Event) -> bool,
) -> Vec<ThreadSummary> {
    let mut seen = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("websocket event")
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => {
                let event: Event = serde_json::from_str(&v).unwrap();
                if let Event::ThreadUpdated { thread } = &event {
                    seen.push(thread.clone());
                }
                if predicate(&event) {
                    return seen;
                }
            }
            Frame::Ping(v) => socket.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
}

#[tokio::test]
async fn thread_metadata_reaches_channel_members_and_only_them() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let outsider = t.member("outsider").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let dm = id(&t
        .post("/dms", &bob.token, json!({"member_ids":[t.admin.user.id]}))
        .await);
    let mut member = socket(&t, &bob.token).await;
    let mut stranger = socket(&t, &outsider.token).await;
    // Both sockets are attached only after their initial Resync, so nothing below
    // races with connection setup.
    for s in [&mut member, &mut stranger] {
        drain(s, |e| matches!(e, Event::Resync { .. })).await;
    }

    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"ship it"}))
        .await);
    let reply = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"a","reply_to":root}),
        )
        .await;
    let thread = reply["thread_id"].as_str().unwrap().to_string();
    // MessageCreated is emitted before the metadata that follows it, so the barrier
    // is a later message: reaching it proves the ThreadUpdated already arrived.
    let marker = t
        .post(&path, &t.admin.token, json!({"content":"barrier zero"}))
        .await;
    let created = drain(
        &mut member,
        |e| matches!(e, Event::MessageCreated(m) if m.id == id(&marker)),
    )
    .await;
    // Creation delivers the real metadata, not merely some event.
    let summary = created
        .iter()
        .rev()
        .find(|s| s.id == thread)
        .expect("no ThreadUpdated on create");
    assert_eq!(summary.root_message_id, root);
    assert_eq!(summary.title, "ship it");
    assert_eq!(summary.reply_count, 1);

    let second = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"b","thread_id":thread}),
        )
        .await;
    drain(
        &mut member,
        |e| matches!(e, Event::MessageCreated(m) if m.id == id(&second)),
    )
    .await;
    // Resolving broadcasts the resolution itself.
    t.req(Method::PATCH, &format!("/threads/{thread}"), &t.admin.token)
        .json(&json!({"resolved":true,"title":"shipped"}))
        .send()
        .await
        .unwrap();
    let marker = t
        .post(&path, &t.admin.token, json!({"content":"barrier one"}))
        .await;
    let resolved = drain(
        &mut member,
        |e| matches!(e, Event::MessageCreated(m) if m.id == id(&marker)),
    )
    .await;
    let summary = resolved
        .iter()
        .rev()
        .find(|s| s.id == thread)
        .expect("no ThreadUpdated on resolve");
    assert!(summary.resolved_at.is_some());
    assert_eq!(summary.title, "shipped");
    assert_eq!(summary.reply_count, 2);

    // Deleting a reply refreshes the count, because it is counted from the rows.
    t.req(
        Method::DELETE,
        &format!("/messages/{}", id(&second)),
        &t.admin.token,
    )
    .send()
    .await
    .unwrap();
    let marker = t
        .post(&path, &t.admin.token, json!({"content":"barrier two"}))
        .await;
    let deleted = drain(
        &mut member,
        |e| matches!(e, Event::MessageCreated(m) if m.id == id(&marker)),
    )
    .await;
    assert_eq!(
        deleted
            .iter()
            .rev()
            .find(|s| s.id == thread)
            .expect("no ThreadUpdated on reply deletion")
            .reply_count,
        1
    );

    // A private conversation's metadata reaches nobody outside it. The public
    // markers above and below are the barrier the outsider does receive.
    let dm_root = id(&t
        .post(
            &format!("/channels/{dm}/messages"),
            &bob.token,
            json!({"content":"between us"}),
        )
        .await);
    t.post(
        &format!("/channels/{dm}/messages"),
        &bob.token,
        json!({"content":"still between us","reply_to":dm_root}),
    )
    .await;
    let last = t
        .post(&path, &t.admin.token, json!({"content":"barrier three"}))
        .await;
    let leaked = drain(
        &mut stranger,
        |e| matches!(e, Event::MessageCreated(m) if m.id == id(&last)),
    )
    .await;
    assert!(
        leaked.iter().all(|s| s.channel_id == channel),
        "private thread metadata reached an outsider: {leaked:?}"
    );
}

#[tokio::test]
async fn typing_is_scoped_to_its_conversation_and_never_confirms_a_private_thread() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let outsider = t.member("outsider").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let thread = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"a","reply_to":root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    let dm = id(&t
        .post("/dms", &bob.token, json!({"member_ids":[t.admin.user.id]}))
        .await);
    let dm_root = id(&t
        .post(
            &format!("/channels/{dm}/messages"),
            &bob.token,
            json!({"content":"ours"}),
        )
        .await);
    let private = t
        .post(
            &format!("/channels/{dm}/messages"),
            &bob.token,
            json!({"content":"b","reply_to":dm_root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut watcher = socket(&t, &bob.token).await;
    let mut sender = socket(&t, &outsider.token).await;
    for s in [&mut watcher, &mut sender] {
        drain(s, |e| matches!(e, Event::Resync { .. })).await;
    }
    // A DM thread ID submitted with a public channel ID must produce nothing: the
    // thread is not in that channel, and answering would confirm it exists.
    for body in [
        json!({"type":"typing","channel_id":channel,"thread_id":private}),
        // Room and thread typing back to back: one must not swallow the other.
        json!({"type":"typing","channel_id":channel}),
        json!({"type":"typing","channel_id":channel,"thread_id":thread}),
    ] {
        sender
            .send(Frame::Text(body.to_string().into()))
            .await
            .unwrap();
    }
    // The socket and the HTTP request are independent connections, so the marker
    // could otherwise overtake the typing frames. The server reads this socket in
    // order, so its Pong cannot arrive until every frame above has been handled.
    sender.send(Frame::Ping(Vec::new().into())).await.unwrap();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), sender.next())
            .await
            .expect("pong")
            .unwrap()
            .unwrap()
        {
            Frame::Pong(_) => break,
            Frame::Ping(v) => sender.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    let marker = t
        .post(&path, &t.admin.token, json!({"content":"barrier"}))
        .await;
    let mut typing = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), watcher.next())
            .await
            .expect("websocket event")
            .unwrap()
            .unwrap()
        {
            Frame::Text(v) => match serde_json::from_str::<Event>(&v).unwrap() {
                Event::Typing {
                    channel_id,
                    thread_id,
                    ..
                } => typing.push((channel_id, thread_id)),
                Event::MessageCreated(m) if m.id == id(&marker) => break,
                _ => {}
            },
            Frame::Ping(v) => watcher.send(Frame::Pong(v)).await.unwrap(),
            _ => {}
        }
    }
    assert_eq!(
        typing,
        vec![
            (channel.clone(), None),
            (channel.clone(), Some(thread.clone()))
        ],
        "expected exactly the room and thread indicators, in order"
    );
}

#[tokio::test]
async fn unsupported_thread_context_is_refused_without_side_effects() {
    let t = Test::new().await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let thread = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"a","reply_to":root}),
        )
        .await["thread_id"]
        .as_str()
        .unwrap()
        .to_string();
    let counts = |db: sqlx::SqlitePool| async move {
        let one = |sql: &'static str, db: sqlx::SqlitePool| async move {
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&db)
                .await
                .unwrap()
        };
        (
            one("SELECT count(*) FROM objects", db.clone()).await,
            one("SELECT count(*) FROM terminal_sessions", db.clone()).await,
            one("SELECT count(*) FROM channels WHERE kind='dm'", db.clone()).await,
            one("SELECT count(*) FROM read_state", db).await,
        )
    };
    let before = counts(t.state.db.clone()).await;
    // Placing cards in a conversation is the next PR. Until then these fields are
    // refused rather than accepted and dropped, which would put a job's card
    // somewhere its runner never asked for.
    let context = [
        json!({"thread_id": thread}),
        json!({"task_id": "run-1"}),
        json!({"reply_to": root}),
    ];
    for extra in &context {
        for (route, base) in [
            (
                format!("/channels/{channel}/objects"),
                json!({"kind":"canvas","name":"board","state":{}}),
            ),
            // The host does not exist, so an unguarded handler would answer 404 at
            // its lookup. A 400 therefore proves the guard runs first, ahead of
            // everything else in the handler. This fixture supplies channel_id and
            // so does not reach the no-channel default self-DM branch; ordinary
            // terminal opening is covered by the existing terminal fixtures.
            (
                format!("/hosts/{}/sessions", ulid::Ulid::new()),
                json!({"channel_id": channel}),
            ),
            (
                format!("/sessions/{}/share", ulid::Ulid::new()),
                json!({"channel_id": channel}),
            ),
        ] {
            let mut body = base.as_object().unwrap().clone();
            body.extend(extra.as_object().unwrap().clone());
            assert_eq!(
                status(&t, Method::POST, &route, &t.admin.token, body.into()).await,
                400,
                "{route} accepted {extra}"
            );
        }
    }
    // The main conversation's own marker is still the flat one until the read model
    // lands, so claiming to read only the roots is refused rather than quietly
    // acknowledging the thread replies it says it is skipping.
    assert_eq!(
        status(
            &t,
            Method::PUT,
            &format!("/channels/{channel}/read"),
            &t.admin.token,
            json!({"message_id": root, "roots_only": true})
        )
        .await,
        400
    );
    assert_eq!(counts(t.state.db.clone()).await, before);
    // The same calls without context are unchanged.
    t.post(
        &format!("/channels/{channel}/objects"),
        &t.admin.token,
        json!({"kind":"canvas","name":"board","state":{}}),
    )
    .await;
    assert_eq!(
        status(
            &t,
            Method::PUT,
            &format!("/channels/{channel}/read"),
            &t.admin.token,
            json!({"message_id": root})
        )
        .await,
        200
    );
}

#[tokio::test]
async fn a_title_is_a_snapshot_and_a_root_cannot_be_deleted_from_under_its_thread() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");
    let root = id(&t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"   \n  first   line  \nsecond line"}),
        )
        .await);
    let reply = t
        .post(&path, &bob.token, json!({"content":"a","reply_to":root}))
        .await;
    let thread = reply["thread_id"].as_str().unwrap().to_string();
    let spare = id(&t
        .post(&path, &bob.token, json!({"content":"b","reply_to":root}))
        .await);
    let title = |db: sqlx::SqlitePool, thread: String| async move {
        sqlx::query_scalar::<_, String>("SELECT title FROM threads WHERE id=?")
            .bind(thread)
            .fetch_one(&db)
            .await
            .unwrap()
    };
    // The first nonempty line, whitespace collapsed; leading blank lines are skipped.
    assert_eq!(
        title(t.state.db.clone(), thread.clone()).await,
        "first line"
    );
    // Editing the root never rewrites the snapshot. Assert the edit itself was
    // accepted, so a rejected request cannot pass as a preserved title.
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/messages/{root}"),
            &t.admin.token,
            json!({"content":"totally different now"})
        )
        .await,
        200
    );
    assert_eq!(
        title(t.state.db.clone(), thread.clone()).await,
        "first line"
    );

    // Deleting the root would take other people's replies with it, so it is refused
    // while a reply of the same conversation deletes normally.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{root}"),
            &t.admin.token,
            json!({})
        )
        .await,
        409
    );
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{spare}"),
            &bob.token,
            json!({})
        )
        .await,
        204
    );
    assert_eq!(
        ids(&t, &format!("/threads/{thread}/messages"), &bob.token).await,
        vec![id(&reply)]
    );
}

/// A threads row is created by the first reply and is never removed: there is no
/// delete-thread route and nothing issues `DELETE FROM threads`. So guarding the
/// root's deletion on the row's mere existence made a message permanently
/// undeletable the moment anyone replied to it, even once that reply was gone, and
/// for its own author. The guard has to follow the conversation, not the row.
#[tokio::test]
async fn a_root_becomes_deletable_again_once_its_last_reply_goes() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let channel = t.general().await;
    let path = format!("/channels/{channel}/messages");

    let root = id(&t
        .post(&path, &t.admin.token, json!({"content":"topic"}))
        .await);
    let reply = id(&t
        .post(&path, &bob.token, json!({"content":"a","reply_to":root}))
        .await);

    // While the conversation holds a reply the guard is doing its job: deleting the
    // root would take someone else's reply with it.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{root}"),
            &t.admin.token,
            json!({})
        )
        .await,
        409
    );

    // Bob removes his own reply. The threads row survives, because nothing deletes it.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{reply}"),
            &bob.token,
            json!({})
        )
        .await,
        204
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM threads WHERE root_message_id=?")
            .bind(&root)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        1
    );

    // With the conversation empty the root is ordinary history again and its author
    // can delete it.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/messages/{root}"),
            &t.admin.token,
            json!({})
        )
        .await,
        204
    );
}
