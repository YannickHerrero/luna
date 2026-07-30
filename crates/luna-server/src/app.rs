use std::{sync::Arc, time::Duration};

use axum::Router;
use luna_pi::SessionRuntimeConfig;
use luna_storage::Database;
use tower_http::services::ServeDir;

use crate::{
    auth::AuthService, config::Config, error::AppError, events::EventHub, maintenance::Maintenance,
    routes, runtime::ConversationRuntime, state::AppState, transcription::TranscriptionService,
};

pub struct BuiltApp {
    pub router: Router,
    pub pairing_code: String,
    pub database: Database,
    pub runtime: Arc<ConversationRuntime>,
    pub maintenance: Maintenance,
}

pub async fn build(config: Config) -> Result<BuiltApp, AppError> {
    let web_directory = config.web_directory.clone();
    let database = Database::connect(&config.database_path).await?;
    let recovered_at = crate::auth::now()?;
    database.recover_inflight_dispatches(&recovered_at).await?;
    for message in database.recover_streaming_messages(&recovered_at).await? {
        database
            .append_event(
                Some(message.conversation_id),
                Some(message.id),
                &luna_protocol::ServerEvent::MessageUpserted(message),
                &recovered_at,
            )
            .await?;
    }
    for conversation_id in database
        .recover_interrupted_conversations(&recovered_at)
        .await?
    {
        database
            .append_event(
                Some(conversation_id),
                Some(conversation_id),
                &luna_protocol::ServerEvent::SessionStateChanged {
                    state: luna_protocol::SessionState::Crashed,
                },
                &recovered_at,
            )
            .await?;
    }
    let maintenance = Maintenance::spawn(database.clone(), config.event_retention_days);
    let events = EventHub::new(database.clone());
    let runtime = Arc::new(ConversationRuntime::new(
        SessionRuntimeConfig {
            pi_executable: config.pi_executable.clone(),
            bridge_extension: config.pi_bridge_path.clone(),
            session_directory: config.pi_session_directory.clone(),
            bridge_directory: config.bridge_directory.clone(),
            request_timeout: Duration::from_secs(15),
        },
        database.clone(),
        events.clone(),
        config.attachment_directory.clone(),
    ));
    let transcription = TranscriptionService::new(
        config.transcription_api_key.clone(),
        config.transcription_base_url.clone(),
        config.transcription_model.clone(),
    )?;
    let state = AppState::new(
        config,
        database.clone(),
        events,
        runtime.clone(),
        transcription,
    );
    let pairing_code = AuthService::new(database.clone())
        .create_pairing_code()
        .await?;
    Ok(BuiltApp {
        router: routes::router(state)
            .fallback_service(ServeDir::new(web_directory).append_index_html_on_directories(true)),
        pairing_code,
        database,
        runtime,
        maintenance,
    })
}
