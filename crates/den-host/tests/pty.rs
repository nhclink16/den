use den_core::HostFrame;
use den_host::pty::Sessions;
use std::time::{Duration, Instant};
fn open() -> HostFrame {
    HostFrame::Open {
        session_id: "test".into(),
        cols: 80,
        rows: 24,
        shell: Some("/bin/sh".into()),
    }
}
fn input(s: &mut Sessions, text: &str) {
    s.handle(HostFrame::Input {
        session_id: "test".into(),
        bytes: text.as_bytes().to_vec(),
    })
    .unwrap();
}
fn until(s: &mut Sessions, needle: &str) -> String {
    let mut out = String::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        for f in s.drain() {
            if let HostFrame::Output { session_id, bytes } = f {
                out.push_str(&String::from_utf8_lossy(&bytes));
                s.handle(HostFrame::Ack {
                    session_id,
                    bytes: bytes.len(),
                })
                .unwrap();
            }
        }
        if out.contains(needle) {
            return out;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    panic!("Missing {needle:?} in {out:?}");
}
#[test]
#[cfg(unix)]
fn real_pty_input_resize_reattach_and_backpressure() {
    let mut s = Sessions::default();
    s.handle(open()).unwrap();
    input(&mut s, "stty -echo; X=DEN_PERSIST; printf 'DEN_%s\\n' OK\n");
    until(&mut s, "DEN_OK");
    s.handle(HostFrame::Resize {
        session_id: "test".into(),
        cols: 99,
        rows: 31,
    })
    .unwrap();
    input(&mut s, "stty size\n");
    until(&mut s, "31 99");
    s.handle(open()).unwrap();
    until(&mut s, "DEN_OK");
    input(&mut s, "printf '%s\\n' \"$X\"\n");
    until(&mut s, "DEN_PERSIST");
    input(
        &mut s,
        "head -c 2097152 /dev/zero | tr '\\0' x; printf 'FLOW_DONE\\n'\n",
    );
    let mut unacked = 0;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        for f in s.drain() {
            if let HostFrame::Output { bytes, .. } = f {
                unacked += bytes.len();
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    assert_eq!(unacked, 1024 * 1024);
    s.handle(HostFrame::Ack {
        session_id: "test".into(),
        bytes: unacked,
    })
    .unwrap();
    until(&mut s, "FLOW_DONE");
    input(&mut s, "exit 7\n");
    let start = Instant::now();
    let mut exited = false;
    while start.elapsed() < Duration::from_secs(5) {
        if s.drain()
            .iter()
            .any(|f| matches!(f, HostFrame::Exited { code: 7, .. }))
        {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    assert!(exited);
    s.handle(HostFrame::Close {
        session_id: "test".into(),
    })
    .unwrap();
}
fn wait_exited(s: &mut Sessions, id: &str) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        for f in s.drain() {
            match f {
                HostFrame::Output { session_id, bytes } => {
                    let n = bytes.len();
                    s.handle(HostFrame::Ack {
                        session_id,
                        bytes: n,
                    })
                    .unwrap();
                }
                HostFrame::Exited { session_id, .. } if session_id == id => return,
                HostFrame::Exited { session_id, .. } => {
                    panic!("Unexpected exit for {session_id:?}, waiting for {id:?}")
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    panic!("Session {id:?} never exited");
}
#[test]
#[cfg(unix)]
fn naturally_exited_sessions_release_capacity() {
    let mut s = Sessions::default();
    for i in 0..16 {
        let id = format!("exited-{i}");
        s.handle(HostFrame::Open {
            session_id: id.clone(),
            cols: 80,
            rows: 24,
            shell: Some("/bin/sh".into()),
        })
        .unwrap();
        s.handle(HostFrame::Input {
            session_id: id.clone(),
            bytes: b"exit\n".to_vec(),
        })
        .unwrap();
        wait_exited(&mut s, &id);
    }
    s.handle(HostFrame::Open {
        session_id: "exited-16".into(),
        cols: 80,
        rows: 24,
        shell: Some("/bin/sh".into()),
    })
    .unwrap();
    s.handle(HostFrame::Input {
        session_id: "exited-16".into(),
        bytes: b"exit\n".to_vec(),
    })
    .unwrap();
    wait_exited(&mut s, "exited-16");
}
