use std::{path::Path, time::Duration};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

use crate::StorageError;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        sqlx::query("PRAGMA application_id = 1280659009")
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }

    pub async fn ping(&self) -> Result<(), StorageError> {
        let value: i64 = sqlx::query_scalar("SELECT 1").fetch_one(&self.pool).await?;
        if value != 1 {
            return Err(StorageError::Conflict);
        }
        Ok(())
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(self) {
        self.pool.close().await;
    }
}
