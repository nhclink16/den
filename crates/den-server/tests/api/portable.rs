use super::*;
use std::{
    fs,
    io::{Read, Write},
    process::{Command, Output, Stdio},
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

fn run(t: &Test, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_den-server"))
        .args(args)
        .env("DEN_DB", t.dir.join("den.db"))
        .env("DEN_UPLOADS", t.dir.join("uploads"))
        .output()
        .unwrap()
}
fn succeeded(out: Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn rewrite(input: &std::path::Path, output: &std::path::Path, change: &str) {
    let mut source = ZipArchive::new(fs::File::open(input).unwrap()).unwrap();
    let mut out = ZipWriter::new(fs::File::create(output).unwrap());
    for i in 0..source.len() {
        let mut file = source.by_index(i).unwrap();
        let name = file.name().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "manifest.json" && matches!(change, "newer" | "divergent") {
            let mut v: Value = serde_json::from_slice(&data).unwrap();
            if change == "newer" {
                v["schema_version"] = json!(9999);
            } else {
                v["migrations"][0]["checksum"] = json!("bad");
            }
            data = serde_json::to_vec(&v).unwrap();
        }
        if name == "den.db" && change == "checksum" {
            data[0] ^= 1;
        }
        out.start_file(&name, SimpleFileOptions::default()).unwrap();
        out.write_all(&data).unwrap();
    }
    if change == "traversal" {
        out.start_file("../escaped", SimpleFileOptions::default())
            .unwrap();
        out.write_all(b"bad").unwrap();
    }
    if change == "symlink" {
        out.add_symlink(
            format!("uploads/{}", ulid::Ulid::new()),
            "/etc/passwd",
            SimpleFileOptions::default(),
        )
        .unwrap();
    }
    out.finish().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn portable_roundtrip_preserves_uploads_recordings_and_revokes_credentials() {
    let t = Test::new().await;
    let channel = t.general().await;
    t.post(
        &format!("/channels/{channel}/messages"),
        &t.admin.token,
        json!({"content":"portable history"}),
    )
    .await;
    let partial = t.post("/uploads", &t.admin.token, json!({"channel_id":channel,"filename":"partial.txt","content_type":"text/plain","size":6})).await;
    let upload = partial["id"].as_str().unwrap();
    let response = t
        .req(Method::PATCH, &format!("/uploads/{upload}"), &t.admin.token)
        .header("Upload-Offset", "0")
        .body("abc")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let token = t
        .post("/tokens", &t.admin.token, json!({"name":"portable-token"}))
        .await;
    let enrollment = t.post("/hosts/enroll", &t.admin.token, json!({})).await;
    let host = t.post("/hosts/login", "", json!({"code":enrollment["code"].as_str().unwrap().rsplit_once('#').unwrap().1,"name":"portable-host"})).await;
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
    tokio::time::sleep(Duration::from_millis(40)).await;
    let terminal = t
        .post(
            &format!("/hosts/{host_id}/sessions"),
            &t.admin.token,
            json!({"channel_id":channel}),
        )
        .await;
    let id = terminal["id"].as_str().unwrap();
    socket
        .send(Frame::Binary(
            serde_json::to_vec(&HostFrame::Output {
                session_id: id.into(),
                bytes: b"recorded output\r\n".to_vec(),
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
    let recording = fs::read(t.dir.join(format!("uploads/{id}.recording"))).unwrap();
    t.post(
        "/invites",
        &t.admin.token,
        json!({"uses":1,"expires_in_hours":1}),
    )
    .await;
    t.post("/hosts/enroll", &t.admin.token, json!({})).await;
    sqlx::query("INSERT INTO grants(id,host_id,grantee_id,capability,created_by,created_at) VALUES(?,?,?,'terminal_control',?,1)").bind(ulid::Ulid::new().to_string()).bind(host_id).bind(&t.admin.user.id).bind(&t.admin.user.id).execute(&t.state.db).await.unwrap();
    drop(socket);
    t.task.abort();
    t.state.db.close().await;
    fs::write(t.dir.join("uploads/host.toml"), "not archive data").unwrap();
    fs::create_dir(t.dir.join("uploads/.ssh")).unwrap();
    fs::write(t.dir.join("uploads/.ssh/key"), "not archive data").unwrap();
    let archive = t.dir.join("backup.zip");
    succeeded(run(&t, &["export", archive.to_str().unwrap()]));
    let mut zip = ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
    assert!(zip.by_name("uploads/host.toml").is_err());
    assert!(zip.by_name("uploads/.ssh/key").is_err());
    assert!(zip.by_name(&format!("uploads/{upload}.part")).is_ok());
    assert!(zip.by_name(&format!("uploads/{id}.recording")).is_ok());
    let target = t.dir.join("restored");
    succeeded(run(
        &t,
        &[
            "import",
            archive.to_str().unwrap(),
            "--into",
            target.to_str().unwrap(),
        ],
    ));
    assert_eq!(
        fs::read(target.join(format!("uploads/{upload}.part"))).unwrap(),
        b"abc"
    );
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", target.join("den.db").display()))
        .await
        .unwrap();
    for table in ["sessions", "tokens", "invites", "host_enrollments"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM grants WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let hash: String = sqlx::query_scalar("SELECT token_hash FROM hosts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hash.starts_with("invalidated:"));
    let state: String = sqlx::query_scalar("SELECT state FROM objects WHERE id=?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let state: Value = serde_json::from_str(&state).unwrap();
    assert!(state["terminal"]["ended_at"].is_number());
    let recording_id = state["terminal"]["recording_upload_id"].as_str().unwrap();
    assert_eq!(
        fs::read(target.join("uploads").join(recording_id)).unwrap(),
        recording
    );
    pool.close().await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let url = format!("http://{addr}");
    let mut server = Command::new(env!("CARGO_BIN_EXE_den-server"))
        .env("DEN_DB", target.join("den.db"))
        .env("DEN_UPLOADS", target.join("uploads"))
        .env("DEN_BIND", addr.to_string())
        .env("DEN_ORIGIN", &url)
        .env("DEN_BOOTSTRAP_FILE", target.join("bootstrap.key"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    struct Stop<'a>(&'a mut std::process::Child);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _stop = Stop(&mut server);
    for _ in 0..100 {
        if t.http.get(format!("{url}/health")).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    for old in [&t.admin.token, token["token"].as_str().unwrap()] {
        assert_eq!(
            t.http
                .get(format!("{url}/channels"))
                .bearer_auth(old)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    let login: Session = t
        .http
        .post(format!("{url}/auth/login"))
        .json(&json!({"username":"admin","password":"test-password-123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let channels: Vec<Channel> = t
        .http
        .get(format!("{url}/channels"))
        .bearer_auth(&login.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(channels.iter().any(|c| c.id == channel));
    let messages: Value = t
        .http
        .get(format!("{url}/channels/{channel}/messages"))
        .bearer_auth(&login.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(messages
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["content"] == "portable history"));
    assert_eq!(
        t.http
            .patch(format!("{url}/uploads/{upload}"))
            .bearer_auth(&login.token)
            .header("Upload-Offset", "3")
            .body("def")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        t.http
            .post(format!("{url}/uploads/{upload}/complete"))
            .bearer_auth(&login.token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let range = t
        .http
        .get(format!("{url}/uploads/{upload}/file"))
        .bearer_auth(&login.token)
        .header("Range", "bytes=1-4")
        .send()
        .await
        .unwrap();
    assert_eq!(range.status(), 206);
    assert_eq!(range.bytes().await.unwrap(), &b"bcde"[..]);
    let replay = t
        .http
        .get(format!("{url}/uploads/{recording_id}/file"))
        .bearer_auth(&login.token)
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), 200);
    assert_eq!(replay.bytes().await.unwrap(), recording);
    let recovery = t.dir.join("recovery");
    succeeded(run(
        &t,
        &[
            "import",
            archive.to_str().unwrap(),
            "--into",
            recovery.to_str().unwrap(),
            "--keep-credentials",
        ],
    ));
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite://{}", recovery.join("den.db").display()))
            .await
            .unwrap();
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tokens")
            .fetch_one(&pool)
            .await
            .unwrap()
            > 0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM grants WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let hash: String = sqlx::query_scalar("SELECT token_hash FROM hosts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!hash.starts_with("invalidated:"));
    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn portable_refuses_live_or_invalid_archives_and_migrates_old_schema() {
    let t = Test::new().await;
    let archive = t.dir.join("backup.zip");
    let running = run(&t, &["export", archive.to_str().unwrap()]);
    assert!(!running.status.success());
    assert!(String::from_utf8_lossy(&running.stderr).contains("stop Den"));
    assert!(!archive.exists());
    t.task.abort();
    t.state.db.close().await;
    succeeded(run(&t, &["export", archive.to_str().unwrap()]));
    for change in ["newer", "divergent", "traversal", "symlink", "checksum"] {
        let bad = t.dir.join(format!("{change}.zip"));
        rewrite(&archive, &bad, change);
        let dest = t.dir.join(format!("target-{change}"));
        assert!(
            !run(
                &t,
                &[
                    "import",
                    bad.to_str().unwrap(),
                    "--into",
                    dest.to_str().unwrap()
                ]
            )
            .status
            .success(),
            "accepted {change}"
        );
        assert!(!dest.exists());
    }
    assert!(!t.dir.join("escaped").exists());
    assert!(!run(
        &t,
        &[
            "import",
            archive.to_str().unwrap(),
            "--into",
            t.dir.to_str().unwrap()
        ]
    )
    .status
    .success());
    let old = t.dir.join("old");
    fs::create_dir(&old).unwrap();
    fs::create_dir(old.join("uploads")).unwrap();
    let db = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(old.join("den.db"))
            .create_if_missing(true),
    )
    .await
    .unwrap();
    let migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(sqlx::migrate!().iter().take(4).cloned().collect()),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    migrator.run(&db).await.unwrap();
    db.close().await;
    let oldzip = t.dir.join("old.zip");
    succeeded(
        Command::new(env!("CARGO_BIN_EXE_den-server"))
            .args(["export", oldzip.to_str().unwrap()])
            .env("DEN_DB", old.join("den.db"))
            .env("DEN_UPLOADS", old.join("uploads"))
            .output()
            .unwrap(),
    );
    let migrated = t.dir.join("migrated");
    fs::create_dir(&migrated).unwrap();
    succeeded(run(
        &t,
        &[
            "import",
            oldzip.to_str().unwrap(),
            "--into",
            migrated.to_str().unwrap(),
        ],
    ));
    let db = sqlx::SqlitePool::connect(&format!("sqlite://{}", migrated.join("den.db").display()))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&db)
            .await
            .unwrap(),
        sqlx::migrate!().iter().count() as i64
    );
    db.close().await;
}
