use anyhow::{ensure, Result};
use den_core::HostFrame;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{Read, Write},
    sync::{Arc, Condvar, Mutex},
};

const WINDOW: usize = 1024 * 1024;
const HISTORY: usize = 256 * 1024;
#[derive(Default)]
struct Feed {
    pending: VecDeque<u8>,
    history: VecDeque<u8>,
    unacked: usize,
    eof: bool,
    exit: Option<u32>,
    reported: bool,
    closed: bool,
}
/// Replace this byte source with a libghostty-vt screen source in v2.
pub trait ScreenSource {
    fn drain(&self) -> Vec<u8>;
}
struct RawSource(Arc<(Mutex<Feed>, Condvar)>);
impl ScreenSource for RawSource {
    fn drain(&self) -> Vec<u8> {
        let mut f = self.0 .0.lock().unwrap();
        let n = f.pending.len().min(64 * 1024).min(WINDOW - f.unacked);
        f.unacked += n;
        f.pending.drain(..n).collect()
    }
}
struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    source: RawSource,
    viewers: HashSet<String>,
}
impl Drop for Pty {
    fn drop(&mut self) {
        self.source.0 .0.lock().unwrap().closed = true;
        self.source.0 .1.notify_all();
        let _ = self.killer.kill();
    }
}
#[derive(Default)]
pub struct Sessions {
    sessions: HashMap<String, Pty>,
}
impl Sessions {
    pub fn ids(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    pub fn handle(&mut self, frame: HostFrame) -> Result<()> {
        match frame {
            HostFrame::Open {
                session_id,
                cols,
                rows,
                shell,
            } => {
                ensure!(
                    (2..=500).contains(&cols) && (2..=300).contains(&rows),
                    "Invalid terminal size"
                );
                if !self.sessions.contains_key(&session_id) {
                    ensure!(self.sessions.len() < 16, "Too many sessions");
                    let pair = native_pty_system().openpty(size(cols, rows))?;
                    let shell = shell.unwrap_or_else(|| {
                        std::env::var("SHELL").unwrap_or_else(|_| {
                            if cfg!(windows) {
                                "powershell.exe"
                            } else {
                                "/bin/sh"
                            }
                            .into()
                        })
                    });
                    let mut command = CommandBuilder::new(shell);
                    command.env("TERM", "xterm-256color");
                    command.env("COLORTERM", "truecolor");
                    // A terminal is its own caller; never inherit the agent lane's Herdr identity.
                    for key in [
                        "HERDR_ENV",
                        "HERDR_WORKSPACE_ID",
                        "HERDR_TAB_ID",
                        "HERDR_PANE_ID",
                        "HERDR_SOCKET",
                    ] {
                        command.env_remove(key);
                    }
                    if let Some(home) = std::env::var_os("HOME") {
                        command.cwd(home);
                    }
                    let mut child = pair.slave.spawn_command(command)?;
                    drop(pair.slave);
                    let killer = child.clone_killer();
                    let mut reader = pair.master.try_clone_reader()?;
                    let writer = pair.master.take_writer()?;
                    let feed = Arc::new((Mutex::new(Feed::default()), Condvar::new()));
                    let output = feed.clone();
                    std::thread::spawn(move || {
                        let mut bytes = [0; 8192];
                        loop {
                            let (lock, ready) = &*output;
                            let mut f = lock.lock().unwrap();
                            while !f.closed && f.pending.len() + f.unacked >= WINDOW {
                                f = ready.wait(f).unwrap();
                            }
                            if f.closed {
                                break;
                            }
                            let limit = bytes.len().min(WINDOW - f.pending.len() - f.unacked);
                            drop(f);
                            let n = match reader.read(&mut bytes[..limit]) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            };
                            let mut f = lock.lock().unwrap();
                            f.pending.extend(&bytes[..n]);
                            f.history.extend(&bytes[..n]);
                            let excess = f.history.len().saturating_sub(HISTORY);
                            f.history.drain(..excess);
                        }
                        output.0.lock().unwrap().eof = true;
                    });
                    let exited = feed.clone();
                    std::thread::spawn(move || {
                        let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
                        exited.0.lock().unwrap().exit = Some(code);
                    });
                    self.sessions.insert(
                        session_id,
                        Pty {
                            master: pair.master,
                            writer,
                            killer,
                            source: RawSource(feed),
                            viewers: HashSet::new(),
                        },
                    );
                } else {
                    let p = self.sessions.get_mut(&session_id).unwrap();
                    p.master.resize(size(cols.saturating_sub(1).max(2), rows))?;
                    p.master.resize(size(cols, rows))?;
                    let mut f = p.source.0 .0.lock().unwrap();
                    f.pending = f.history.clone();
                    f.unacked = 0;
                    f.reported = false;
                    p.source.0 .1.notify_all();
                }
            }
            HostFrame::Input { session_id, bytes } => {
                if let Some(p) = self.sessions.get_mut(&session_id) {
                    p.writer.write_all(&bytes)?;
                    p.writer.flush()?;
                }
            }
            HostFrame::Resize {
                session_id,
                cols,
                rows,
            } => {
                ensure!(
                    (2..=500).contains(&cols) && (2..=300).contains(&rows),
                    "Invalid terminal size"
                );
                if let Some(p) = self.sessions.get(&session_id) {
                    p.master.resize(size(cols, rows))?;
                }
            }
            HostFrame::Close { session_id } => {
                self.sessions.remove(&session_id);
            }
            HostFrame::Ack { session_id, bytes } => {
                if let Some(p) = self.sessions.get(&session_id) {
                    let mut f = p.source.0 .0.lock().unwrap();
                    f.unacked = f.unacked.saturating_sub(bytes);
                    p.source.0 .1.notify_all();
                }
            }
            HostFrame::Viewer {
                session_id,
                user_id,
                name,
            } => {
                if let Some(p) = self.sessions.get_mut(&session_id) {
                    if p.viewers.insert(user_id) {
                        let name: String =
                            name.chars().filter(|c| !c.is_control()).take(100).collect();
                        let message = format!("{name} is viewing your terminal");
                        eprintln!("{message} ({session_id})");
                        #[cfg(target_os = "linux")]
                        let _ = std::process::Command::new("notify-send")
                            .arg("Den")
                            .arg(message)
                            .spawn();
                    }
                }
            }
            _ => anyhow::bail!("Unexpected host frame"),
        }
        Ok(())
    }
    pub fn dimensions(&self, id: &str) -> Option<(u16, u16)> {
        self.sessions
            .get(id)
            .and_then(|p| p.master.get_size().ok())
            .map(|s| (s.cols, s.rows))
    }
    pub fn refresh(&self, id: &str) -> Result<()> {
        if let Some(p) = self.sessions.get(id) {
            let dimensions = p.master.get_size()?;
            p.master.resize(size(
                dimensions.cols.saturating_sub(1).max(2),
                dimensions.rows,
            ))?;
            p.master.resize(dimensions)?;
        }
        Ok(())
    }
    pub fn history(&self, id: &str) -> Vec<u8> {
        self.sessions
            .get(id)
            .map(|p| {
                p.source
                    .0
                     .0
                    .lock()
                    .unwrap()
                    .history
                    .iter()
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn drain(&mut self) -> Vec<HostFrame> {
        let mut frames = Vec::new();
        let mut exited = Vec::new();
        for (id, p) in &self.sessions {
            let bytes = p.source.drain();
            if !bytes.is_empty() {
                frames.push(HostFrame::Output {
                    session_id: id.clone(),
                    bytes,
                });
            }
            let mut f = p.source.0 .0.lock().unwrap();
            if f.eof && f.pending.is_empty() && !f.reported {
                if let Some(code) = f.exit {
                    frames.push(HostFrame::Exited {
                        session_id: id.clone(),
                        code,
                    });
                    f.reported = true;
                    exited.push(id.clone());
                }
            }
        }
        // A naturally exited shell no longer needs its PTY. Releasing the
        // entry here frees active-terminal capacity; retaining the card or
        // recording is the server's job, not a live slot.
        for id in exited {
            self.sessions.remove(&id);
        }
        frames
    }
}
fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    }
}
