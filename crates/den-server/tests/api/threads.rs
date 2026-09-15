use super::*;
use sqlx::{Row, SqlitePool};

/// One quoted text column per row, ordered, so a before/after comparison shows any
/// rewritten byte rather than only a changed row count.
async fn dump(db: &SqlitePool, table: &str, columns: &str) -> Vec<String> {
    let quoted = columns
        .split(',')
        .map(|c| format!("quote({})", c.trim()))
        .collect::<Vec<_>>()
        .join("||'|'||");
    sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
        "SELECT {quoted} AS row FROM {table} ORDER BY row"
    )))
    .fetch_all(db)
    .await
    .unwrap()
}

async fn fails(db: &SqlitePool, sql: String) -> String {
    let message = format!("accepted: {sql}");
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(db)
        .await
        .expect_err(&message)
        .to_string()
}

async fn count(db: &SqlitePool, sql: String) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
        .fetch_one(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn thread_storage_keys_resolution_and_cascade_hold() {
    let t = Test::new().await;
    let channel = t
        .post(
            "/channels",
            &t.admin.token,
            json!({"name":"threads","category_id":null,"position":5}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let root = t
        .post(
            &format!("/channels/{channel}/messages"),
            &t.admin.token,
            json!({"content":"why is the build red"}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let db = &t.state.db;
    let thread = ulid::Ulid::new().to_string();
    let reply = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO threads(id,channel_id,root_message_id,title,created_by) VALUES(?,?,?,?,?)",
    )
    .bind(&thread)
    .bind(&channel)
    .bind(&root)
    .bind("why is the build red")
    .bind(&t.admin.user.id)
    .execute(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO messages(id,channel_id,author_id,content,thread_id) VALUES(?,?,?,'a flaky test',?)")
        .bind(&reply).bind(&channel).bind(&t.admin.user.id).bind(&thread)
        .execute(db).await.unwrap();
    sqlx::query("INSERT INTO thread_read_state(user_id,thread_id,last_read_id) VALUES(?,?,?)")
        .bind(&t.admin.user.id)
        .bind(&thread)
        .bind(&reply)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO thread_tasks(user_id,channel_id,task_id,thread_id) VALUES(?,?,'run-42',?)",
    )
    .bind(&t.admin.user.id)
    .bind(&channel)
    .bind(&thread)
    .execute(db)
    .await
    .unwrap();

    // A second thread cannot claim a root another thread already owns. This is what
    // makes two simultaneous first replies converge on one conversation.
    assert!(fails(db, format!(
        "INSERT INTO threads(id,channel_id,root_message_id,title,created_by) VALUES('{}','{channel}','{root}','race','{}')",
        ulid::Ulid::new(), t.admin.user.id)).await.contains("UNIQUE"));
    // One job maps to one thread. A retry or reconnect reuses it; it never forks.
    assert!(fails(db, format!(
        "INSERT INTO thread_tasks(user_id,channel_id,task_id,thread_id) VALUES('{}','{channel}','run-42','{thread}')",
        t.admin.user.id)).await.contains("UNIQUE"));
    // Resolution records who and when together or not at all.
    for half in [
        format!("UPDATE threads SET resolved_at='2026-09-15T10:00:00.000Z' WHERE id='{thread}'"),
        format!(
            "UPDATE threads SET resolved_by='{}' WHERE id='{thread}'",
            t.admin.user.id
        ),
    ] {
        assert!(fails(db, half.clone()).await.contains("CHECK"), "{half}");
    }
    sqlx::query("UPDATE threads SET resolved_at=?,resolved_by=? WHERE id=?")
        .bind("2026-09-15T10:00:00.000Z")
        .bind(&t.admin.user.id)
        .bind(&thread)
        .execute(db)
        .await
        .unwrap();
    // Nothing may point at a thread that does not exist.
    assert!(fails(db, format!(
        "INSERT INTO messages(id,channel_id,author_id,content,thread_id) VALUES('{}','{channel}','{}','orphan','no-such-thread')",
        ulid::Ulid::new(), t.admin.user.id)).await.contains("FOREIGN KEY"));

    // Deleting the channel takes every thread row with it, through the
    // threads/messages reference cycle, and leaves no dangling reference.
    // Delete and verify on one acquired connection, which is what a storage
    // invariant should be asserted against. Reading through the pool instead was
    // observed to return one pre-delete snapshot, and only after this test's
    // deliberate foreign key failure. The cause of that is not established and is
    // not this test's subject.
    let mut one = db.acquire().await.unwrap();
    assert_eq!(
        sqlx::query("DELETE FROM channels WHERE id=?")
            .bind(&channel)
            .execute(&mut *one)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    for table in ["threads", "thread_read_state", "thread_tasks"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table}"
            )))
            .fetch_one(&mut *one)
            .await
            .unwrap(),
            0,
            "{table}"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM messages WHERE channel_id=?")
            .bind(&channel)
            .fetch_one(&mut *one)
            .await
            .unwrap(),
        0
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *one)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn deleting_a_root_message_takes_its_thread_and_replies() {
    // Storage-level fallback. The HTTP refusal that keeps a live conversation
    // intact is server work in the next PR; this proves nothing is left dangling
    // if a root ever does go, for instance through a channel deletion.
    let t = Test::new().await;
    let channel = t.general().await;
    let root = t
        .post(
            &format!("/channels/{channel}/messages"),
            &t.admin.token,
            json!({"content":"root"}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let db = &t.state.db;
    let thread = ulid::Ulid::new().to_string();
    sqlx::query("INSERT INTO threads(id,channel_id,root_message_id,title,created_by) VALUES(?,?,?,'root',?)")
        .bind(&thread).bind(&channel).bind(&root).bind(&t.admin.user.id)
        .execute(db).await.unwrap();
    for _ in 0..2 {
        sqlx::query("INSERT INTO messages(id,channel_id,author_id,content,thread_id) VALUES(?,?,?,'reply',?)")
            .bind(ulid::Ulid::new().to_string()).bind(&channel).bind(&t.admin.user.id).bind(&thread)
            .execute(db).await.unwrap();
    }
    sqlx::query("DELETE FROM messages WHERE id=?")
        .bind(&root)
        .execute(db)
        .await
        .unwrap();
    assert_eq!(count(db, "SELECT count(*) FROM threads".into()).await, 0);
    assert_eq!(
        count(
            db,
            format!("SELECT count(*) FROM messages WHERE thread_id='{thread}'")
        )
        .await,
        0
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(db)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn upgrading_a_populated_pre_thread_database_rewrites_nothing() {
    let t = Test::new().await;
    let path = t.dir.join("legacy.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let db = SqlitePool::connect_with(options.clone()).await.unwrap();
    let before = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(sqlx::migrate!().iter().take(18).cloned().collect()),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    assert_eq!(before.iter().last().unwrap().version, 18);
    before.run(&db).await.unwrap();
    for sql in [
        "INSERT INTO users(id,username,display_name,role) VALUES('u1','ada','Ada','admin'),('u2','bo','Bo','member')",
        "INSERT INTO channels(id,name,kind,position) VALUES('c1','general','text',0)",
        "INSERT INTO messages(id,channel_id,author_id,content,mentions_indexed) VALUES('m1','c1','u1','the original question',1)",
        // An old quoted reply must keep pointing where it pointed. Its content and
        // mention row agree and are already indexed, as a live v18 database's are,
        // so startup backfill has nothing to reindex.
        "INSERT INTO messages(id,channel_id,author_id,content,reply_to,mentions_indexed) VALUES('m2','c1','u2','@ada quoting you','m1',1)",
        "INSERT INTO reactions(message_id,user_id,emoji) VALUES('m1','u2','eyes')",
        "INSERT INTO message_mentions(message_id,user_id) VALUES('m2','u1')",
        "INSERT INTO read_state(user_id,channel_id,last_read_id) VALUES('u2','c1','m1')",
        "INSERT INTO uploads(id,channel_id,owner_id,message_id,filename,content_type,size,offset,complete,touched_at) VALUES('up1','c1','u1','m1','notes.txt','text/plain',3,3,1,1700000000)",
        "INSERT INTO objects(id,channel_id,message_id,kind,name,state,version,created_by) VALUES('o1','c1','m2','canvas','Sketch','{}',2,'u1')",
    ] {
        sqlx::query(sql).execute(&db).await.unwrap();
    }
    let tables = [
        ("users", "id,username,display_name,password_hash,avatar_url,bot,owner_id,role"),
        ("channels", "id,name,category_id,kind,position,dm_key"),
        ("messages", "id,channel_id,author_id,content,reply_to,created_at,edited_at,mentions_indexed"),
        ("reactions", "message_id,user_id,emoji"),
        ("message_mentions", "message_id,user_id"),
        ("read_state", "user_id,channel_id,last_read_id"),
        ("uploads", "id,channel_id,owner_id,message_id,filename,content_type,size,offset,complete,touched_at,thumbnail_ready"),
        ("objects", "id,channel_id,message_id,kind,name,state,version,thumbnail_upload_id,created_by,created_at,updated_at"),
    ];
    let mut snapshot = Vec::new();
    for (table, columns) in tables {
        snapshot.push(dump(&db, table, columns).await);
    }
    db.close().await;

    // Upgrade through the same path a real server takes on startup.
    let upgraded = den_server::AppState::open(
        path.clone(),
        t.dir.join("legacy-uploads"),
        t.dir.join("legacy-bootstrap.key"),
        t.url.clone(),
        1024 * 1024,
    )
    .await
    .unwrap();
    let db = &upgraded.db;
    for ((table, columns), was) in tables.into_iter().zip(snapshot) {
        assert_eq!(dump(db, table, columns).await, was, "{table} changed");
    }
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(db)
        .await
        .unwrap()
        .is_empty());
    // The new column exists, defaults to NULL, and reinterprets no history: the old
    // quoted reply is still a main-conversation message.
    assert_eq!(
        count(
            db,
            "SELECT count(*) FROM messages WHERE thread_id IS NOT NULL".into()
        )
        .await,
        0
    );
    assert!(sqlx::query("PRAGMA table_info(messages)")
        .fetch_all(db)
        .await
        .unwrap()
        .iter()
        .any(|r| r.get::<String, _>("name") == "thread_id"));
    for table in ["threads", "thread_read_state", "thread_tasks"] {
        assert_eq!(count(db, format!("SELECT count(*) FROM {table}")).await, 0);
    }
    // Threads attach to the migrated history rather than replacing it.
    sqlx::query("INSERT INTO threads(id,channel_id,root_message_id,title,created_by) VALUES('t1','c1','m1','the original question','u1')")
        .execute(db).await.unwrap();
    sqlx::query("UPDATE messages SET thread_id='t1' WHERE id='m2'")
        .execute(db)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT reply_to FROM messages WHERE id='m2'")
            .fetch_one(db)
            .await
            .unwrap(),
        "m1"
    );
    upgraded.db.close().await;
}

#[tokio::test]
async fn requests_without_the_new_context_fields_keep_working() {
    let t = Test::new().await;
    let channel = t.general().await;
    // Exactly what a pre-threads client sends.
    let message = t
        .post(
            &format!("/channels/{channel}/messages"),
            &t.admin.token,
            json!({"content":"legacy send"}),
        )
        .await;
    assert!(message.get("thread_id").is_none(), "{message}");
    assert!(message.get("thread").is_none(), "{message}");
    let id = message["id"].as_str().unwrap();
    assert_eq!(
        t.req(
            Method::PUT,
            &format!("/channels/{channel}/read"),
            &t.admin.token
        )
        .json(&json!({"message_id":id}))
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    t.post(
        &format!("/channels/{channel}/objects"),
        &t.admin.token,
        json!({"kind":"canvas","name":"legacy canvas"}),
    )
    .await;
    // The flat listing is still the default; no new parameter is required.
    let listed: Vec<Message> = t
        .req(
            Method::GET,
            &format!("/channels/{channel}/messages"),
            &t.admin.token,
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(listed.iter().any(|m| m.id == id && m.thread_id.is_none()));
    // A malformed known payload is still an error, not a tolerated unknown.
    assert_eq!(
        t.req(
            Method::POST,
            &format!("/channels/{channel}/messages"),
            &t.admin.token
        )
        .json(&json!({"reply_to":null}))
        .send()
        .await
        .unwrap()
        .status(),
        422
    );
}
