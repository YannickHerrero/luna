use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use luna_protocol::AgentTaskList;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::{broadcast, mpsc, watch},
    time::timeout,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum BridgeCommand {
    Dispatch { dispatch_id: Uuid },
    CancelDispatch { dispatch_id: Uuid },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub dispatch_id: Option<Uuid>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub task_list: Option<AgentTaskList>,
    #[serde(flatten)]
    pub details: serde_json::Map<String, Value>,
}

pub struct PiBridge {
    path: PathBuf,
    commands: mpsc::Sender<BridgeCommand>,
    events: broadcast::Sender<BridgeEvent>,
    ready: watch::Receiver<bool>,
    shutdown: mpsc::Sender<()>,
    timeout: Duration,
}

impl PiBridge {
    pub async fn bind(
        path: impl AsRef<Path>,
        operation_timeout: Duration,
    ) -> Result<Self, BridgeError> {
        let path = path.as_ref().to_owned();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).await?;
            }
        }
        if tokio::fs::try_exists(&path).await? {
            tokio::fs::remove_file(&path).await?;
        }
        let listener = UnixListener::bind(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
        }
        let (commands, command_receiver) = mpsc::channel(128);
        let (events, _) = broadcast::channel(1_024);
        let (ready_sender, ready) = watch::channel(false);
        let (shutdown, shutdown_receiver) = mpsc::channel(1);
        tokio::spawn(run_bridge(
            listener,
            path.clone(),
            command_receiver,
            events.clone(),
            ready_sender,
            shutdown_receiver,
        ));
        Ok(Self {
            path,
            commands,
            events,
            ready,
            shutdown,
            timeout: operation_timeout,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<BridgeEvent> {
        self.events.subscribe()
    }

    pub async fn wait_until_ready(&self) -> Result<(), BridgeError> {
        let mut ready = self.ready.clone();
        timeout(self.timeout, async move {
            while !*ready.borrow() {
                ready.changed().await.map_err(|_| BridgeError::Closed)?;
            }
            Ok(())
        })
        .await
        .map_err(|_| BridgeError::Timeout)?
    }

    pub async fn prepare_dispatch(
        &self,
        dispatch_id: Uuid,
    ) -> Result<broadcast::Receiver<BridgeEvent>, BridgeError> {
        self.wait_until_ready().await?;
        let mut receiver = self.subscribe();
        self.commands
            .send(BridgeCommand::Dispatch { dispatch_id })
            .await
            .map_err(|_| BridgeError::Closed)?;
        self.wait_for_dispatch(&mut receiver, "dispatch_ready", dispatch_id)
            .await?;
        Ok(receiver)
    }

    pub async fn cancel_dispatch(&self, dispatch_id: Uuid) {
        let _ = self
            .commands
            .send(BridgeCommand::CancelDispatch { dispatch_id })
            .await;
    }

    pub async fn wait_until_recorded(
        &self,
        receiver: broadcast::Receiver<BridgeEvent>,
        dispatch_id: Uuid,
    ) -> Result<(), BridgeError> {
        let mut receiver = receiver;
        self.wait_for_dispatch(&mut receiver, "dispatch_recorded", dispatch_id)
            .await
    }

    pub async fn shutdown(&self) {
        let _ = self.shutdown.send(()).await;
    }

    async fn wait_for_dispatch(
        &self,
        receiver: &mut broadcast::Receiver<BridgeEvent>,
        event_type: &str,
        dispatch_id: Uuid,
    ) -> Result<(), BridgeError> {
        timeout(self.timeout, async {
            loop {
                let event = receiver.recv().await.map_err(|_| BridgeError::Closed)?;
                if event.event_type == event_type && event.dispatch_id == Some(dispatch_id) {
                    return Ok(());
                }
            }
        })
        .await
        .map_err(|_| BridgeError::Timeout)?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Pi bridge I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Pi bridge connection closed")]
    Closed,
    #[error("Pi bridge operation timed out")]
    Timeout,
    #[error("Pi bridge received invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

async fn run_bridge(
    listener: UnixListener,
    path: PathBuf,
    mut commands: mpsc::Receiver<BridgeCommand>,
    events: broadcast::Sender<BridgeEvent>,
    ready: watch::Sender<bool>,
    mut shutdown: mpsc::Receiver<()>,
) {
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            _ = shutdown.recv() => break,
        };
        let Ok((stream, _)) = accepted else { break };
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        loop {
            tokio::select! {
                line = lines.next_line() => {
                    let Ok(Some(line)) = line else { break };
                    match serde_json::from_str::<BridgeEvent>(&line) {
                        Ok(event) => {
                            if event.event_type == "ready" { ready.send_replace(true); }
                            let _ = events.send(event);
                        }
                        Err(error) => tracing::warn!("Invalid Pi bridge event: {error}"),
                    }
                }
                command = commands.recv() => {
                    let Some(command) = command else { break };
                    let Ok(mut line) = serde_json::to_vec(&command) else { continue };
                    line.push(b'\n');
                    if writer.write_all(&line).await.is_err() || writer.flush().await.is_err() { break; }
                }
                _ = shutdown.recv() => {
                    let _ = tokio::fs::remove_file(&path).await;
                    return;
                }
            }
        }
        ready.send_replace(false);
    }
    let _ = tokio::fs::remove_file(path).await;
}
