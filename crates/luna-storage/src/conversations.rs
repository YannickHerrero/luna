use luna_protocol::{Conversation, Repository, RepositoryIcon, SessionState, TitleMode};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, StorageError};

#[derive(FromRow)]
struct ConversationRow {
    id: String,
    title: String,
    title_mode: String,
    state: String,
    preview: String,
    active_working_directory: String,
    notification_target_device_id: Option<String>,
    unread_count: i64,
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
    version: i64,
}

#[derive(FromRow)]
pub struct ConversationRuntimeRecord {
    pub conversation: Conversation,
    pub pi_session_id: Option<String>,
    pub pi_session_path: Option<String>,
}

#[derive(FromRow)]
struct RuntimeConversationRow {
    pi_session_id: Option<String>,
    pi_session_path: Option<String>,
}

#[derive(FromRow)]
struct RepositoryRow {
    id: String,
    display_name: String,
    canonical_root: String,
    branch: Option<String>,
    active: bool,
    icon_storage_key: Option<String>,
    first_seen_at: String,
    last_seen_at: String,
}

fn parse_state(value: &str) -> Result<SessionState, StorageError> {
    match value {
        "creating" => Ok(SessionState::Creating),
        "starting" => Ok(SessionState::Starting),
        "idle" => Ok(SessionState::Idle),
        "working" => Ok(SessionState::Working),
        "compacting" => Ok(SessionState::Compacting),
        "retrying" => Ok(SessionState::Retrying),
        "crashed" => Ok(SessionState::Crashed),
        "restoring" => Ok(SessionState::Restoring),
        "interrupted" => Ok(SessionState::Interrupted),
        "stopped" => Ok(SessionState::Stopped),
        "error" => Ok(SessionState::Error),
        value => Err(StorageError::InvalidEnum(value.into())),
    }
}

fn state_name(value: SessionState) -> &'static str {
    match value {
        SessionState::Creating => "creating",
        SessionState::Starting => "starting",
        SessionState::Idle => "idle",
        SessionState::Working => "working",
        SessionState::Compacting => "compacting",
        SessionState::Retrying => "retrying",
        SessionState::Crashed => "crashed",
        SessionState::Restoring => "restoring",
        SessionState::Interrupted => "interrupted",
        SessionState::Stopped => "stopped",
        SessionState::Error => "error",
    }
}

impl Database {
    pub async fn create_conversation(
        &self,
        id: Uuid,
        home_directory: &str,
        created_at: &str,
    ) -> Result<Conversation, StorageError> {
        sqlx::query(
            "INSERT INTO conversations (id, active_working_directory, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(home_directory)
        .bind(created_at)
        .bind(created_at)
        .execute(self.pool())
        .await?;
        self.conversation(id).await?.ok_or(StorageError::NotFound)
    }

    pub async fn conversations(
        &self,
        include_archived: bool,
    ) -> Result<Vec<Conversation>, StorageError> {
        let query = if include_archived {
            "SELECT id, title, title_mode, state, preview, active_working_directory, notification_target_device_id, unread_count, archived_at, created_at, updated_at, version FROM conversations ORDER BY updated_at DESC"
        } else {
            "SELECT id, title, title_mode, state, preview, active_working_directory, notification_target_device_id, unread_count, archived_at, created_at, updated_at, version FROM conversations WHERE archived_at IS NULL ORDER BY updated_at DESC"
        };
        let rows = sqlx::query_as::<_, ConversationRow>(query)
            .fetch_all(self.pool())
            .await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result.push(self.map_conversation(row).await?);
        }
        Ok(result)
    }

    pub async fn conversation(&self, id: Uuid) -> Result<Option<Conversation>, StorageError> {
        let row = sqlx::query_as::<_, ConversationRow>(
            "SELECT id, title, title_mode, state, preview, active_working_directory, notification_target_device_id, unread_count, archived_at, created_at, updated_at, version FROM conversations WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(self.pool())
        .await?;
        match row {
            Some(row) => Ok(Some(self.map_conversation(row).await?)),
            None => Ok(None),
        }
    }

    pub async fn conversation_runtime(
        &self,
        id: Uuid,
    ) -> Result<Option<ConversationRuntimeRecord>, StorageError> {
        let Some(conversation) = self.conversation(id).await? else {
            return Ok(None);
        };
        let runtime = sqlx::query_as::<_, RuntimeConversationRow>(
            "SELECT pi_session_id, pi_session_path FROM conversations WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_one(self.pool())
        .await?;
        Ok(Some(ConversationRuntimeRecord {
            conversation,
            pi_session_id: runtime.pi_session_id,
            pi_session_path: runtime.pi_session_path,
        }))
    }

    pub async fn recover_interrupted_conversations(
        &self,
        updated_at: &str,
    ) -> Result<Vec<Uuid>, StorageError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM conversations WHERE archived_at IS NULL AND state IN ('starting', 'working', 'compacting', 'retrying', 'restoring')",
        )
        .fetch_all(self.pool())
        .await?;
        if !ids.is_empty() {
            sqlx::query(
                "UPDATE conversations SET state = 'crashed', updated_at = ?, version = version + 1 WHERE archived_at IS NULL AND state IN ('starting', 'working', 'compacting', 'retrying', 'restoring')",
            )
            .bind(updated_at)
            .execute(self.pool())
            .await?;
        }
        ids.into_iter()
            .map(|id| Uuid::parse_str(&id).map_err(StorageError::from))
            .collect()
    }

    pub async fn set_conversation_session(
        &self,
        id: Uuid,
        session_id: &str,
        session_path: &str,
        updated_at: &str,
    ) -> Result<(), StorageError> {
        let updated = sqlx::query(
            "UPDATE conversations SET pi_session_id = ?, pi_session_path = ?, updated_at = ?, version = version + 1 WHERE id = ?",
        )
        .bind(session_id)
        .bind(session_path)
        .bind(updated_at)
        .bind(id.to_string())
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub async fn set_conversation_state(
        &self,
        id: Uuid,
        state: SessionState,
        updated_at: &str,
    ) -> Result<(), StorageError> {
        let updated = sqlx::query(
            "UPDATE conversations SET state = ?, updated_at = ?, version = version + 1 WHERE id = ?",
        )
        .bind(state_name(state))
        .bind(updated_at)
        .bind(id.to_string())
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub async fn set_working_directory(
        &self,
        id: Uuid,
        working_directory: &str,
        updated_at: &str,
    ) -> Result<(), StorageError> {
        let updated = sqlx::query(
            "UPDATE conversations SET active_working_directory = ?, updated_at = ?, version = version + 1 WHERE id = ?",
        )
        .bind(working_directory)
        .bind(updated_at)
        .bind(id.to_string())
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub async fn set_automatic_title(
        &self,
        id: Uuid,
        title: &str,
        updated_at: &str,
    ) -> Result<Option<Conversation>, StorageError> {
        let updated = sqlx::query(
            "UPDATE conversations SET title = ?, updated_at = ?, version = version + 1 WHERE id = ? AND title_mode = 'automatic' AND title != ?",
        )
        .bind(title)
        .bind(updated_at)
        .bind(id.to_string())
        .bind(title)
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        self.conversation(id).await
    }

    pub async fn rename_conversation(
        &self,
        id: Uuid,
        title: &str,
        updated_at: &str,
    ) -> Result<Conversation, StorageError> {
        let updated = sqlx::query(
            "UPDATE conversations SET title = ?, title_mode = 'manual', updated_at = ?, version = version + 1 WHERE id = ?",
        )
        .bind(title)
        .bind(updated_at)
        .bind(id.to_string())
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        self.conversation(id).await?.ok_or(StorageError::NotFound)
    }

    pub async fn archive_conversation(
        &self,
        id: Uuid,
        archived_at: &str,
    ) -> Result<(), StorageError> {
        let updated = sqlx::query(
            "UPDATE conversations SET archived_at = ?, state = 'stopped', updated_at = ?, version = version + 1 WHERE id = ?",
        )
        .bind(archived_at)
        .bind(archived_at)
        .bind(id.to_string())
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    async fn map_conversation(&self, row: ConversationRow) -> Result<Conversation, StorageError> {
        let id = Uuid::parse_str(&row.id)?;
        let repository_rows = sqlx::query_as::<_, RepositoryRow>(
            "SELECT r.id, r.display_name, r.canonical_root, cr.branch, cr.active, r.icon_storage_key, cr.first_seen_at, cr.last_seen_at FROM conversation_repositories cr JOIN repositories r ON r.id = cr.repository_id WHERE cr.conversation_id = ? ORDER BY cr.active DESC, cr.last_seen_at DESC",
        )
        .bind(&row.id)
        .fetch_all(self.pool())
        .await?;
        let repositories = repository_rows
            .into_iter()
            .map(|repository| {
                let repository_id = Uuid::parse_str(&repository.id)?;
                Ok(Repository {
                    id: repository_id,
                    display_name: repository.display_name.clone(),
                    root_path: repository.canonical_root,
                    branch: repository.branch,
                    active: repository.active,
                    icon: RepositoryIcon {
                        repository_id,
                        content_url: repository
                            .icon_storage_key
                            .map(|_| format!("/v1/repositories/{repository_id}/icon")),
                        fallback_text: repository
                            .display_name
                            .chars()
                            .next()
                            .unwrap_or('•')
                            .to_uppercase()
                            .to_string(),
                        fallback_color: "#7287fd".into(),
                    },
                    first_seen_at: repository.first_seen_at,
                    last_seen_at: repository.last_seen_at,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        Ok(Conversation {
            id,
            title: row.title,
            title_mode: match row.title_mode.as_str() {
                "automatic" => TitleMode::Automatic,
                "manual" => TitleMode::Manual,
                value => return Err(StorageError::InvalidEnum(value.into())),
            },
            state: parse_state(&row.state)?,
            preview: row.preview,
            active_working_directory: row.active_working_directory,
            repositories,
            notification_target_device_id: row
                .notification_target_device_id
                .map(|value| Uuid::parse_str(&value))
                .transpose()?,
            unread_count: row.unread_count,
            archived_at: row.archived_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}
