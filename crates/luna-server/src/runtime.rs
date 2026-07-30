use std::{
    collections::HashSet,
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
    ActivityPhase, AgentActivityChanged, AgentTaskList, ConversationTitleUpdated, MessageDelivery,
    RepositoriesUpdated, ServerEvent, SessionState, SteeringQueueChanged, WorkspaceUpdated,
};
use luna_storage::{ConversationRuntimeRecord, Database, RepositoryObservation};
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
                "task_list_updated" => {
                    let Some(task_list) = event.task_list else {
                        return Ok(());
                    };
                    validate_task_list(&task_list)?;
                    let event = self
                        .database
                        .replace_agent_task_list(conversation_id, &task_list, &now()?)
                        .await?;
                    self.events.publish(event);
                }
                "task_list_cleared" => {
                    let event = self
                        .database
                        .clear_agent_task_list(conversation_id, &now()?)
                        .await?;
                    self.events.publish(event);
                }
                _ => {}
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            warn!(%conversation_id, "Unable to persist Pi bridge event: {error}");
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

fn validate_task_list(task_list: &AgentTaskList) -> Result<(), AppError> {
    if task_list.revision < 1 {
        return Err(AppError::InvalidRequest(
            "Task-list revision must be positive".into(),
        ));
    }
    if task_list.tasks.is_empty() || task_list.tasks.len() > 30 {
        return Err(AppError::InvalidRequest(
            "Task lists must contain between 1 and 30 tasks".into(),
        ));
    }
    if task_list
        .title
        .as_ref()
        .is_some_and(|title| title.trim().is_empty() || title.chars().count() > 120)
    {
        return Err(AppError::InvalidRequest(
            "Task-list title is invalid".into(),
        ));
    }
    let mut ids = HashSet::new();
    for (index, task) in task_list.tasks.iter().enumerate() {
        let expected_sequence = i64::try_from(index + 1).unwrap_or(i64::MAX);
        if task.sequence != expected_sequence || !ids.insert(task.id) {
            return Err(AppError::InvalidRequest(
                "Task identifiers and sequence must be unique and ordered".into(),
            ));
        }
        if task.text.trim().is_empty() || task.text.chars().count() > 240 {
            return Err(AppError::InvalidRequest("Task text is invalid".into()));
        }
        if task
            .note
            .as_ref()
            .is_some_and(|note| note.trim().is_empty() || note.chars().count() > 500)
        {
            return Err(AppError::InvalidRequest("Task note is invalid".into()));
        }
    }
    Ok(())
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
    use luna_protocol::{AgentTask, AgentTaskList, AgentTaskStatus};
    use uuid::Uuid;

    use super::{ActivityCapture, progress_summary, validate_task_list};

    #[test]
    fn validates_structured_task_list_boundaries() {
        let timestamp = "2026-03-20T12:00:00Z".to_owned();
        let mut task_list = AgentTaskList {
            id: Uuid::new_v4(),
            title: Some("Ship progress".into()),
            revision: 1,
            tasks: vec![AgentTask {
                id: Uuid::new_v4(),
                sequence: 1,
                text: "Verify progress".into(),
                status: AgentTaskStatus::InProgress,
                note: None,
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
            }],
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        assert!(validate_task_list(&task_list).is_ok());
        task_list.tasks[0].sequence = 2;
        assert!(validate_task_list(&task_list).is_err());
    }

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
