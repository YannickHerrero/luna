use axum::Router;
use luna_storage::Database;

use crate::{auth::AuthService, config::Config, error::AppError, routes, state::AppState};

pub struct BuiltApp {
    pub router: Router,
    pub pairing_code: String,
    pub database: Database,
}

pub async fn build(config: Config) -> Result<BuiltApp, AppError> {
    let database = Database::connect(&config.database_path).await?;
    let state = AppState::new(config, database.clone());
    let pairing_code = AuthService::new(database.clone())
        .create_pairing_code()
        .await?;
    Ok(BuiltApp {
        router: routes::router(state),
        pairing_code,
        database,
    })
}
