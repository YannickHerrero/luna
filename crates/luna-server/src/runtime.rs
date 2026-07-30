use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use base64::{Engine, engine::general_purpose::STANDARD};
use luna_pi::{
    BridgeEvent, ManagedSession, NormalizedPiEvent, RpcDelivery, RpcImage, SessionRuntimeConfig,
    SessionSupervisor,
};
use luna_protocol::{
    ActivityPhase, AgentActivityChanged, ConversationTitleUpdated, MessageDelivery,
    RepositoriesUpdated, ServerEvent, SessionState, SteeringQueueChanged, WorkspaceUpdated,
};
use luna_storage::{ConversationRuntimeRecord, Database, RepositoryObservation};
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use crate::{auth::now, error::AppError, events::EventHub};

pub struct ConversationRuntime {
    supervisor: SessionSupervisor,
    database: Database,
    events: EventHub,
    attachment_directory: PathBuf,
    pumps: Mutex<HashSet<Uuid>>,
    stopping: Mutex<HashSet<Uuid>>,
    shutting_down: AtomicBool,
}

impl ConversationRuntime {
    #[must_use]
    pub fn new(
        config: SessionRuntimeConfig,
        database: Database,
        events: EventHub,
        attachment_directory: PathBuf,
    ) -> Self {
        Self {
            supervisor: SessionSupervisor::new(config),
            database,
            events,
            attachment_directory,
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
        attachment_ids: Vec<Uuid>,
        delivery: MessageDelivery,
    ) {
        let result = self
            .dispatch_inner(
                conversation_id,
                dispatch_id,
                &text,
                &attachment_ids,
                delivery,
            )
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
        attachment_ids: &[Uuid],
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
        let reconcile_marker = conversation.pi_session_path.is_some()
            && self.supervisor.active(conversation_id).await.is_none();
        let session = self.session(conversation).await?;
        if reconcile_marker && session.has_dispatch_marker(dispatch_id).await? {
            self.database
                .set_dispatch_state(dispatch_id, "dispatched", None, &now()?)
                .await?;
            return Ok(());
        }
        let rpc_delivery = match delivery {
            MessageDelivery::Initial => RpcDelivery::Normal,
            MessageDelivery::Steer => RpcDelivery::Steer,
        };
        let mut images = Vec::with_capacity(attachment_ids.len());
        for attachment_id in attachment_ids {
            let stored = self
                .database
                .stored_attachment(*attachment_id)
                .await?
                .ok_or(AppError::NotFound)?;
            let bytes =
                tokio::fs::read(self.attachment_directory.join(&stored.storage_key)).await?;
            images.push(RpcImage::new(
                STANDARD.encode(bytes),
                stored.attachment.mime_type,
            ));
        }
        session
            .send(dispatch_id, text, &images, rpc_delivery)
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
                    self.generate_initial_title(conversation_id).await?;
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
        let result: Result<(), AppError> = async {
            match event.event_type.as_str() {
                "workspace" => {
                    let Some(cwd) = event.cwd else { return Ok(()) };
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
                    self.observe_repository(conversation_id, &cwd, true).await?;
                }
                "path_observed" => {
                    if let Some(path) = event.path {
                        self.observe_repository(conversation_id, &path, false)
                            .await?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            warn!(%conversation_id, "Unable to persist Pi workspace observation: {error}");
        }
    }

    async fn observe_repository(
        &self,
        conversation_id: Uuid,
        path: &std::path::Path,
        active: bool,
    ) -> Result<(), AppError> {
        let Some(root) = find_repository_root(path).await else {
            return Ok(());
        };
        let git_directory = git_output(&root, &["rev-parse", "--absolute-git-dir"])
            .await
            .unwrap_or_else(|| root.join(".git").to_string_lossy().into_owned());
        let branch = git_output(&root, &["symbolic-ref", "--short", "-q", "HEAD"]).await;
        let display_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Repository")
            .to_owned();
        let timestamp = now()?;
        let observation = self
            .database
            .observe_repository(RepositoryObservation {
                conversation_id,
                canonical_root: &root.to_string_lossy(),
                git_directory: &git_directory,
                display_name: &display_name,
                branch: branch.as_deref(),
                active,
                observed_at: &timestamp,
            })
            .await?;
        if observation.changed {
            self.events
                .append(
                    Some(conversation_id),
                    Some(conversation_id),
                    &ServerEvent::RepositoriesUpdated(RepositoriesUpdated {
                        repositories: observation.repositories,
                    }),
                    &timestamp,
                )
                .await?;
        }
        Ok(())
    }

    async fn generate_initial_title(&self, conversation_id: Uuid) -> Result<(), AppError> {
        let messages = self.database.messages(conversation_id, None, 100).await?;
        let Some(text) = messages
            .iter()
            .find(|message| message.role == luna_protocol::MessageRole::User)
            .map(|message| message.text.as_str())
        else {
            return Ok(());
        };
        let Some(title) = title_from_text(text) else {
            return Ok(());
        };
        let timestamp = now()?;
        let Some(conversation) = self
            .database
            .set_initial_automatic_title(conversation_id, &title, &timestamp)
            .await?
        else {
            return Ok(());
        };
        self.events
            .append(
                Some(conversation_id),
                Some(conversation_id),
                &ServerEvent::ConversationTitleUpdated(ConversationTitleUpdated {
                    title,
                    automatic: true,
                }),
                &timestamp,
            )
            .await?;
        self.events
            .append(
                Some(conversation_id),
                Some(conversation_id),
                &ServerEvent::ConversationUpserted(conversation),
                &timestamp,
            )
            .await?;
        Ok(())
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

fn title_from_text(text: &str) -> Option<String> {
    let compact = text
        .replace(['#', '*', '`', '_'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut candidate = compact.trim();
    for prefix in [
        "please ",
        "can you ",
        "could you ",
        "would you ",
        "help me ",
        "i need you to ",
    ] {
        if candidate.to_ascii_lowercase().starts_with(prefix) {
            candidate = candidate.get(prefix.len()..).unwrap_or(candidate).trim();
            break;
        }
    }
    candidate = candidate
        .split(['\n', '.', '!', '?'])
        .next()
        .unwrap_or(candidate)
        .trim();
    if candidate.is_empty() {
        return None;
    }
    let mut title = String::new();
    for word in candidate.split_whitespace() {
        if !title.is_empty() && title.len() + word.len() + 1 > 56 {
            break;
        }
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(word);
    }
    let mut characters = title.chars();
    let first = characters.next()?;
    Some(first.to_uppercase().collect::<String>() + characters.as_str())
}

async fn find_repository_root(path: &std::path::Path) -> Option<PathBuf> {
    let mut current = path.to_owned();
    while tokio::fs::metadata(&current).await.is_err() {
        current = current.parent()?.to_owned();
    }
    if tokio::fs::metadata(&current).await.ok()?.is_file() {
        current = current.parent()?.to_owned();
    }
    loop {
        if tokio::fs::symlink_metadata(current.join(".git"))
            .await
            .is_ok()
        {
            return tokio::fs::canonicalize(&current).await.ok();
        }
        current = current.parent()?.to_owned();
    }
}

async fn git_output(root: &std::path::Path, arguments: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::title_from_text;

    #[test]
    fn derives_a_short_title_from_the_first_request() {
        assert_eq!(
            title_from_text("Please implement authentication and password reset. Keep it private."),
            Some("Implement authentication and password reset".into())
        );
        assert_eq!(
            title_from_text(
                "Investigate the very long and unexpectedly complicated synchronization behavior across every connected client"
            ),
            Some("Investigate the very long and unexpectedly complicated".into())
        );
    }
}
