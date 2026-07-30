use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    BridgeError, PiBridge, PiError, PiProcess, PiProcessConfig, RpcDelivery, RpcImage, RpcResponse,
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

    pub async fn activate(
        &self,
        conversation_id: Uuid,
        working_directory: PathBuf,
        session_path: Option<PathBuf>,
    ) -> Result<Arc<ManagedSession>, SessionError> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&conversation_id) {
            return Ok(session.clone());
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
            session_path,
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

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Pi(#[from] PiError),
    #[error(transparent)]
    Bridge(#[from] BridgeError),
}
