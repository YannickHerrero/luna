use futures_util::{SinkExt, StreamExt};
use luna_protocol::ServerEventEnvelope;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message},
};

use crate::api::{ApiClientError, LunaApi};

pub struct EventSocket {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl EventSocket {
    pub async fn connect(api: &LunaApi, after: i64) -> Result<Self, RealtimeError> {
        let request = api.events_request(after)?;
        let (socket, _) = connect_async(request).await?;
        Ok(Self { socket })
    }

    pub async fn next(&mut self) -> Result<Option<ServerEventEnvelope>, RealtimeError> {
        loop {
            let Some(frame) = self.socket.next().await else {
                return Ok(None);
            };
            match frame? {
                Message::Text(text) => {
                    return serde_json::from_str(&text)
                        .map(Some)
                        .map_err(RealtimeError::Decode);
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(&bytes)
                        .map(Some)
                        .map_err(RealtimeError::Decode);
                }
                Message::Ping(payload) => {
                    self.socket.send(Message::Pong(payload)).await?;
                }
                Message::Close(_) => return Ok(None),
                Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    pub async fn close(mut self) -> Result<(), RealtimeError> {
        self.socket.close(None).await.map_err(RealtimeError::Socket)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RealtimeError {
    #[error(transparent)]
    Request(#[from] ApiClientError),
    #[error("the Luna event stream failed: {0}")]
    Socket(#[from] WebSocketError),
    #[error("Luna sent an invalid event: {0}")]
    Decode(serde_json::Error),
}
