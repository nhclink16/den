use anyhow::{bail, Result};
use den_core::HostFrame;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};

/// The future direct WebRTC data channel implements this same frame transport.
#[allow(async_fn_in_trait)]
pub trait HostTransport {
    async fn send(&mut self, frame: &HostFrame) -> Result<()>;
    async fn receive(&mut self) -> Result<Option<HostFrame>>;
}
pub struct WebSocketTransport(WebSocketStream<MaybeTlsStream<TcpStream>>);
impl WebSocketTransport {
    pub async fn connect(url: &str, token: &str) -> Result<Self> {
        let mut req = url.into_client_request()?;
        req.headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse()?);
        let (socket, _) = tokio_tungstenite::connect_async(req).await?;
        Ok(Self(socket))
    }
}
impl HostTransport for WebSocketTransport {
    async fn send(&mut self, frame: &HostFrame) -> Result<()> {
        self.0
            .send(Message::Binary(serde_json::to_vec(frame)?.into()))
            .await?;
        Ok(())
    }
    async fn receive(&mut self) -> Result<Option<HostFrame>> {
        loop {
            match self.0.next().await {
                Some(Ok(Message::Binary(b))) => return Ok(Some(serde_json::from_slice(&b)?)),
                Some(Ok(Message::Ping(_))) => self.0.flush().await?,
                Some(Ok(Message::Pong(_))) => (),
                Some(Ok(Message::Close(_))) | None => return Ok(None),
                Some(Err(e)) => return Err(e.into()),
                _ => bail!("Expected a binary host frame"),
            }
        }
    }
}
