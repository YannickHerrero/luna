use std::{sync::Arc, time::Duration};

use axum::Router;
use luna_pi::SessionRuntimeConfig;
use luna_storage::Database;

use crate::{
    auth::AuthService, config::Config, error::AppError, events::EventHub, routes,
    runtime::ConversationRuntime, state::AppState,
};

pub struct BuiltApp {
    pub router: Router,
    pub pairing_code: String,
    pub database: Database,
    pub runtime: Arc<ConversationRuntime>,
}

pub async fn build(config: Config) -> Result<BuiltApp, AppError> {
    let database = Database::connect(&config.database_path).await?;
    let recovered_at = crate::auth::now()?;
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
    ));
    let state = AppState::new(config, database.clone(), events, runtime.clone());
    let pairing_code = AuthService::new(database.clone())
        .create_pairing_code()
        .await?;
    Ok(BuiltApp {
        router: routes::router(state),
        pairing_code,
        database,
        runtime,
    })
}
