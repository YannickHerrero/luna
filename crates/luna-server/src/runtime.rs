use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use luna_pi::{
    BridgeEvent, ManagedSession, NormalizedPiEvent, RpcDelivery, RpcImage, SessionRuntimeConfig,
    SessionSupervisor,
};
use luna_protocol::{
    ActivityPhase, AgentActivityChanged, AgentModel, CompactConversationResponse, ContextUsage,
    ConversationAgentState, ConversationTitleUpdated, MessageDelivery, RepositoriesUpdated,
    ServerEvent, SessionState, SteeringQueueChanged, ThinkingLevel, UpdateConversationAgentRequest,
    WorkspaceUpdated,
};
use luna_storage::{ConversationRuntimeRecord, Database, RepositoryObservation};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use crate::{auth::now, error::AppError, events::EventHub, title::TitleGenerator};

pub struct ConversationRuntime {
    supervisor: SessionSupervisor,
    database: Database,
    events: EventHub,
    attachment_directory: PathBuf,
    repository_icon_directory: PathBuf,
    title_generator: TitleGenerator,
    title_jobs: Mutex<HashSet<Uuid>>,
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
        repository_icon_directory: PathBuf,
        title_model: String,
    ) -> Self {
        let title_generator = TitleGenerator::new(
            config.pi_executable.clone(),
            title_model,
            Duration::from_secs(90),
        );
        Self {
            supervisor: SessionSupervisor::new(config),
            database,
            events,
            attachment_directory,
            repository_icon_directory,
            title_generator,
            title_jobs: Mutex::new(HashSet::new()),
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
        session.process.abort_retry().await?;
        session.process.abort_bash().await?;
        session.abort().await?;
        self.set_state(conversation_id, SessionState::Interrupted)
            .await?;
        Ok(())
    }

    pub async fn agent_state(
        self: &Arc<Self>,
        conversation_id: Uuid,
    ) -> Result<ConversationAgentState, AppError> {
        let conversation = self
            .database
            .conversation_runtime(conversation_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let session = self.session(conversation).await?;
        agent_state_for_session(&session).await
    }

    pub async fn update_agent(
        self: &Arc<Self>,
        conversation_id: Uuid,
        request: UpdateConversationAgentRequest,
    ) -> Result<ConversationAgentState, AppError> {
        let conversation = self
            .database
            .conversation_runtime(conversation_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let session = self.session(conversation).await?;
        ensure_session_idle(&session).await?;
        let current = agent_state_for_session(&session).await?;
        let selected_model = request.model.as_ref().map(|selection| {
            current
                .available_models
                .iter()
                .find(|model| {
                    model.provider == selection.provider && model.id == selection.model_id
                })
                .cloned()
                .ok_or_else(|| AppError::InvalidRequest("That model is not available.".into()))
        });
        let target_model = match selected_model {
            Some(model) => Some(model?),
            None => current.model.clone(),
        };
        if let Some(level) = request.thinking_level
            && !target_model
                .as_ref()
                .is_some_and(|model| model.supported_thinking_levels.contains(&level))
        {
            return Err(AppError::InvalidRequest(
                "That thinking level is not supported by the selected model.".into(),
            ));
        }
        if let Some(selection) = request.model
            && !current.model.as_ref().is_some_and(|model| {
                model.provider == selection.provider && model.id == selection.model_id
            })
        {
            session
                .process
                .set_model(&selection.provider, &selection.model_id)
                .await?;
        }
        if let Some(level) = request.thinking_level
            && level != current.thinking_level
        {
            session.process.set_thinking_level(level.as_str()).await?;
        }
        agent_state_for_session(&session).await
    }

    pub async fn compact_context(
        self: &Arc<Self>,
        conversation_id: Uuid,
    ) -> Result<CompactConversationResponse, AppError> {
        let conversation = self
            .database
            .conversation_runtime(conversation_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let session = self.session(conversation).await?;
        ensure_session_idle(&session).await?;
        let response = session.process.compact().await?;
        let data = response
            .data
            .ok_or_else(|| AppError::InvalidRequest("Pi returned no compaction result.".into()))?;
        Ok(serde_json::from_value(data)?)
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
        let mut activity: Option<ActivityCapture> = None;
        let mut activity_sequence = 0_i64;
        loop {
            tokio::select! {
                event = pi_events.recv() => {
                    match event {
                        Ok(event) => self.handle_pi_event(
                            conversation_id,
                            event.normalized,
                            &mut assistant_message,
                            &mut chunk_index,
                            &mut activity,
                            &mut activity_sequence,
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
        self: &Arc<Self>,
        conversation_id: Uuid,
        event: NormalizedPiEvent,
        assistant_message: &mut Option<Uuid>,
        chunk_index: &mut i64,
        activity: &mut Option<ActivityCapture>,
        activity_sequence: &mut i64,
    ) {
        let result: Result<(), AppError> = async {
            match event {
                NormalizedPiEvent::AgentStarted => {
                    *activity = None;
                    *activity_sequence = 0;
                    self.set_state(conversation_id, SessionState::Working)
                        .await?;
                    let reset = self
                        .database
                        .reset_agent_activities(conversation_id, &now()?)
                        .await?;
                    self.events.publish(reset);
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
                NormalizedPiEvent::ThinkingStarted => {
                    *activity = Some(ActivityCapture::new(*activity_sequence));
                    *activity_sequence = activity_sequence.saturating_add(1);
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
                NormalizedPiEvent::ThinkingDelta { delta } => {
                    let capture = activity.get_or_insert_with(|| {
                        let next = ActivityCapture::new(*activity_sequence);
                        *activity_sequence = activity_sequence.saturating_add(1);
                        next
                    });
                    if let Some(summary) = capture.update(&delta) {
                        let (_, event) = self
                            .database
                            .upsert_agent_activity(
                                conversation_id,
                                capture.id,
                                capture.sequence,
                                &summary,
                                &now()?,
                            )
                            .await?;
                        self.events.publish(event);
                    }
                }
                NormalizedPiEvent::ThinkingEnded => {
                    *activity = None;
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
                    *activity = None;
                    if let Some(message_id) = assistant_message.take() {
                        let event = self
                            .database
                            .complete_message(conversation_id, message_id, &now()?)
                            .await?;
                        self.events.publish(event);
                    }
                    let reset = self
                        .database
                        .reset_agent_activities(conversation_id, &now()?)
                        .await?;
                    self.events.publish(reset);
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
                    if self.title_jobs.lock().await.insert(conversation_id) {
                        let runtime = Arc::clone(self);
                        tokio::spawn(async move {
                            if let Err(error) = runtime.generate_title(conversation_id).await {
                                warn!(%conversation_id, "Unable to generate conversation title: {error}");
                            }
                            runtime.title_jobs.lock().await.remove(&conversation_id);
                        });
                    }
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
                NormalizedPiEvent::CompactionEnded { succeeded, aborted } => {
                    let state = if succeeded {
                        SessionState::Idle
                    } else if aborted {
                        SessionState::Interrupted
                    } else {
                        SessionState::Error
                    };
                    self.set_state(conversation_id, state).await?;
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
        let icon = self.prepare_repository_icon(&root).await?;
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
                icon_storage_key: icon.as_ref().map(|icon| icon.storage_key.as_str()),
                icon_source: icon.as_ref().map(|icon| icon.source.as_str()),
                icon_fingerprint: icon.as_ref().map(|icon| icon.fingerprint.as_str()),
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

    async fn prepare_repository_icon(
        &self,
        root: &std::path::Path,
    ) -> Result<Option<RepositoryIconAsset>, AppError> {
        let root = root.to_owned();
        let discovered = tokio::task::spawn_blocking(move || discover_repository_icon(&root))
            .await
            .map_err(|error| AppError::DependencyUnavailable(error.to_string()))?;
        let Some((source_path, source)) = discovered else {
            return Ok(None);
        };
        let bytes = tokio::fs::read(&source_path).await?;
        if bytes.is_empty() || bytes.len() > 10 * 1024 * 1024 {
            return Ok(None);
        }
        let extension = source_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .filter(|extension| matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp"))
            .unwrap_or_else(|| "png".into());
        let fingerprint = format!("{:x}", Sha256::digest(&bytes));
        let storage_key = format!("{fingerprint}.{extension}");
        let destination = self.repository_icon_directory.join(&storage_key);
        if !tokio::fs::try_exists(&destination).await? {
            tokio::fs::create_dir_all(&self.repository_icon_directory).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(
                    &self.repository_icon_directory,
                    std::fs::Permissions::from_mode(0o700),
                )
                .await?;
            }
            tokio::fs::write(&destination, bytes).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o600))
                    .await?;
            }
        }
        Ok(Some(RepositoryIconAsset {
            storage_key,
            source,
            fingerprint,
        }))
    }

    async fn generate_title(&self, conversation_id: Uuid) -> Result<(), AppError> {
        let Some(current) = self.database.conversation(conversation_id).await? else {
            return Ok(());
        };
        if current.title_mode != luna_protocol::TitleMode::Automatic
            || current.title != "New Conversation"
        {
            return Ok(());
        }
        let messages = self.database.messages(conversation_id, None, 20).await?;
        let Some(title) = self
            .title_generator
            .generate(&messages)
            .await
            .map_err(|error| AppError::DependencyUnavailable(error.to_string()))?
        else {
            return Ok(());
        };
        let timestamp = now()?;
        let Some(conversation) = self
            .database
            .set_automatic_title(conversation_id, &title, &timestamp)
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiSessionState {
    model: Option<PiModel>,
    thinking_level: ThinkingLevel,
    #[serde(default)]
    is_streaming: bool,
    #[serde(default)]
    is_compacting: bool,
    #[serde(default)]
    auto_compaction_enabled: bool,
}

#[derive(Deserialize)]
struct PiAvailableModels {
    models: Vec<PiModel>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiModel {
    provider: String,
    id: String,
    name: String,
    #[serde(default)]
    reasoning: bool,
    context_window: u64,
    #[serde(default)]
    thinking_level_map: HashMap<String, Option<serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiSessionStats {
    context_usage: Option<ContextUsage>,
}

async fn ensure_session_idle(session: &ManagedSession) -> Result<(), AppError> {
    let state: PiSessionState = rpc_data(session.process.get_state().await?)?;
    if state.is_streaming || state.is_compacting {
        return Err(AppError::Conflict(
            "Wait for the current Pi operation to finish or stop it first.".into(),
        ));
    }
    Ok(())
}

async fn agent_state_for_session(
    session: &ManagedSession,
) -> Result<ConversationAgentState, AppError> {
    let (state_response, models_response, stats_response) = tokio::try_join!(
        session.process.get_state(),
        session.process.get_available_models(),
        session.process.get_session_stats(),
    )?;
    let state: PiSessionState = rpc_data(state_response)?;
    let mut available_models = rpc_data::<PiAvailableModels>(models_response)?
        .models
        .into_iter()
        .map(agent_model)
        .collect::<Vec<_>>();
    available_models.sort_by(|left, right| {
        left.provider
            .cmp(&right.provider)
            .then_with(|| left.name.cmp(&right.name))
    });
    let stats: PiSessionStats = rpc_data(stats_response)?;
    Ok(ConversationAgentState {
        model: state.model.map(agent_model),
        thinking_level: state.thinking_level,
        available_models,
        context_usage: stats.context_usage,
        auto_compaction_enabled: state.auto_compaction_enabled,
    })
}

fn rpc_data<T: for<'de> Deserialize<'de>>(response: luna_pi::RpcResponse) -> Result<T, AppError> {
    Ok(serde_json::from_value(response.data.ok_or_else(|| {
        serde_json::Error::io(std::io::Error::other("missing RPC data"))
    })?)?)
}

fn agent_model(model: PiModel) -> AgentModel {
    let supported_thinking_levels = if model.reasoning {
        [
            ThinkingLevel::Off,
            ThinkingLevel::Minimal,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ]
        .into_iter()
        .filter(|level| !matches!(model.thinking_level_map.get(level.as_str()), Some(None)))
        .chain(
            [ThinkingLevel::Xhigh, ThinkingLevel::Max]
                .into_iter()
                .filter(|level| {
                    model
                        .thinking_level_map
                        .get(level.as_str())
                        .is_some_and(Option::is_some)
                }),
        )
        .collect()
    } else {
        vec![ThinkingLevel::Off]
    };
    AgentModel {
        provider: model.provider,
        id: model.id,
        name: model.name,
        reasoning: model.reasoning,
        context_window: model.context_window,
        supported_thinking_levels,
    }
}

struct ActivityCapture {
    id: Uuid,
    sequence: i64,
    source: String,
    published_summary: Option<String>,
}

impl ActivityCapture {
    fn new(sequence: i64) -> Self {
        Self {
            id: Uuid::now_v7(),
            sequence,
            source: String::new(),
            published_summary: None,
        }
    }

    fn update(&mut self, delta: &str) -> Option<String> {
        let remaining = 2_048_usize.saturating_sub(self.source.chars().count());
        self.source.extend(delta.chars().take(remaining));
        let summary = progress_summary(&self.source)?;
        if self.published_summary.as_deref() == Some(&summary) {
            return None;
        }
        self.published_summary = Some(summary.clone());
        Some(summary)
    }
}

fn progress_summary(value: &str) -> Option<String> {
    let line = value.lines().find(|line| !line.trim().is_empty())?.trim();
    let line = line
        .trim_start_matches(['#', '>', '-', '*', '_', '`', ' '])
        .trim_end_matches(['*', '_', '`', ' ']);
    let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }
    Some(compact.chars().take(240).collect())
}

struct RepositoryIconAsset {
    storage_key: String,
    source: String,
    fingerprint: String,
}

fn discover_repository_icon(root: &std::path::Path) -> Option<(PathBuf, String)> {
    let mut candidates: Vec<(u64, PathBuf, String)> = Vec::new();
    for config_name in ["app.json", "app.config.json"] {
        let config_path = root.join(config_name);
        if let Ok(bytes) = std::fs::read(&config_path)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && let Some(icon) = value
                .get("expo")
                .and_then(|expo| expo.get("icon"))
                .and_then(serde_json::Value::as_str)
        {
            add_icon_candidate(&mut candidates, root.join(icon), 1_000, "expo_icon");
        }
    }

    let mut pending = vec![(root.to_owned(), 0_u8)];
    while let Some((directory, depth)) = pending.pop() {
        if depth > 7 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if path.is_dir() {
                if matches!(
                    file_name.as_str(),
                    ".git" | "node_modules" | "target" | "build" | "dist" | ".next" | "pods"
                ) {
                    continue;
                }
                pending.push((path, depth + 1));
                continue;
            }
            if matches!(
                file_name.as_str(),
                "manifest.json" | "site.webmanifest" | "manifest.webmanifest"
            ) && let Ok(bytes) = std::fs::read(&path)
                && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
                && let Some(icons) = value.get("icons").and_then(serde_json::Value::as_array)
            {
                for icon in icons {
                    if let Some(source) = icon.get("src").and_then(serde_json::Value::as_str) {
                        add_icon_candidate(
                            &mut candidates,
                            path.parent()
                                .unwrap_or(root)
                                .join(source.trim_start_matches('/')),
                            700,
                            "web_manifest",
                        );
                    }
                }
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase);
            if !extension
                .as_deref()
                .is_some_and(|extension| matches!(extension, "png" | "jpg" | "jpeg" | "webp"))
            {
                continue;
            }
            let parent = path
                .parent()
                .and_then(std::path::Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let (score, source) = if parent.ends_with(".appiconset") {
                (900, "ios_app_icon")
            } else if file_name.starts_with("ic_launcher") {
                (800, "android_app_icon")
            } else if file_name.starts_with("apple-touch-icon") {
                (650, "apple_touch_icon")
            } else if matches!(file_name.as_str(), "icon.png" | "app-icon.png" | "logo.png") {
                (600, "conventional_icon")
            } else if file_name.starts_with("favicon") {
                (500, "favicon")
            } else {
                continue;
            };
            add_icon_candidate(&mut candidates, path, score, source);
        }
    }
    candidates
        .into_iter()
        .max_by_key(|(score, _, _)| *score)
        .map(|(_, path, source)| (path, source))
}

fn add_icon_candidate(
    candidates: &mut Vec<(u64, PathBuf, String)>,
    path: PathBuf,
    score: u64,
    source: &str,
) {
    let Some(extension) = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return;
    };
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return;
    }
    if let Ok(metadata) = std::fs::metadata(&path)
        && metadata.is_file()
        && metadata.len() > 0
    {
        candidates.push((
            score.saturating_add(metadata.len().min(10_000_000) / 10_000),
            path,
            source.into(),
        ));
    }
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
    use super::{ActivityCapture, progress_summary};

    #[test]
    fn derives_short_progress_summaries_from_thinking_headings() {
        assert_eq!(
            progress_summary("\n**Planning Luna deployment with log verification**\n\nDetails"),
            Some("Planning Luna deployment with log verification".into())
        );
        let mut capture = ActivityCapture::new(0);
        assert_eq!(
            capture.update("**Finalizing Luna"),
            Some("Finalizing Luna".into())
        );
        assert_eq!(
            capture.update(" restart and log validation**\nMore reasoning"),
            Some("Finalizing Luna restart and log validation".into())
        );
        assert_eq!(capture.update(" that stays private"), None);
    }
}
