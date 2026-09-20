//! Offline archives. These are operator backups and include private messages and hashes.
mod archive;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqliteConnectOptions, AssertSqlSafe, Connection, Row, SqliteConnection};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
const MAX_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 100_000;
const MAX_MANIFEST: u64 = 32 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Migration {
    version: i64,
    checksum: String,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FileEntry {
    size: u64,
    sha256: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    archive_version: u32,
    schema_version: i64,
    migrations: Vec<Migration>,
    files: BTreeMap<String, FileEntry>,
    instance_name: String,
    exported_at: i64,
}

pub async fn command(args: Vec<String>) -> Result<bool> {
    if args.is_empty() {
        return Ok(false);
    }
    let usage =
        "den-server export <zip> | import <zip> --into <empty-data-dir> [--keep-credentials]";
    if matches!(args[0].as_str(), "--help" | "-h") {
        println!(
            "{usage}\nWith no arguments, start the server. Export uses DEN_DB and DEN_UPLOADS."
        );
        return Ok(true);
    }
    ensure!(matches!(args[0].as_str(), "export" | "import"), "{usage}");
    // Refuse both system and user instances. Missing systemd is normal on other OSes.
    for user in [false, true] {
        let mut cmd = std::process::Command::new("systemctl");
        if user {
            cmd.arg("--user");
        }
        if cmd
            .args(["is-active", "--quiet", "den-server.service"])
            .output()
            .is_ok_and(|o| o.status.success())
        {
            bail!("Stop den-server.service before exporting or importing");
        }
    }
    match args[0].as_str() {
        "export" => {
            ensure!(args.len() == 2, "{usage}");
            export(
                &PathBuf::from(std::env::var_os("DEN_DB").unwrap_or_else(|| "data/den.db".into())),
                &PathBuf::from(
                    std::env::var_os("DEN_UPLOADS").unwrap_or_else(|| "data/uploads".into()),
                ),
                Path::new(&args[1]),
            )
            .await?;
        }
        "import" => {
            ensure!(args.len() == 4 || args.len() == 5, "{usage}");
            ensure!(
                args[2] == "--into" && (args.len() == 4 || args[4] == "--keep-credentials"),
                "{usage}"
            );
            import(Path::new(&args[1]), Path::new(&args[3]), args.len() == 5).await?;
        }
        _ => unreachable!(),
    }
    Ok(true)
}

async fn connect(path: &Path) -> Result<SqliteConnection> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "Database must be a regular file"
    );
    Ok(SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5)),
    )
    .await?)
}
async fn migrations(db: &mut SqliteConnection) -> Result<Vec<Migration>> {
    let rows =
        sqlx::query("SELECT version,checksum,success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(db)
            .await?;
    rows.into_iter()
        .map(|r| {
            ensure!(r.get::<bool, _>("success"), "Incomplete migration");
            Ok(Migration {
                version: r.get("version"),
                checksum: hex(&r.get::<Vec<u8>, _>("checksum")),
            })
        })
        .collect()
}
fn validate_schema(m: &Manifest) -> Result<()> {
    ensure!(m.archive_version == 1, "Unsupported archive version");
    let expected: Vec<_> = MIGRATOR
        .iter()
        .map(|m| Migration {
            version: m.version,
            checksum: hex(&m.checksum),
        })
        .collect();
    ensure!(
        m.schema_version <= expected.last().unwrap().version,
        "Archive schema is newer than this binary"
    );
    ensure!(
        !m.migrations.is_empty()
            && m.migrations.len() <= expected.len()
            && m.migrations == expected[..m.migrations.len()]
            && m.schema_version == m.migrations.last().unwrap().version,
        "Divergent migration history"
    );
    Ok(())
}
async fn integrity(db: &mut SqliteConnection) -> Result<()> {
    let result: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&mut *db)
        .await?;
    ensure!(result == ["ok"], "Database integrity check failed");
    ensure!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(db)
            .await?
            .is_empty(),
        "Database has invalid foreign keys"
    );
    Ok(())
}
async fn instance_name(db: &mut SqliteConnection) -> Result<String> {
    let cols = sqlx::query("PRAGMA table_info(settings)")
        .fetch_all(&mut *db)
        .await?;
    if cols
        .iter()
        .any(|r| r.get::<String, _>("name") == "instance_name")
    {
        Ok(
            sqlx::query_scalar("SELECT instance_name FROM settings WHERE id=1")
                .fetch_one(db)
                .await?,
        )
    } else {
        Ok("Den".into())
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn export(db_path: &Path, uploads: &Path, output: &Path) -> Result<()> {
    ensure!(!output.try_exists()?, "Export destination already exists");
    let mut db = connect(db_path).await?;
    sqlx::query("PRAGMA busy_timeout=0")
        .execute(&mut db)
        .await?;
    sqlx::query("PRAGMA locking_mode=EXCLUSIVE")
        .execute(&mut db)
        .await?;
    // BEGIN EXCLUSIVE alone does not exclude WAL readers. locking_mode does.
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut db)
        .await
        .context("Database is in use; stop Den before export")?;
    sqlx::query("COMMIT").execute(&mut db).await?;
    integrity(&mut db).await?;
    let migrations = migrations(&mut db).await?;
    let mut manifest = Manifest {
        archive_version: 1,
        schema_version: migrations
            .last()
            .context("Missing migration history")?
            .version,
        migrations,
        files: BTreeMap::new(),
        instance_name: instance_name(&mut db).await?,
        exported_at: crate::now(),
    };
    validate_schema(&manifest)?;
    let scratch = archive::Scratch::new(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let snapshot = scratch.0.join("den.db");
    sqlx::query("VACUUM INTO ?")
        .bind(snapshot.to_str().context("Non-UTF8 archive path")?)
        .execute(&mut db)
        .await?;
    check_uploads(&mut db, uploads).await?;
    let packed = scratch.0.join("export.zip");
    archive::pack(&snapshot, uploads, &packed, &mut manifest)?;
    fs::hard_link(&packed, output)
        .context("Cannot install export without overwriting an existing file")?;
    archive::sync_dir(scratch.0.parent().unwrap())?;
    db.close().await?;
    println!(
        "Exported {} files, {} bytes; schema {}; instance {:?}",
        manifest.files.len(),
        manifest.files.values().map(|f| f.size).sum::<u64>(),
        manifest.schema_version,
        manifest.instance_name
    );
    Ok(())
}

pub async fn import(input: &Path, target: &Path, keep: bool) -> Result<()> {
    archive::empty_target(target)?;
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let scratch = archive::Scratch::new(parent)?;
    let manifest = archive::unpack(input, &scratch.0)?;
    validate_schema(&manifest)?;
    let db_path = scratch.0.join("den.db");
    let uploads = scratch.0.join("uploads");
    let mut db = connect(&db_path).await?;
    ensure!(
        migrations(&mut db).await? == manifest.migrations,
        "Manifest does not match database migrations"
    );
    ensure!(
        instance_name(&mut db).await? == manifest.instance_name,
        "Manifest instance name does not match database"
    );
    integrity(&mut db).await?;
    check_uploads(&mut db, &uploads).await?;
    db.close().await?;
    // Use the same migration path as normal startup, including the FK-safe table rebuild.
    let state = crate::AppState::open(
        db_path.clone(),
        uploads.clone(),
        scratch.0.join("bootstrap.key"),
        "http://127.0.0.1:7000".into(),
        1024 * 1024 * 1024,
    )
    .await?;
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM terminal_sessions")
        .fetch_all(&state.db)
        .await?;
    let mut ended = 0;
    for id in ids {
        ensure!(id.parse::<ulid::Ulid>().is_ok(), "Invalid terminal ID");
        if crate::terminal::load(&state, &id)
            .await
            .map_err(|_| anyhow::anyhow!("Invalid terminal state"))?
            .ended_at
            .is_none()
        {
            crate::terminal::finish(&state, &id)
                .await
                .map_err(|_| anyhow::anyhow!("Cannot finalize archived terminal recording"))?;
            ended += 1;
        }
    }
    let mut invalidated = Vec::new();
    if !keep {
        for table in [
            "sessions",
            "tokens",
            "invites",
            "host_enrollments",
            // Importing someone else's archive must not resurrect live Spotify grants.
            "spotify_accounts",
        ] {
            // sqlx 0.9 only accepts `&'static str` as a query; the table name is
            // interpolated, so the assertion is required. The list above is literal.
            let count = sqlx::query(AssertSqlSafe(format!("DELETE FROM {table}")))
                .execute(&state.db)
                .await?
                .rows_affected();
            invalidated.push(format!("{table}={count}"));
        }
        let count = sqlx::query(
            "UPDATE hosts SET token_hash='invalidated:'||id, online=0, direct_url=NULL",
        )
        .execute(&state.db)
        .await?
        .rows_affected();
        invalidated.push(format!("host credentials={count}"));
        let count = sqlx::query("UPDATE grants SET revoked_at=? WHERE revoked_at IS NULL")
            .bind(crate::now())
            .execute(&state.db)
            .await?
            .rows_affected();
        invalidated.push(format!("grants={count}"));
    }
    sqlx::query("UPDATE hosts SET online=0,direct_url=NULL")
        .execute(&state.db)
        .await?;
    state.db.close().await;
    drop(state);
    let mut db = connect(&db_path).await?;
    integrity(&mut db).await?;
    check_uploads(&mut db, &uploads).await?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&mut db)
        .await?;
    db.close().await?;
    archive::sync_files(&scratch.0)?;
    archive::empty_target(target)?;
    fs::rename(&scratch.0, target).context("Cannot atomically install into target directory")?;
    archive::sync_dir(parent)?;
    println!(
        "Imported {} archived files into {}; ended {ended} terminal sessions. {}",
        manifest.files.len(),
        target.display(),
        if keep {
            "Credentials and grants preserved for disaster recovery.".into()
        } else {
            format!(
                "Invalidated {}. Log in again; re-enroll hosts.",
                invalidated.join(", ")
            )
        }
    );
    Ok(())
}

async fn check_uploads(db: &mut SqliteConnection, root: &Path) -> Result<()> {
    for row in sqlx::query("SELECT id,size,offset,complete FROM uploads")
        .fetch_all(db)
        .await?
    {
        let id: String = row.get("id");
        ensure!(id.parse::<ulid::Ulid>().is_ok(), "Invalid upload ID");
        let complete: bool = row.get("complete");
        let size: i64 = row.get("size");
        let offset: i64 = row.get("offset");
        ensure!(
            size > 0 && offset >= 0 && offset <= size,
            "Invalid upload state"
        );
        let mut path = root.join(if complete {
            id.clone()
        } else {
            format!("{id}.part")
        });
        // A crash between the file rename and the DB commit is resumable.
        if !complete && !path.try_exists()? && offset == size {
            path = root.join(&id);
        }
        let meta = fs::symlink_metadata(&path).context("Archive is missing an upload")?;
        ensure!(
            meta.file_type().is_file()
                && if complete {
                    meta.len() == size as u64 && offset == size
                } else {
                    meta.len() >= offset as u64 && meta.len() <= size as u64
                },
            "Upload size or file type does not match database"
        );
    }
    Ok(())
}
