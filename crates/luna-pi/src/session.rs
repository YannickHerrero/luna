use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use tokio::{fs::File, io::BufReader, sync::Mutex};
use uuid::Uuid;

use crate::{
    BridgeError, PiBridge, PiError, PiProcess, PiProcessConfig, RpcDelivery, RpcImage, RpcResponse,
    read_jsonl_record,
};

#[derive(Debug, Clone)]
pub struct SessionRuntimeConfig {
    pub pi_executable: PathBuf,
    pub bridge_extension: PathBuf,
    pub session_directory: PathBuf,
    pub bridge_directory: PathBuf,
    pub request_timeout: Duration,
}

pub struct ManagedSession {
    pub conversation_id: Uuid,
    pub process: PiProcess,
    pub bridge: PiBridge,
    session_path: Option<PathBuf>,
    dispatch_lock: Mutex<()>,
}

impl ManagedSession {
    pub async fn send(
        &self,
        dispatch_id: Uuid,
        message: &str,
        images: &[RpcImage],
        delivery: RpcDelivery,
    ) -> Result<RpcResponse, SessionError> {
        let _dispatch = self.dispatch_lock.lock().await;
        let bridge_events = self.bridge.prepare_dispatch(dispatch_id).await?;
        let response = self.process.prompt(message, images, delivery).await;
        match response {
            Ok(response) => {
                self.bridge
                    .wait_until_recorded(bridge_events, dispatch_id)
                    .await?;
                Ok(response)
            }
            Err(error) => {
                self.bridge.cancel_dispatch(dispatch_id).await;
                Err(error.into())
            }
        }
    }

    pub async fn has_dispatch_marker(&self, dispatch_id: Uuid) -> Result<bool, SessionError> {
        let Some(session_path) = &self.session_path else {
            return Ok(false);
        };
        Ok(session_contains_dispatch_marker(session_path, dispatch_id).await?)
    }

    pub async fn abort(&self) -> Result<RpcResponse, SessionError> {
        Ok(self.process.abort().await?)
    }

    pub async fn shutdown(&self) {
        self.process.shutdown().await;
        self.bridge.shutdown().await;
    }
}

pub struct SessionSupervisor {
    config: SessionRuntimeConfig,
    sessions: Mutex<HashMap<Uuid, Arc<ManagedSession>>>,
}

impl SessionSupervisor {
    #[must_use]
    pub fn new(config: SessionRuntimeConfig) -> Self {
        Self {
            config,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub async fn active(&self, conversation_id: Uuid) -> Option<Arc<ManagedSession>> {
        self.sessions
            .lock()
            .await
            .get(&conversation_id)
            .filter(|session| {
                matches!(
                    *session.process.status().borrow(),
                    crate::ProcessStatus::Running
                )
            })
            .cloned()
    }

    pub async fn activate(
        &self,
        conversation_id: Uuid,
        working_directory: PathBuf,
        session_path: Option<PathBuf>,
    ) -> Result<Arc<ManagedSession>, SessionError> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&conversation_id)
            && matches!(
                *session.process.status().borrow(),
                crate::ProcessStatus::Running
            )
        {
            return Ok(session.clone());
        }
        if let Some(stale) = sessions.remove(&conversation_id) {
            stale.shutdown().await;
        }
        let bridge_path = self
            .config
            .bridge_directory
            .join(format!("{conversation_id}.sock"));
        let bridge = PiBridge::bind(&bridge_path, self.config.request_timeout).await?;
        let mut environment = HashMap::new();
        environment.insert(
            "LUNA_BRIDGE_SOCKET".into(),
            bridge.path().to_string_lossy().into_owned(),
        );
        environment.insert(
            "LUNA_WORKING_DIRECTORY".into(),
            working_directory.to_string_lossy().into_owned(),
        );
        let process = match PiProcess::spawn(PiProcessConfig {
            executable: self.config.pi_executable.clone(),
            working_directory,
            session_directory: self.config.session_directory.clone(),
            session_path: session_path.clone(),
            extension_path: Some(self.config.bridge_extension.clone()),
            environment,
            request_timeout: self.config.request_timeout,
        })
        .await
        {
            Ok(process) => process,
            Err(error) => {
                bridge.shutdown().await;
                return Err(error.into());
            }
        };
        if let Err(error) = bridge.wait_until_ready().await {
            process.shutdown().await;
            bridge.shutdown().await;
            return Err(error.into());
        }
        let session = Arc::new(ManagedSession {
            conversation_id,
            process,
            bridge,
            session_path,
            dispatch_lock: Mutex::new(()),
        });
        sessions.insert(conversation_id, session.clone());
        Ok(session)
    }

    pub async fn deactivate(&self, conversation_id: Uuid) {
        if let Some(session) = self.sessions.lock().await.remove(&conversation_id) {
            session.shutdown().await;
        }
    }

    pub async fn shutdown(&self) {
        let sessions = self
            .sessions
            .lock()
            .await
            .drain()
            .map(|(_, session)| session)
            .collect::<Vec<_>>();
        for session in sessions {
            session.shutdown().await;
        }
    }
}

async fn session_contains_dispatch_marker(
    session_path: &std::path::Path,
    dispatch_id: Uuid,
) -> Result<bool, PiError> {
    let file = File::open(session_path).await?;
    let mut reader = BufReader::new(file);
    let dispatch_id = dispatch_id.to_string();
    while let Some(entry) = read_jsonl_record(&mut reader).await? {
        if entry.get("type").and_then(serde_json::Value::as_str) == Some("custom")
            && entry.get("customType").and_then(serde_json::Value::as_str) == Some("luna-dispatch")
            && entry
                .get("data")
                .and_then(|data| data.get("dispatchId"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == dispatch_id)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Pi(#[from] PiError),
    #[error(transparent)]
    Bridge(#[from] BridgeError),
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn scans_dispatch_markers_after_more_than_one_rpc_frame_of_history() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session_path = directory.path().join("large-session.jsonl");
        let mut session = std::fs::File::create(&session_path).expect("session file");
        let payload = "x".repeat(1024 * 1024);
        for sequence in 0..17 {
            writeln!(
                session,
                "{}",
                json!({
                    "type": "custom",
                    "id": format!("history-{sequence}"),
                    "customType": "history",
                    "data": { "payload": &payload }
                })
            )
            .expect("history entry");
        }
        let dispatch_id = Uuid::new_v4();
        writeln!(
            session,
            "{}",
            json!({
                "type": "custom",
                "id": "dispatch-marker",
                "customType": "luna-dispatch",
                "data": { "dispatchId": dispatch_id }
            })
        )
        .expect("dispatch marker");
        session.flush().expect("flush session");

        assert!(
            std::fs::metadata(&session_path)
                .expect("session metadata")
                .len()
                > 16 * 1024 * 1024
        );
        assert!(
            session_contains_dispatch_marker(&session_path, dispatch_id)
                .await
                .expect("marker scan")
        );
        assert!(
            !session_contains_dispatch_marker(&session_path, Uuid::new_v4())
                .await
                .expect("missing marker scan")
        );
    }
}
