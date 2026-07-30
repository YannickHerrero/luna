#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("stored identifier is invalid: {0}")]
    InvalidIdentifier(#[from] uuid::Error),
    #[error("stored JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("record was not found")]
    NotFound,
    #[error("record conflicts with existing state")]
    Conflict,
    #[error("stored enum value is invalid: {0}")]
    InvalidEnum(String),
}
