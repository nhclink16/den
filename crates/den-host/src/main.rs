mod direct;
mod service;
use anyhow::{ensure, Context, Result};
use den_core::{HostCredential, HostFrame, HostLogin};
use den_host::{
    pty::Sessions,
    transport::{HostTransport, WebSocketTransport},
};
use fs2::FileExt;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn directory() -> Result<PathBuf> {
    Ok(std::env::var_os("DEN_HOST_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or(
            PathBuf::from(
                std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .context("No home directory")?,
            )
            .join(".config/den"),
        ))
}
fn private_write(path: &std::path::Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        if tmp.exists() {
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    let mut f = options.open(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    std::fs::rename(tmp, path)?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(unix)]
    ensure!(
        std::process::Command::new("id").arg("-u").output()?.stdout != b"0\n",
        "den-host must run as the logged-in user, never root"
    );
    let dir = directory()?;
    std::fs::create_dir_all(&dir)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("login") => {
            let invite = args
                .get(1)
                .context("Usage: den-host login <enrollment-code>")?;
            let (url, code) = invite
                .rsplit_once('#')
                .context("Enrollment code must include the server URL")?;
            let parsed = reqwest::Url::parse(url)?;
            ensure!(
                parsed.scheme() == "https"
                    || (parsed.scheme() == "http"
                        && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
                "HTTPS required except on loopback"
            );
            ensure!(
                parsed.username().is_empty()
                    && parsed.password().is_none()
                    && parsed.query().is_none()
                    && parsed.path() == "/",
                "Expected a server origin"
            );
            let name = std::env::var("DEN_HOST_NAME")
                .ok()
                .or_else(|| {
                    std::process::Command::new("hostname")
                        .output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                })
                .unwrap_or_else(|| "my-machine".into());
            let response = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(15))
                .build()?
                .post(format!("{url}/hosts/login"))
                .json(&HostLogin {
                    code: code.into(),
                    name,
                })
                .send()
                .await?;
            ensure!(
                response.status().is_success(),
                "Enrollment rejected ({})",
                response.status()
            );
            let credential: HostCredential = response.json().await?;
            ensure!(credential.server_url == url, "Server origin mismatch");
            private_write(
                &dir.join("host.toml"),
                toml::to_string(&credential)?.as_bytes(),
            )?;
            println!(
                "Enrolled {}. Run den-host install or den-host run.",
                credential.name
            );
        }
        Some("install") => service::install()?,
        Some("status") => {
            let value: serde_json::Value = std::fs::read(dir.join("host-status.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            let fresh = value["at"]
                .as_u64()
                .is_some_and(|at| epoch().saturating_sub(at) < 15);
            println!(
                "{}",
                if fresh && value["connected"] == true {
                    "connected"
                } else {
                    "disconnected"
                }
            );
        }
        Some("run") => {
            let lock = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(dir.join("host.lock"))?;
            lock.try_lock_exclusive()
                .context("den-host is already running")?;
            let cfg: HostCredential =
                toml::from_str(&std::fs::read_to_string(dir.join("host.toml"))?)
                    .context("Run den-host login first")?;
            let url = format!("{}/hosts/ws", cfg.server_url.replacen("http", "ws", 1));
            let sessions = std::sync::Arc::new(tokio::sync::Mutex::new(Sessions::default()));
            let mut delay = 1;
            let (output, _) = tokio::sync::broadcast::channel(64);
            let (commands, mut inputs) = tokio::sync::mpsc::channel(32);
            let direct_url =
                direct::listen(cfg.clone(), sessions.clone(), output.clone(), commands)
                    .await
                    .ok();
            loop {
                private_write(
                    &dir.join("host-status.json"),
                    &serde_json::to_vec(&serde_json::json!({"connected":false,"at":epoch()}))?,
                )?;
                let connected = tokio::select! { r = WebSocketTransport::connect(&url, &cfg.token) => r, _ = tokio::signal::ctrl_c()=>break };
                if let Ok(mut socket) = connected {
                    eprintln!("Connected as {}", cfg.name);
                    delay = 1;
                    let _ = socket
                        .send(&HostFrame::Hello {
                            direct_url: direct_url.clone(),
                        })
                        .await;
                    let mut tick = tokio::time::interval(Duration::from_millis(16));
                    let mut status = tokio::time::interval(Duration::from_secs(5));
                    loop {
                        tokio::select! {
                            _ = tokio::signal::ctrl_c() => return Ok(()),
                            _ = status.tick() => { private_write(&dir.join("host-status.json"), &serde_json::to_vec(&serde_json::json!({"connected":true,"at":epoch()}))?)?; }
                            frame = socket.receive() => match frame {
                                Ok(Some(HostFrame::Replay{session_id,connection_id})) => {
                                    let bytes=sessions.lock().await.history(&session_id);
                                    if socket.send(&HostFrame::Scrollback{session_id:session_id.clone(),connection_id,bytes}).await.is_err(){break;}
                                    if let Some((cols,rows))=sessions.lock().await.dimensions(&session_id){let _=output.send(HostFrame::Resize{session_id:session_id.clone(),cols,rows});}
                                    let _=sessions.lock().await.refresh(&session_id);
                                }
                                Ok(Some(frame)) => {if matches!(frame,HostFrame::Resize{..}) {let _=output.send(frame.clone());} if let Err(e) = sessions.lock().await.handle(frame) { eprintln!("PTY operation failed: {e}"); }}, _ => break
                            },
                            Some(frame) = inputs.recv() => { let _=sessions.lock().await.handle(frame); },
                            _ = tick.tick() => {
                                let mut failed = false;
                                for frame in sessions.lock().await.drain() { let _=output.send(frame.clone()); if !matches!(tokio::time::timeout(Duration::from_secs(5), socket.send(&frame)).await, Ok(Ok(()))) { failed=true; break; } }
                                if failed {break;}
                            }
                        }
                    }
                    eprintln!("Disconnected; PTYs retained");
                }
                tokio::select! { _=tokio::time::sleep(Duration::from_secs(delay))=>(), _=tokio::signal::ctrl_c()=>break }
                delay = (delay * 2).min(30);
            }
        }
        _ => println!("den-host login <enrollment-code> | install | run | status"),
    }
    Ok(())
}
fn epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
