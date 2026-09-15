use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    net::TcpListener,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tungstenite::Message;

// Always reap the CLI, including when an assertion fails.
struct Tail(Child);
impl Drop for Tail {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn tail_skips_unknown_events_and_keeps_the_same_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let mut tail = Tail(
        Command::new(env!("CARGO_BIN_EXE_den"))
            .args(["--url", &format!("http://{address}"), "tail"])
            .env("DEN_TOKEN", "fixture-token")
            .env(
                "DEN_CONFIG",
                std::env::temp_dir()
                    .join(format!(
                        "den-tail-{}-{}",
                        std::process::id(),
                        address.port()
                    ))
                    .join("credentials.json"),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let (lines, received) = mpsc::channel();
    let stdout = tail.0.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if lines.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    tail.0.try_wait().unwrap().is_none(),
                    "tail exited before connecting"
                );
                assert!(Instant::now() < deadline, "tail did not connect");
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accept: {error}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut socket = tungstenite::accept(stream).unwrap();
    let before = json!({"type":"presence","user_id":"alice","online":true});
    let after = json!({"type":"presence","user_id":"alice","online":false});
    for event in [
        before.clone(),
        json!({"type":"future_notification","payload":{"nested":[1,true,null]}}),
        after.clone(),
    ] {
        socket
            .send(Message::Text(event.to_string().into()))
            .unwrap();
    }
    for expected in [before, after] {
        let line = received
            .recv_timeout(Duration::from_secs(10))
            .expect("tail must print the next known event on the same connection");
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), expected);
    }
    assert!(
        tail.0.try_wait().unwrap().is_none(),
        "tail must remain running"
    );

    // Tolerating future types must not hide a broken payload for a known type.
    socket
        .send(Message::Text(
            r#"{"type":"presence","user_id":"alice"}"#.into(),
        ))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = tail.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "malformed known event was swallowed"
        );
        thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success());
    let mut stderr = String::new();
    tail.0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.contains("missing field `online`"), "{stderr}");
    reader.join().unwrap();
    assert!(
        received.try_recv().is_err(),
        "unknown events must not be printed"
    );
}
