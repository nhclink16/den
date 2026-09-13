use anyhow::{ensure, Result};
use den_core::{DirectCheck, DirectPermission, HostCredential, HostFrame, TerminalFrame};
use den_host::pty::Sessions;
use futures_util::{SinkExt, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc, Mutex},
};
use tokio_tungstenite::tungstenite::Message;

// Only the Tailscale interface is eligible, never a LAN or wildcard listener.
// Den validates every input and revalidates viewers twice per second. Host
// credentials stay in headers; the short-lived viewer token is the first frame.
pub async fn listen(
    cfg: HostCredential,
    sessions: Arc<Mutex<Sessions>>,
    output: broadcast::Sender<HostFrame>,
    commands: mpsc::Sender<HostFrame>,
) -> Result<String> {
    let result = tokio::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .await?;
    ensure!(result.status.success(), "Tailscale unavailable");
    let ip: std::net::Ipv4Addr = std::str::from_utf8(&result.stdout)?.trim().parse()?;
    let octets = ip.octets();
    ensure!(
        octets[0] == 100 && (64..=127).contains(&octets[1]),
        "Expected a Tailscale address"
    );
    let listener = TcpListener::bind((ip, 0)).await?;
    let url = format!("ws://{}", listener.local_addr()?);
    tokio::spawn(async move {
        let limit = Arc::new(tokio::sync::Semaphore::new(32));
        while let Ok((stream, _)) = listener.accept().await {
            let Ok(permit) = limit.clone().try_acquire_owned() else {
                continue;
            };
            let (cfg, sessions, commands) = (cfg.clone(), sessions.clone(), commands.clone());
            let mut rx = output.subscribe();
            tokio::spawn(async move {
                let _permit = permit;
                let run = async {
                    let origin = cfg.server_url.clone();
                    let mut ws=tokio::time::timeout(Duration::from_secs(3), tokio_tungstenite::accept_hdr_async(stream, move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                        if req.headers().get("origin").and_then(|h|h.to_str().ok()) != Some(origin.as_str()) {return Err(tokio_tungstenite::tungstenite::http::Response::builder().status(403).body(None).unwrap());} Ok(response)
                    })).await??;
                    let first = tokio::time::timeout(Duration::from_secs(2), ws.next())
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Closed"))??;
                    let check: DirectCheck = serde_json::from_slice(&first.into_data())?;
                    let http = reqwest::Client::builder()
                        .timeout(Duration::from_millis(450))
                        .build()?;
                    let validate = |input| {
                        http.post(format!("{}/hosts/direct/check", cfg.server_url))
                            .bearer_auth(&cfg.token)
                            .json(&DirectCheck {
                                token: check.token.clone(),
                                session_id: check.session_id.clone(),
                                input,
                            })
                            .send()
                    };
                    let response = validate(false).await?;
                    ensure!(response.status().is_success(), "Denied");
                    let permission: DirectPermission = response.json().await?;
                    if !permission.owner {
                        commands
                            .send(HostFrame::Viewer {
                                session_id: check.session_id.clone(),
                                user_id: permission.user_id,
                                name: permission.name,
                            })
                            .await?;
                    }
                    let history = sessions.lock().await.history(&check.session_id);
                    ws.send(Message::Binary(
                        serde_json::to_vec(&HostFrame::Output {
                            session_id: check.session_id.clone(),
                            bytes: history,
                        })?
                        .into(),
                    ))
                    .await?;
                    let mut tick = tokio::time::interval(Duration::from_millis(500));
                    loop {
                        tokio::select! {
                            _=tick.tick()=> {ensure!(validate(false).await?.status().is_success(),"Revoked or expired");}
                            frame=ws.next()=> {
                                let Some(Ok(Message::Binary(b)))=frame else {break;};
                                ensure!(b.len()<=64*1024,"Frame too large");
                                let frame:TerminalFrame=serde_json::from_slice(&b)?;
                                ensure!(validate(true).await?.status().is_success(),"View only");
                                let frame=match frame {
                                    TerminalFrame::TerminalInput{session_id,bytes} if session_id==check.session_id => HostFrame::Input{session_id,bytes},
                                    TerminalFrame::TerminalResize{session_id,cols,rows} if session_id==check.session_id => HostFrame::Resize{session_id,cols,rows},
                                    _=>break,
                                };
                                commands.send(frame).await?;
                            }
                            frame=rx.recv()=> {
                                let frame=frame?;
                                if matches!(&frame,HostFrame::Output{session_id,..}|HostFrame::Exited{session_id,..} if session_id==&check.session_id) {
                                    tokio::time::timeout(Duration::from_millis(450),ws.send(Message::Binary(serde_json::to_vec(&frame)?.into()))).await??;
                                }
                            }
                        }
                    }
                    Ok::<_, anyhow::Error>(())
                };
                let _ = run.await;
            });
        }
    });
    Ok(url)
}
