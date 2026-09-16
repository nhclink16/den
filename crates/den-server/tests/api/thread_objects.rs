use super::terminal_reconciliation::{alive, stopped, Host};
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
async fn canvas(t: &Test, channel: &str, token: &str, name: &str, ctx: Value) -> Value {
    let mut body = json!({"kind":"canvas","name":name,"state":{}})
        .as_object()
        .unwrap()
        .clone();
    body.extend(ctx.as_object().unwrap().clone());
    t.post(&format!("/channels/{channel}/objects"), token, body.into())
        .await
}
/// The conversation a card's message landed in, read back through the API rather
/// than from the database, so the response a client actually sees is what is tested.
async fn placed(t: &Test, token: &str, message: &str) -> Message {
    t.req(Method::GET, &format!("/messages/{message}"), token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn counts(db: &sqlx::SqlitePool) -> (i64, i64, i64, i64) {
    let one = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(db)
            .await
            .unwrap()
    };
    (
        one("SELECT count(*) FROM threads").await,
        one("SELECT count(*) FROM thread_tasks").await,
        one("SELECT count(*) FROM messages").await,
        one("SELECT count(*) FROM objects").await,
    )
}

#[tokio::test]
async fn a_task_groups_its_cards_and_its_text_in_one_conversation() {
    let t = Test::new().await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    // A canvas is the first thing this job produces, so the card itself roots the
    // conversation and the card's own name titles it.
    let board = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Deploy board",
        json!({"task_id":"run-1"}),
    )
    .await;
    let root = board["message_id"].as_str().unwrap().to_string();
    let rooted = placed(&t, &t.admin.token, &root).await;
    assert!(
        rooted.thread_id.is_none(),
        "the card that opens a job stays in the room"
    );
    let summary = rooted
        .thread
        .clone()
        .expect("the card opened a conversation");
    assert_eq!(summary.title, "Deploy board");
    assert_eq!(summary.reply_count, 0);
    let thread = summary.id.clone();

    // Progress text inherits the job without naming the thread at all.
    let progress = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"building","task_id":"run-1"}),
        )
        .await;
    assert_eq!(progress["thread_id"].as_str().unwrap(), thread);
    // So does a second card.
    let second = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Rollout plan",
        json!({"task_id":"run-1"}),
    )
    .await;
    let second_message = second["message_id"].as_str().unwrap().to_string();
    assert_eq!(
        placed(&t, &t.admin.token, &second_message)
            .await
            .thread_id
            .as_deref(),
        Some(thread.as_str())
    );
    // The root's summary counts the object-only reply like any other.
    let after = placed(&t, &t.admin.token, &root).await.thread.unwrap();
    assert_eq!(after.reply_count, 2, "an object-only reply is a reply");
    assert_eq!(
        after.last_reply_id.as_deref(),
        Some(second_message.as_str())
    );

    // A card can also quote a specific message. Placement roots the conversation at
    // the quoted message, and the card must still carry the quote itself: losing it
    // leaves the reply arrow pointing nowhere even though the thread is right.
    let question = id(&t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"can you chart this"}),
        )
        .await);
    let answer = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Chart",
        json!({"task_id":"run-3","reply_to":question}),
    )
    .await;
    let answer_message = placed(&t, &t.admin.token, answer["message_id"].as_str().unwrap()).await;
    assert_eq!(
        answer_message.reply_to.as_deref(),
        Some(question.as_str()),
        "the card lost the message it was replying to"
    );
    assert_eq!(
        placed(&t, &t.admin.token, &question)
            .await
            .thread
            .expect("the quoted message roots the conversation")
            .id,
        answer_message.thread_id.clone().unwrap()
    );

    // A second job from the same identity is a separate conversation.
    let other = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Other job",
        json!({"task_id":"run-2"}),
    )
    .await;
    let other_thread = placed(&t, &t.admin.token, other["message_id"].as_str().unwrap())
        .await
        .thread
        .unwrap()
        .id;
    assert_ne!(other_thread, thread);
}

#[tokio::test]
async fn a_refused_card_destination_leaves_nothing_behind() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let outsider = t.member("outsider").await;
    let cid = t.general().await;
    let other = id(&t
        .post(
            "/channels",
            &t.admin.token,
            json!({"name":"other","category_id":null,"position":4}),
        )
        .await);
    // A conversation in another channel.
    let stray = canvas(
        &t,
        &other,
        &t.admin.token,
        "Elsewhere",
        json!({"task_id":"far"}),
    )
    .await;
    let stray_thread = placed(&t, &t.admin.token, stray["message_id"].as_str().unwrap())
        .await
        .thread
        .unwrap()
        .id;
    // A private DM between admin and bob. The outsider is in neither.
    let dm = id(&t
        .post("/dms", &t.admin.token, json!({"member_ids":[bob.user.id]}))
        .await);
    let private = canvas(&t, &dm, &t.admin.token, "Ours", json!({"task_id":"dm-job"})).await;
    let private_thread = placed(&t, &t.admin.token, private["message_id"].as_str().unwrap())
        .await
        .thread
        .unwrap()
        .id;
    // Two jobs in this room: one that gets resolved, one that stays open.
    let job = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Shipping",
        json!({"task_id":"run-1"}),
    )
    .await;
    let job_thread = placed(&t, &t.admin.token, job["message_id"].as_str().unwrap())
        .await
        .thread
        .unwrap()
        .id;
    let open_job = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Still open",
        json!({"task_id":"run-2"}),
    )
    .await;
    let open_thread = placed(&t, &t.admin.token, open_job["message_id"].as_str().unwrap())
        .await
        .thread
        .unwrap()
        .id;
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{job_thread}"),
            &t.admin.token,
            json!({"resolved": true})
        )
        .await,
        200
    );

    let route = format!("/channels/{cid}/objects");
    // Each refusal is checked against its own before/after snapshot, so a failure
    // names the case that leaked rather than the batch.
    let refuse = |label: &'static str, token: String, body: Value, expected: u16| {
        let t = &t;
        let route = route.clone();
        async move {
            let before = counts(&t.state.db).await;
            assert_eq!(
                status(t, Method::POST, &route, &token, body).await,
                expected,
                "{label}"
            );
            assert_eq!(
                counts(&t.state.db).await,
                before,
                "{label} left rows behind"
            );
        }
    };
    refuse(
        "a thread in another channel is not a destination here",
        t.admin.token.clone(),
        json!({"kind":"canvas","name":"x","state":{},"thread_id":stray_thread}),
        400,
    )
    .await;
    refuse(
        "a resolved conversation refuses a card",
        t.admin.token.clone(),
        json!({"kind":"canvas","name":"x","state":{},"thread_id":job_thread}),
        409,
    )
    .await;
    refuse(
        "a resolved job refuses its own task ID",
        t.admin.token.clone(),
        json!({"kind":"canvas","name":"x","state":{},"task_id":"run-1"}),
        409,
    )
    .await;
    refuse(
        "a task pointed at another channel's conversation",
        t.admin.token.clone(),
        json!({"kind":"canvas","name":"x","state":{},"task_id":"run-2","thread_id":stray_thread}),
        400,
    )
    .await;

    // The mapping-disagreement case needs BOTH destinations open, or a 409 would
    // only prove the target was resolved. Reopen the finished job and aim run-2 at it.
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{job_thread}"),
            &t.admin.token,
            json!({"resolved": false})
        )
        .await,
        200
    );
    refuse(
        "an established job cannot be redirected into another open conversation",
        t.admin.token.clone(),
        json!({"kind":"canvas","name":"x","state":{},"task_id":"run-2","thread_id":job_thread}),
        409,
    )
    .await;

    // Someone outside the DM cannot reach its conversation at all, and learns
    // nothing from the difference between forbidden and missing.
    let dm_route = format!("/channels/{dm}/objects");
    let before = counts(&t.state.db).await;
    assert_eq!(
        status(
            &t,
            Method::POST,
            &dm_route,
            &outsider.token,
            json!({"kind":"canvas","name":"x","state":{},"thread_id":private_thread})
        )
        .await,
        404,
        "an outsider reached a private conversation"
    );
    assert_eq!(
        counts(&t.state.db).await,
        before,
        "a refused outsider left rows behind"
    );

    // After all of that the still-open job is exactly where it was: read the card
    // back and compare its real placement rather than asserting a value into being.
    let carried = canvas(
        &t,
        &cid,
        &t.admin.token,
        "carry on",
        json!({"task_id":"run-2"}),
    )
    .await;
    assert_eq!(
        placed(&t, &t.admin.token, carried["message_id"].as_str().unwrap())
            .await
            .thread_id
            .as_deref(),
        Some(open_thread.as_str()),
        "the surviving job moved"
    );
}

#[tokio::test]
async fn a_real_terminal_joins_its_job_and_survives_the_conversation_being_resolved() {
    let t = Test::new().await;
    let bob = t.member("bob").await;
    let cid = t.general().await;
    let path = format!("/channels/{cid}/messages");
    let mut host = Host::new(&t).await;
    let mut socket = host.connect(&t).await;
    host.hello(&mut socket, None).await;

    // The job starts with a human request, and the runner replies to it, so the
    // terminal it opens next inherits that conversation by task ID alone.
    let request = id(&t
        .post(
            &path,
            &bob.token,
            json!({"content":"why is deploy failing"}),
        )
        .await);
    let first = t
        .post(
            &path,
            &t.admin.token,
            json!({"content":"looking","reply_to":request,"task_id":"run-1"}),
        )
        .await;
    let thread = first["thread_id"].as_str().unwrap().to_string();

    let (session, card) = host
        .open_with(
            &t,
            &mut socket,
            json!({"channel_id":cid,"task_id":"run-1","reply_to":request}),
        )
        .await;
    let card_message = card["message_id"].as_str().unwrap().to_string();
    let placed_card = placed(&t, &t.admin.token, &card_message).await;
    assert_eq!(
        placed_card.thread_id.as_deref(),
        Some(thread.as_str()),
        "the terminal card joined the job's conversation"
    );
    assert_eq!(
        placed_card.reply_to.as_deref(),
        Some(request.as_str()),
        "the terminal card lost the request it was answering"
    );

    // A real shell, running a real command, reporting its own real process ID.
    let pid = host.pid(&t, &mut socket, &session).await;
    assert!(alive(pid), "no live process {pid} for the opened session");

    // Somebody who can see the card still cannot drive the machine behind it.
    assert_eq!(
        status(
            &t,
            Method::POST,
            &format!("/sessions/{session}/write"),
            &bob.token,
            json!({"text":"echo nope\n"})
        )
        .await,
        404,
        "a visible card is not permission to control the host"
    );

    // Resolving ends the conversation, not the machine.
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{thread}"),
            &t.admin.token,
            json!({"resolved": true})
        )
        .await,
        200
    );
    let marker = t.dir.join("after-resolve.pid");
    host.command(
        &t,
        &mut socket,
        &session,
        &format!("echo $$ > '{}'\n", marker.display()),
    )
    .await;
    let still_alive = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(v) = tokio::fs::read_to_string(&marker).await {
                if let Ok(p) = v.trim().parse::<u32>() {
                    break p;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the PTY stopped responding after its thread was resolved");
    assert_eq!(
        still_alive, pid,
        "resolving a conversation must not restart or replace the shell"
    );

    // A terminal for the resolved job is refused too, and no PTY is requested. The
    // host's own inventory is the evidence: a valid command on the original session
    // is an Input frame on the same outbound queue, so once the host has applied it
    // anything the refused request might have queued earlier would already be there.
    let sessions_before = host.ids();
    let rows_before = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM terminal_sessions")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(
        status(
            &t,
            Method::POST,
            &format!(
                "/hosts/{}/sessions",
                host.credential["host_id"].as_str().unwrap()
            ),
            &t.admin.token,
            json!({"channel_id":cid,"task_id":"run-1"})
        )
        .await,
        409
    );
    host.command(&t, &mut socket, &session, "true\n").await;
    assert_eq!(
        host.ids(),
        sessions_before,
        "a refused terminal still reached the host"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM terminal_sessions")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        rows_before,
        "a refused terminal left a session row"
    );

    // New content is refused until somebody reopens it explicitly.
    assert_eq!(
        status(
            &t,
            Method::POST,
            &path,
            &t.admin.token,
            json!({"content":"late","task_id":"run-1"})
        )
        .await,
        409
    );
    assert_eq!(
        status(
            &t,
            Method::POST,
            &format!("/channels/{cid}/objects"),
            &t.admin.token,
            json!({"kind":"canvas","name":"late board","state":{},"task_id":"run-1"})
        )
        .await,
        409
    );
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{thread}"),
            &t.admin.token,
            json!({"resolved": false})
        )
        .await,
        200
    );
    let reopened = canvas(
        &t,
        &cid,
        &t.admin.token,
        "after reopen",
        json!({"task_id":"run-1"}),
    )
    .await;
    assert_eq!(
        placed(&t, &t.admin.token, reopened["message_id"].as_str().unwrap())
            .await
            .thread_id
            .as_deref(),
        Some(thread.as_str())
    );

    // Closing the session leaves no process behind. Wait for the host to apply the
    // actual Close rather than for a Ping, which would not order it.
    assert_eq!(
        status(
            &t,
            Method::DELETE,
            &format!("/sessions/{session}"),
            &t.admin.token,
            json!({})
        )
        .await,
        204
    );
    host.closed(&mut socket, &session).await;
    stopped(pid).await;
    assert!(!alive(pid), "the shell outlived its session");
    assert!(
        host.ids().is_empty(),
        "the host still holds a session it was told to close"
    );
}

#[tokio::test]
async fn sharing_adds_a_card_for_the_same_session_without_starting_another() {
    let t = Test::new().await;
    let cid = t.general().await;
    let mut host = Host::new(&t).await;
    let mut socket = host.connect(&t).await;
    host.hello(&mut socket, None).await;
    // A job in the room, and a terminal opened into it.
    let job = canvas(
        &t,
        &cid,
        &t.admin.token,
        "Room job",
        json!({"task_id":"room-job"}),
    )
    .await;
    let room_thread = placed(&t, &t.admin.token, job["message_id"].as_str().unwrap())
        .await
        .thread
        .unwrap()
        .id;
    let (session, _) = host
        .open_with(
            &t,
            &mut socket,
            json!({"channel_id":cid,"task_id":"room-job"}),
        )
        .await;
    let room_pid = host.pid(&t, &mut socket, &session).await;

    // The pinned default-destination case. The SAME task ID with no channel does not
    // reach across channels to find the room's mapping: it belongs to the author's
    // self-DM and gets its own mapping there, because a task is keyed by channel.
    let (second, second_card) = host
        .open_with(&t, &mut socket, json!({"task_id":"room-job"}))
        .await;
    let dm_pid = host.pid(&t, &mut socket, &second).await;
    assert_ne!(second, session, "a second terminal is its own session");
    let dm_message = placed(
        &t,
        &t.admin.token,
        second_card["message_id"].as_str().unwrap(),
    )
    .await;
    assert_ne!(
        dm_message.channel_id, cid,
        "an omitted channel uses the self-DM default, never the task's room"
    );
    let dm_thread = dm_message
        .thread
        .clone()
        .expect("the card rooted its own conversation in the self-DM")
        .id;
    assert_ne!(dm_thread, room_thread);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM thread_tasks WHERE task_id='room-job'")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        2,
        "one mapping per channel, and the room's is untouched"
    );
    // Naming the room explicitly still joins the original conversation.
    let back = canvas(
        &t,
        &cid,
        &t.admin.token,
        "back in the room",
        json!({"task_id":"room-job"}),
    )
    .await;
    assert_eq!(
        placed(&t, &t.admin.token, back["message_id"].as_str().unwrap())
            .await
            .thread_id
            .as_deref(),
        Some(room_thread.as_str())
    );

    // Sharing adds a card for the SAME session rather than opening another PTY.
    let quoted = id(&t
        .post(
            &format!("/channels/{cid}/messages"),
            &t.admin.token,
            json!({"content":"look at this run","thread_id":room_thread}),
        )
        .await);
    let shared: Value = t
        .post(
            &format!("/sessions/{session}/share"),
            &t.admin.token,
            json!({"channel_id":cid,"thread_id":room_thread,"reply_to":quoted}),
        )
        .await;
    assert_eq!(
        shared["state"]["terminal"]["id"].as_str().unwrap(),
        session,
        "the shared card points at the original session"
    );
    let shared_message = placed(&t, &t.admin.token, shared["message_id"].as_str().unwrap()).await;
    assert_eq!(
        shared_message.thread_id.as_deref(),
        Some(room_thread.as_str())
    );
    assert_eq!(
        shared_message.reply_to.as_deref(),
        Some(quoted.as_str()),
        "the shared card lost the message it was replying to"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM terminal_sessions")
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        2,
        "sharing started no third process"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM terminal_cards WHERE session_id=?")
            .bind(&session)
            .fetch_one(&t.state.db)
            .await
            .unwrap(),
        2,
        "the same session now has two cards"
    );
    // And the host itself never heard about a third one. A command on the original
    // session orders the outbound queue past anything sharing might have sent.
    host.command(&t, &mut socket, &session, "true\n").await;
    let mut expected = vec![session.clone(), second.clone()];
    expected.sort();
    assert_eq!(host.ids(), expected, "sharing opened another PTY");

    // A resolved destination refuses a shared card like any other new content.
    assert_eq!(
        status(
            &t,
            Method::PATCH,
            &format!("/threads/{room_thread}"),
            &t.admin.token,
            json!({"resolved": true})
        )
        .await,
        200
    );
    assert_eq!(
        status(
            &t,
            Method::POST,
            &format!("/sessions/{session}/share"),
            &t.admin.token,
            json!({"channel_id":cid,"thread_id":room_thread})
        )
        .await,
        409
    );
    // The canvas in that resolved conversation is still a working canvas.
    let version: Value = t
        .post(
            &format!("/objects/{}/patch", id(&job)),
            &t.admin.token,
            json!({"base_version":0,"put":[{"id":"shape:1","kind":"note"}],"remove":[]}),
        )
        .await;
    assert_eq!(version["version"], 1);
    let reread: Value = t
        .req(
            Method::GET,
            &format!("/objects/{}", id(&job)),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(reread["state"]["shape:1"]["kind"], "note");
    for (s, pid) in [(&session, room_pid), (&second, dm_pid)] {
        assert_eq!(
            status(
                &t,
                Method::DELETE,
                &format!("/sessions/{s}"),
                &t.admin.token,
                json!({})
            )
            .await,
            204
        );
        host.closed(&mut socket, s).await;
        stopped(pid).await;
        assert!(!alive(pid), "session {s} outlived its close");
    }
    assert!(
        host.ids().is_empty(),
        "the host still holds a closed session"
    );
}
