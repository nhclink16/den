//! `den tail` against a mock event stream: it reconnects after a dropped socket,
//! re-sends credentials every time, and stops when the server rejects the token.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    Message, WebSocket,
};

const TOKEN: &str = "tail-regression-token";

/// Kills `den tail` even when an assertion fails, so no stray process survives.
struct Tail(Child);
impl Drop for Tail {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn spawn(port: u16) -> Tail {
    let child = Command::new(env!("CARGO_BIN_EXE_den"))
        .args([
            "--url",
            &format!("http://127.0.0.1:{port}"),
            "--token",
            TOKEN,
            "--config",
            &std::env::temp_dir()
                .join(format!("den-tail-test-{}.json", std::process::id()))
                .to_string_lossy(),
            "tail",
        ])
        .env_remove("DEN_URL")
        .env_remove("DEN_TOKEN")
        .env_remove("DEN_CONFIG")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run den tail");
    Tail(child)
}

/// Completes the handshake, returning the socket and the credentials it carried.
fn accept(listener: &TcpListener) -> (WebSocket<TcpStream>, Option<String>) {
    let (stream, _) = listener.accept().expect("accept");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut seen = None;
    let socket = accept_hdr(stream, |request: &Request, response: Response| {
        seen = request
            .headers()
            .get("authorization")
            .map(|v| v.to_str().unwrap_or_default().to_string());
        Ok(response)
    })
    .map_err(|e| e.to_string())
    .expect("handshake");
    (socket, seen)
}

fn message(id: u8) -> String {
    format!(
        r#"{{"type":"message_created","id":"{id:026}","channel_id":"00000000000000000000000100","author_id":"00000000000000000000000300","content":"event {id}","reply_to":null,"created_at":"2026-09-14T00:00:00Z","edited_at":null,"attachments":[],"objects":[],"reactions":[],"mention_ids":[]}}"#
    )
}

#[test]
fn tail_reauthenticates_on_reconnect_and_stops_when_rejected() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let mut tail = spawn(port);

    // First connection: a gap notice and one event, then the socket drops.
    let (mut socket, first) = accept(&listener);
    socket
        .send(Message::text(
            r#"{"type":"resync","reason":"connected; refresh history"}"#,
        ))
        .unwrap();
    socket.send(Message::text(message(1))).unwrap();
    socket.flush().unwrap();
    drop(socket);

    // Second connection: the reconnect must authenticate again, then close cleanly.
    let (mut socket, second) = accept(&listener);
    socket.send(Message::text(message(2))).unwrap();
    socket.close(None).unwrap();
    let _ = socket.flush();
    drop(socket);

    // Third connection: the server rejects the token, so tail must give up.
    let (mut stream, _) = listener.accept().expect("accept");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    let third = String::from_utf8_lossy(&head).to_lowercase();
    stream
        .write_all(b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
        .unwrap();
    let _ = stream.flush();
    drop(stream);

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = tail.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "tail kept reconnecting after a 401 instead of exiting"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    tail.0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    tail.0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();

    let bearer = Some(format!("Bearer {TOKEN}"));
    assert_eq!(first, bearer, "first connection credentials");
    assert_eq!(second, bearer, "reconnect must send the token again");
    assert!(
        third.contains(&format!("authorization: bearer {TOKEN}")),
        "every connection must authenticate, got head: {third}"
    );
    assert!(!status.success(), "a rejected token must fail the command");

    let events: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout carries JSON lines only"))
        .collect();
    let ids: Vec<&str> = events
        .iter()
        .filter(|e| e["type"] == "message_created")
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["00000000000000000000000001", "00000000000000000000000002"],
        "events from both connections, once each"
    );
    assert_eq!(
        events.iter().filter(|e| e["type"] == "resync").count(),
        1,
        "resync is forwarded, never invented"
    );
    assert!(
        stderr.contains("Gap:") && stderr.contains("Connection lost"),
        "gaps and reconnects are reported on stderr: {stderr}"
    );
}
