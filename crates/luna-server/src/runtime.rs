use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use luna_pi::{
    BridgeEvent, ManagedSession, NormalizedPiEvent, RpcDelivery, RpcImage, SessionRuntimeConfig,
    SessionSupervisor,
};
use luna_protocol::{
    ActivityPhase, AgentActivityChanged, MessageDelivery, ServerEvent, SessionState,
    SteeringQueueChanged, WorkspaceUpdated,
};
use luna_storage::{ConversationRuntimeRecord, Database};
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use crate::{auth::now, error::AppError, events::EventHub};

pub struct ConversationRuntime {
    supervisor: SessionSupervisor,
    database: Database,
    events: EventHub,
    pumps: Mutex<HashSet<Uuid>>,
    stopping: Mutex<HashSet<Uuid>>,
    shutting_down: AtomicBool,
}

impl ConversationRuntime {
    #[must_use]
    pub fn new(config: SessionRuntimeConfig, database: Database, events: EventHub) -> Self {
        Self {
            supervisor: SessionSupervisor::new(config),
            database,
            events,
            pumps: Mutex::new(HashSet::new()),
            stopping: Mutex::new(HashSet::new()),
            shutting_down: AtomicBool::new(false),
        }
    }

    pub async fn session(
        self: &Arc<Self>,
        conversation: ConversationRuntimeRecord,
    ) -> Result<Arc<ManagedSession>, AppError> {
        let conversation_id = conversation.conversation.id;
        let restoring = conversation.pi_session_path.is_some()
            && self.supervisor.active(conversation_id).await.is_none();
        if restoring {
            self.set_state(conversation_id, SessionState::Restoring)
                .await?;
        }
        let session = self
            .supervisor
            .activate(
                conversation_id,
                PathBuf::from(&conversation.conversation.active_working_directory),
                conversation.pi_session_path.map(PathBuf::from),
            )
            .await?;
        if let Some(data) = session.process.get_state().await?.data {
            let session_id = data.get("sessionId").and_then(serde_json::Value::as_str);
            let session_path = data.get("sessionFile").and_then(serde_json::Value::as_str);
            if let (Some(session_id), Some(session_path)) = (session_id, session_path) {
                self.database
                    .set_conversation_session(
                        conversation.conversation.id,
                        session_id,
                        session_path,
                        &now()?,
                    )
                    .await?;
            }
        }
        let should_mark_idle = restoring
            || matches!(
                conversation.conversation.state,
                SessionState::Creating | SessionState::Starting | SessionState::Restoring
            );
        let mut pumps = self.pumps.lock().await;
        if pumps.insert(conversation.conversation.id) {
            let runtime = self.clone();
            let session_for_pump = session.clone();
            tokio::spawn(async move {
                runtime.pump(session_for_pump).await;
            });
        }
        drop(pumps);
        if should_mark_idle {
            self.set_state(conversation.conversation.id, SessionState::Idle)
                .await?;
        }
        Ok(session)
    }

    pub async fn dispatch(
        self: Arc<Self>,
        conversation_id: Uuid,
        dispatch_id: Uuid,
        text: String,
        delivery: MessageDelivery,
    ) {
        let result = self
            .dispatch_inner(conversation_id, dispatch_id, &text, delivery)
            .await;
        if let Err(error) = result {
            warn!(%conversation_id, %dispatch_id, "Pi dispatch failed: {error}");
            let timestamp = now().unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
            let _ = self
                .database
                .set_dispatch_state(
                    dispatch_id,
                    "failed",
                    Some("pi_dispatch_failed"),
                    &timestamp,
                )
                .await;
            let _ = self.set_state(conversation_id, SessionState::Error).await;
        }
    }

    async fn dispatch_inner(
        self: &Arc<Self>,
        conversation_id: Uuid,
        dispatch_id: Uuid,
        text: &str,
        delivery: MessageDelivery,
    ) -> Result<(), AppError> {
        self.database
            .set_dispatch_state(dispatch_id, "running", None, &now()?)
            .await?;
        let conversation = self
            .database
            .conversation_runtime(conversation_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let session = self.session(conversation).await?;
        let rpc_delivery = match delivery {
            MessageDelivery::Initial => RpcDelivery::Normal,
            MessageDelivery::Steer => RpcDelivery::Steer,
        };
        session
            .send(dispatch_id, text, &Vec::<RpcImage>::new(), rpc_delivery)
            .await?;
        self.database
            .set_dispatch_state(dispatch_id, "dispatched", None, &now()?)
            .await?;
        Ok(())
    }

    pub async fn abort(self: &Arc<Self>, conversation_id: Uuid) -> Result<(), AppError> {
        let conversation = self
            .database
            .conversation_runtime(conversation_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let session = self.session(conversation).await?;
        session.abort().await?;
        self.set_state(conversation_id, SessionState::Interrupted)
            .await?;
        Ok(())
    }

    pub async fn deactivate(&self, conversation_id: Uuid) {
        self.stopping.lock().await.insert(conversation_id);
        self.supervisor.deactivate(conversation_id).await;
    }

    pub async fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        self.supervisor.shutdown().await;
    }

    async fn pump(self: Arc<Self>, session: Arc<ManagedSession>) {
        let conversation_id = session.conversation_id;
        let mut pi_events = session.process.subscribe();
        let mut bridge_events = session.bridge.subscribe();
        let mut status = session.process.status();
        let mut assistant_message: Option<Uuid> = None;
        let mut chunk_index = 0_i64;
        loop {
            tokio::select! {
                event = pi_events.recv() => {
                    match event {
                        Ok(event) => self.handle_pi_event(
                            conversation_id,
                            event.normalized,
                            &mut assistant_message,
                            &mut chunk_index,
                        ).await,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                            warn!(%conversation_id, count, "Pi event pump lagged");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                event = bridge_events.recv() => {
                    if let Ok(event) = event {
                        self.handle_bridge_event(conversation_id, event).await;
                    }
                }
                changed = status.changed() => {
                    if changed.is_err() || !matches!(*status.borrow(), luna_pi::ProcessStatus::Running) {
                        let intentional = self.shutting_down.load(Ordering::Acquire)
                            || self.stopping.lock().await.remove(&conversation_id);
                        if !intentional {
                            let _ = self.set_state(conversation_id, SessionState::Crashed).await;
                        }
                        break;
                    }
                }
            }
        }
        self.pumps.lock().await.remove(&conversation_id);
    }

    async fn handle_pi_event(
        &self,
        conversation_id: Uuid,
        event: NormalizedPiEvent,
        assistant_message: &mut Option<Uuid>,
        chunk_index: &mut i64,
    ) {
        let result: Result<(), AppError> = async {
            match event {
                NormalizedPiEvent::AgentStarted => {
                    self.set_state(conversation_id, SessionState::Working)
                        .await?;
                    self.events
                        .append(
                            Some(conversation_id),
                            Some(conversation_id),
                            &ServerEvent::AgentActivityChanged(AgentActivityChanged {
                                active: true,
                                phase: ActivityPhase::Thinking,
                            }),
                            &now()?,
                        )
                        .await?;
                }
                NormalizedPiEvent::TextDelta {
                    content_index,
                    delta,
                } => {
                    let message_id = match *assistant_message {
                        Some(id) => id,
                        None => {
                            let id = Uuid::now_v7();
                            let (_, event) = self
                                .database
                                .begin_assistant_message(conversation_id, id, &now()?)
                                .await?;
                            self.events.publish(event);
                            *assistant_message = Some(id);
                            *chunk_index = 0;
                            id
                        }
                    };
                    let event = self
                        .database
                        .append_message_delta(
                            conversation_id,
                            message_id,
                            *chunk_index,
                            i64::try_from(content_index).unwrap_or(i64::MAX),
                            &delta,
                            &now()?,
                        )
                        .await?;
                    *chunk_index += 1;
                    self.events.publish(event);
                }
                NormalizedPiEvent::AgentSettled => {
                    if let Some(message_id) = assistant_message.take() {
                        let event = self
                            .database
                            .complete_message(conversation_id, message_id, &now()?)
                            .await?;
                        self.events.publish(event);
                    }
                    self.events
                        .append(
                            Some(conversation_id),
                            Some(conversation_id),
                            &ServerEvent::AgentActivityChanged(AgentActivityChanged {
                                active: false,
                                phase: ActivityPhase::Thinking,
                            }),
                            &now()?,
                        )
                        .await?;
                    self.set_state(conversation_id, SessionState::Idle).await?;
                }
                NormalizedPiEvent::ToolStarted | NormalizedPiEvent::ToolEnded { .. } => {
                    self.events
                        .append(
                            Some(conversation_id),
                            Some(conversation_id),
                            &ServerEvent::AgentActivityChanged(AgentActivityChanged {
                                active: true,
                                phase: ActivityPhase::Working,
                            }),
                            &now()?,
                        )
                        .await?;
                }
                NormalizedPiEvent::QueueUpdated {
                    steering,
                    follow_up,
                } => {
                    self.events
                        .append(
                            Some(conversation_id),
                            Some(conversation_id),
                            &ServerEvent::SteeringQueueChanged(SteeringQueueChanged {
                                pending: i64::try_from(steering + follow_up).unwrap_or(i64::MAX),
                                delivery: MessageDelivery::Steer,
                            }),
                            &now()?,
                        )
                        .await?;
                }
                NormalizedPiEvent::CompactionStarted => {
                    self.set_state(conversation_id, SessionState::Compacting)
                        .await?;
                }
                NormalizedPiEvent::RetryStarted => {
                    self.set_state(conversation_id, SessionState::Retrying)
                        .await?;
                }
                _ => {}
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            warn!(%conversation_id, "Unable to persist Pi event: {error}");
        }
    }

    async fn handle_bridge_event(&self, conversation_id: Uuid, event: BridgeEvent) {
        if event.event_type != "workspace" {
            return;
        }
        let Some(cwd) = event.cwd else { return };
        let result: Result<(), AppError> = async {
            let timestamp = now()?;
            self.database
                .set_working_directory(conversation_id, &cwd.to_string_lossy(), &timestamp)
                .await?;
            self.events
                .append(
                    Some(conversation_id),
                    Some(conversation_id),
                    &ServerEvent::WorkspaceUpdated(WorkspaceUpdated {
                        working_directory: cwd.to_string_lossy().into_owned(),
                    }),
                    &timestamp,
                )
                .await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            warn!(%conversation_id, "Unable to persist Pi workspace: {error}");
        }
    }

    async fn set_state(&self, conversation_id: Uuid, state: SessionState) -> Result<(), AppError> {
        let timestamp = now()?;
        self.database
            .set_conversation_state(conversation_id, state, &timestamp)
            .await?;
        self.events
            .append(
                Some(conversation_id),
                Some(conversation_id),
                &ServerEvent::SessionStateChanged { state },
                &timestamp,
            )
            .await?;
        Ok(())
    }
}
