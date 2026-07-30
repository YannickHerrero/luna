use luna_protocol::Repository;
use sqlx::Row;
use uuid::Uuid;

use crate::{Database, StorageError};

pub struct RepositoryObservation<'a> {
    pub conversation_id: Uuid,
    pub canonical_root: &'a str,
    pub git_directory: &'a str,
    pub display_name: &'a str,
    pub branch: Option<&'a str>,
    pub active: bool,
    pub observed_at: &'a str,
}

pub struct RepositoryObservationResult {
    pub repositories: Vec<Repository>,
    pub changed: bool,
}

impl Database {
    pub async fn observe_repository(
        &self,
        observation: RepositoryObservation<'_>,
    ) -> Result<RepositoryObservationResult, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let existing = sqlx::query(
            "SELECT id, display_name, git_directory FROM repositories WHERE canonical_root = ?",
        )
        .bind(observation.canonical_root)
        .fetch_optional(&mut *transaction)
        .await?;
        let (repository_id, mut changed) = match existing {
            Some(row) => {
                let id: String = row.get("id");
                let old_name: String = row.get("display_name");
                let old_git_directory: String = row.get("git_directory");
                let changed = old_name != observation.display_name
                    || old_git_directory != observation.git_directory;
                sqlx::query(
                    "UPDATE repositories SET display_name = ?, git_directory = ?, updated_at = ? WHERE id = ?",
                )
                .bind(observation.display_name)
                .bind(observation.git_directory)
                .bind(observation.observed_at)
                .bind(&id)
                .execute(&mut *transaction)
                .await?;
                (Uuid::parse_str(&id)?, changed)
            }
            None => {
                let id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO repositories (id, canonical_root, git_directory, display_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(id.to_string())
                .bind(observation.canonical_root)
                .bind(observation.git_directory)
                .bind(observation.display_name)
                .bind(observation.observed_at)
                .bind(observation.observed_at)
                .execute(&mut *transaction)
                .await?;
                (id, true)
            }
        };
        let relationship = sqlx::query(
            "SELECT branch, active FROM conversation_repositories WHERE conversation_id = ? AND repository_id = ?",
        )
        .bind(observation.conversation_id.to_string())
        .bind(repository_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(row) = relationship {
            let old_branch: Option<String> = row.get("branch");
            let old_active: bool = row.get("active");
            let next_active = observation.active || old_active;
            changed |= old_branch.as_deref() != observation.branch || old_active != next_active;
            sqlx::query(
                "UPDATE conversation_repositories SET branch = ?, active = ?, last_seen_at = ? WHERE conversation_id = ? AND repository_id = ?",
            )
            .bind(observation.branch)
            .bind(next_active)
            .bind(observation.observed_at)
            .bind(observation.conversation_id.to_string())
            .bind(repository_id.to_string())
            .execute(&mut *transaction)
            .await?;
        } else {
            changed = true;
            sqlx::query(
                "INSERT INTO conversation_repositories (conversation_id, repository_id, branch, active, first_seen_at, last_seen_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(observation.conversation_id.to_string())
            .bind(repository_id.to_string())
            .bind(observation.branch)
            .bind(observation.active)
            .bind(observation.observed_at)
            .bind(observation.observed_at)
            .execute(&mut *transaction)
            .await?;
        }
        if observation.active {
            let deactivated = sqlx::query(
                "UPDATE conversation_repositories SET active = 0 WHERE conversation_id = ? AND repository_id != ? AND active = 1",
            )
            .bind(observation.conversation_id.to_string())
            .bind(repository_id.to_string())
            .execute(&mut *transaction)
            .await?;
            changed |= deactivated.rows_affected() > 0;
        }
        transaction.commit().await?;
        let repositories = self
            .conversation(observation.conversation_id)
            .await?
            .ok_or(StorageError::NotFound)?
            .repositories;
        Ok(RepositoryObservationResult {
            repositories,
            changed,
        })
    }
}
