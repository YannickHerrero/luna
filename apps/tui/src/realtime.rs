use futures_util::{SinkExt, StreamExt};
use luna_protocol::ServerEventEnvelope;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message},
};

use crate::api::{ApiClientError, LunaApi};

#[derive(Debug)]
pub enum RealtimeUpdate {
    Connecting,
    Connected,
    Disconnected(String),
    Event(Box<ServerEventEnvelope>),
}

pub struct RealtimeHandle {
    cursor: tokio::sync::watch::Sender<i64>,
    shutdown: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl RealtimeHandle {
    pub fn set_cursor(&self, cursor: i64) {
        self.cursor.send_replace(cursor.max(0));
    }

    pub async fn shutdown(self) {
        self.shutdown.send_replace(true);
        let _ = self.task.await;
    }
}

pub fn spawn_realtime(
    api: LunaApi,
    initial_cursor: i64,
) -> (RealtimeHandle, tokio::sync::mpsc::Receiver<RealtimeUpdate>) {
    let (updates_tx, updates_rx) = tokio::sync::mpsc::channel(256);
    let (cursor_tx, cursor_rx) = tokio::sync::watch::channel(initial_cursor.max(0));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(reconnect_loop(api, updates_tx, cursor_rx, shutdown_rx));
    (
        RealtimeHandle {
            cursor: cursor_tx,
            shutdown: shutdown_tx,
            task,
        },
        updates_rx,
    )
}

async fn reconnect_loop(
    api: LunaApi,
    updates: tokio::sync::mpsc::Sender<RealtimeUpdate>,
    mut cursor: tokio::sync::watch::Receiver<i64>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut delay = 1_u64;
    let mut after = *cursor.borrow_and_update();
    loop {
        if *shutdown.borrow() {
            return;
        }
        if updates.send(RealtimeUpdate::Connecting).await.is_err() {
            return;
        }
        after = after.max(*cursor.borrow());
        match EventSocket::connect(&api, after).await {
            Ok(mut socket) => {
                delay = 1;
                if updates.send(RealtimeUpdate::Connected).await.is_err() {
                    return;
                }
                loop {
                    tokio::select! {
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() {
                                let _ = socket.close().await;
                                return;
                            }
                        }
                        changed = cursor.changed() => {
                            if changed.is_err() {
                                let _ = socket.close().await;
                                return;
                            }
                            after = after.max(*cursor.borrow_and_update());
                        }
                        event = socket.next() => {
                            match event {
                                Ok(Some(event)) => {
                                    if let Some(event_id) = event.event_id {
                                        after = after.max(event_id);
                                    }
                                    let reset = matches!(
                                        event.event,
                                        luna_protocol::ServerEvent::SyncResetRequired { .. }
                                    );
                                    if updates.send(RealtimeUpdate::Event(Box::new(event))).await.is_err() {
                                        return;
                                    }
                                    if reset {
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(error) => {
                                    if updates
                                        .send(RealtimeUpdate::Disconnected(error.to_string()))
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                if updates
                    .send(RealtimeUpdate::Disconnected(error.to_string()))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
        delay = (delay * 2).min(15);
    }
}

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
