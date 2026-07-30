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
    pub icon_storage_key: Option<&'a str>,
    pub icon_source: Option<&'a str>,
    pub icon_fingerprint: Option<&'a str>,
    pub observed_at: &'a str,
}

pub struct RepositoryObservationResult {
    pub repositories: Vec<Repository>,
    pub changed: bool,
}

pub struct RepositoryIconFile {
    pub storage_key: String,
}

impl Database {
    pub async fn repository_icon_file(
        &self,
        id: Uuid,
    ) -> Result<Option<RepositoryIconFile>, StorageError> {
        let storage_key: Option<String> = sqlx::query_scalar(
            "SELECT icon_storage_key FROM repositories WHERE id = ? AND icon_storage_key IS NOT NULL",
        )
        .bind(id.to_string())
        .fetch_optional(self.pool())
        .await?
        .flatten();
        Ok(storage_key.map(|storage_key| RepositoryIconFile { storage_key }))
    }

    pub async fn observe_repository(
        &self,
        observation: RepositoryObservation<'_>,
    ) -> Result<RepositoryObservationResult, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let existing = sqlx::query(
            "SELECT id, display_name, git_directory, icon_fingerprint FROM repositories WHERE canonical_root = ?",
        )
        .bind(observation.canonical_root)
        .fetch_optional(&mut *transaction)
        .await?;
        let (repository_id, mut changed) = match existing {
            Some(row) => {
                let id: String = row.get("id");
                let old_name: String = row.get("display_name");
                let old_git_directory: String = row.get("git_directory");
                let old_icon_fingerprint: Option<String> = row.get("icon_fingerprint");
                let changed = old_name != observation.display_name
                    || old_git_directory != observation.git_directory
                    || observation
                        .icon_fingerprint
                        .is_some_and(|value| Some(value) != old_icon_fingerprint.as_deref());
                sqlx::query(
                    "UPDATE repositories SET display_name = ?, git_directory = ?, icon_storage_key = COALESCE(?, icon_storage_key), icon_source = COALESCE(?, icon_source), icon_fingerprint = COALESCE(?, icon_fingerprint), updated_at = ? WHERE id = ?",
                )
                .bind(observation.display_name)
                .bind(observation.git_directory)
                .bind(observation.icon_storage_key)
                .bind(observation.icon_source)
                .bind(observation.icon_fingerprint)
                .bind(observation.observed_at)
                .bind(&id)
                .execute(&mut *transaction)
                .await?;
                (Uuid::parse_str(&id)?, changed)
            }
            None => {
                let id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO repositories (id, canonical_root, git_directory, display_name, icon_storage_key, icon_source, icon_fingerprint, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(id.to_string())
                .bind(observation.canonical_root)
                .bind(observation.git_directory)
                .bind(observation.display_name)
                .bind(observation.icon_storage_key)
                .bind(observation.icon_source)
                .bind(observation.icon_fingerprint)
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
