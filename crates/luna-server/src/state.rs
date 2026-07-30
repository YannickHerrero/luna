use std::sync::Arc;

use luna_storage::Database;

use crate::{
    auth::AuthService, config::Config, events::EventHub, runtime::ConversationRuntime,
    transcription::TranscriptionService,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Database,
    pub auth: AuthService,
    pub events: EventHub,
    pub runtime: Arc<ConversationRuntime>,
    pub transcription: TranscriptionService,
}

impl AppState {
    #[must_use]
    pub fn new(
        config: Config,
        database: Database,
        events: EventHub,
        runtime: Arc<ConversationRuntime>,
        transcription: TranscriptionService,
    ) -> Self {
        let auth = AuthService::new(database.clone());
        Self {
            config: Arc::new(config),
            database,
            auth,
            events,
            runtime,
            transcription,
        }
    }
}
