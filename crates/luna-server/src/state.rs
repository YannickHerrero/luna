use std::sync::Arc;

use luna_storage::Database;

use crate::{auth::AuthService, config::Config};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Database,
    pub auth: AuthService,
}

impl AppState {
    #[must_use]
    pub fn new(config: Config, database: Database) -> Self {
        let auth = AuthService::new(database.clone());
        Self {
            config: Arc::new(config),
            database,
            auth,
        }
    }
}
