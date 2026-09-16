use crate::{
    client::{print, Client},
    id,
};
use den_core::*;
use reqwest::Method;
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::Duration,
};
use tungstenite::{client::IntoClientRequest, Message as Frame};

fn is_auth_rejection(status: u16) -> bool {
    matches!(status, 401 | 403)
}

fn passes_filter(event: &Event, channel: &Option<String>) -> bool {
    let event_channel = match event {
        Event::MessageCreated(m) | Event::MessageEdited(m) => Some(&m.channel_id),
        Event::MessageDeleted { channel_id, .. }
        | Event::ObjectPatched { channel_id, .. }
        | Event::ObjectPresence { channel_id, .. }
        | Event::Typing { channel_id, .. } => Some(channel_id),
        _ => None,
    };
    channel.is_none() || event_channel.is_none() || event_channel == channel.as_ref()
}

fn ws_request(url: &str, token: &str) -> anyhow::Result<tungstenite::http::Request<()>> {
    let mut request = url.into_client_request()?;
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse()?);
    Ok(request)
}

pub fn tail(c: &Client, channel: Option<String>) -> anyhow::Result<()> {
    let token = c
        .token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Login or set DEN_TOKEN first"))?;
    if let Some(channel) = &channel {
        let _: Channel = c.get(&format!("/channels/{}", id(channel)?))?;
    }
    let url = format!("{}/ws", c.url.replacen("http", "ws", 1));
    let mut delay = 1;
    loop {
        let request = ws_request(&url, token)?;
        match tungstenite::connect(request) {
            Ok((mut socket, _)) => {
                // Detect a half-open connection even when no chat events arrive.
                match socket.get_mut() {
                    tungstenite::stream::MaybeTlsStream::Plain(tcp) => {
                        tcp.set_read_timeout(Some(Duration::from_secs(50)))?;
                        tcp.set_write_timeout(Some(Duration::from_secs(10)))?;
                    }
                    tungstenite::stream::MaybeTlsStream::Rustls(tls) => {
                        tls.sock.set_read_timeout(Some(Duration::from_secs(50)))?;
                        tls.sock.set_write_timeout(Some(Duration::from_secs(10)))?;
                    }
                    _ => {}
                }
                delay = 1;
                loop {
                    match socket.read() {
                        Ok(Frame::Text(text)) => {
                            let event: Event = serde_json::from_str(&text)?;
                            if let Event::Resync { reason } = &event {
                                eprintln!("Gap: {reason}. Run den read to refresh; events are not replayed.");
                            }
                            if passes_filter(&event, &channel) {
                                print(&event)?;
                            }
                        }
                        Ok(Frame::Ping(bytes)) => {
                            if socket.send(Frame::Pong(bytes)).is_err() {
                                break;
                            }
                        }
                        Ok(Frame::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
            }
            Err(tungstenite::Error::Http(response))
                if is_auth_rejection(response.status().as_u16()) =>
            {
                anyhow::bail!("Authentication rejected; log in again")
            }
            Err(_) => {}
        }
        eprintln!("Connection lost; events may be missing. Reconnecting in {delay}s.");
        std::thread::sleep(Duration::from_secs(delay));
        delay = (delay * 2).min(30);
    }
}
pub fn upload(
    c: &Client,
    channel: &str,
    path: &Path,
    resume: Option<String>,
    content_type: Option<String>,
) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let size = i64::try_from(file.metadata()?.len())?;
    let filename = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
        .to_string();
    let mime = content_type.unwrap_or_else(|| {
        match path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "mp4" | "m4v" => "video/mp4",
            "mov" => "video/quicktime",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "m4a" => "audio/mp4",
            "ogg" => "audio/ogg",
            "wav" => "audio/wav",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        }
        .into()
    });
    let mut upload: Upload = if let Some(resume) = resume {
        c.get(&format!("/uploads/{}", id(&resume)?))?
    } else {
        c.send(
            Method::POST,
            "/uploads",
            &BeginUpload {
                channel_id: channel.into(),
                filename: filename.clone(),
                content_type: mime,
                size,
            },
        )?
    };
    anyhow::ensure!(
        upload.channel_id == channel && upload.size == size && upload.filename == filename,
        "Resume metadata does not match this file/channel; use the original unchanged file"
    );
    eprintln!(
        "Upload {}: resume with den upload {} {:?} --resume {}",
        upload.id, channel, path, upload.id
    );
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    while upload.offset < size {
        file.seek(SeekFrom::Start(upload.offset as u64))?;
        let count = file.read(&mut buffer)?;
        anyhow::ensure!(count > 0, "File changed during upload");
        upload = Client::decode(
            c.request(Method::PATCH, &format!("/uploads/{}", upload.id))
                .header("Upload-Offset", upload.offset)
                .header("Content-Type", "application/octet-stream")
                .body(buffer[..count].to_vec())
                .send()?,
        )?;
        eprintln!("{} / {} bytes", upload.offset, size);
    }
    let result: Upload = c.send(
        Method::POST,
        &format!("/uploads/{}/complete", upload.id),
        &serde_json::json!({}),
    )?;
    print(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_message(channel_id: &str, id: &str) -> Event {
        Event::MessageCreated(Message {
            id: id.to_string(),
            channel_id: channel_id.to_string(),
            author_id: "00000000000000000000000300".to_string(),
            content: "event".to_string(),
            reply_to: None,
            created_at: "2026-09-14T00:00:00Z".to_string(),
            edited_at: None,
            attachments: vec![],
            objects: vec![],
            reactions: vec![],
            mention_ids: vec![],
        })
    }

    #[test]
    fn tail_stops_on_401_and_403_but_retries_transient() {
        assert!(is_auth_rejection(401));
        assert!(is_auth_rejection(403));
        assert!(!is_auth_rejection(503));
        assert!(!is_auth_rejection(500));
    }

    #[test]
    fn tail_authenticates_every_connection() {
        let url = "ws://127.0.0.1:1/ws";
        for _ in 0..2 {
            let request = ws_request(url, "synthetic-arena-token").unwrap();
            assert_eq!(
                request.headers().get("Authorization").unwrap(),
                "Bearer synthetic-arena-token"
            );
        }
    }

    #[test]
    fn tail_preserves_filtering_and_global_events() {
        let kept = "00000000000000000000000100".to_string();
        let other = "00000000000000000000000200".to_string();
        let filter = Some(kept.clone());
        assert!(passes_filter(
            &test_message(&kept, "00000000000000000000000001"),
            &filter
        ));
        assert!(!passes_filter(
            &test_message(&other, "00000000000000000000000090"),
            &filter
        ));
        assert!(passes_filter(
            &Event::Resync {
                reason: "connected; refresh history".to_string()
            },
            &filter
        ));
        assert!(passes_filter(
            &Event::Presence {
                user_id: "00000000000000000000000300".to_string(),
                online: true
            },
            &filter
        ));
        assert!(passes_filter(
            &test_message(&other, "00000000000000000000000090"),
            &None
        ));
    }
}
