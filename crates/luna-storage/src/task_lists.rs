use luna_protocol::{
    AgentTask, AgentTaskList, AgentTaskListChanged, AgentTaskStatus, ServerEvent,
    ServerEventEnvelope,
};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, StorageError, events::insert_event};

#[derive(FromRow)]
struct TaskListRow {
    id: String,
    title: Option<String>,
    revision: i64,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct TaskRow {
    id: String,
    sequence: i64,
    text: String,
    status: String,
    note: Option<String>,
    created_at: String,
    updated_at: String,
}

impl Database {
    pub async fn agent_task_list(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<AgentTaskList>, StorageError> {
        let row = sqlx::query_as::<_, TaskListRow>(
            "SELECT id, title, revision, created_at, updated_at FROM agent_task_lists WHERE conversation_id = ?",
        )
        .bind(conversation_id.to_string())
        .fetch_optional(self.pool())
        .await?;
        let Some(row) = row else { return Ok(None) };
        let task_rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, sequence, text, status, note, created_at, updated_at FROM agent_tasks WHERE task_list_id = ? ORDER BY sequence ASC",
        )
        .bind(&row.id)
        .fetch_all(self.pool())
        .await?;
        Ok(Some(AgentTaskList {
            id: Uuid::parse_str(&row.id)?,
            title: row.title,
            revision: row.revision,
            tasks: task_rows
                .into_iter()
                .map(map_task)
                .collect::<Result<Vec<_>, StorageError>>()?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }

    pub async fn replace_agent_task_list(
        &self,
        conversation_id: Uuid,
        task_list: &AgentTaskList,
        observed_at: &str,
    ) -> Result<ServerEventEnvelope, StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query("DELETE FROM agent_task_lists WHERE conversation_id = ?")
            .bind(conversation_id.to_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO agent_task_lists (id, conversation_id, title, revision, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(task_list.id.to_string())
        .bind(conversation_id.to_string())
        .bind(&task_list.title)
        .bind(task_list.revision)
        .bind(&task_list.created_at)
        .bind(&task_list.updated_at)
        .execute(&mut *transaction)
        .await?;
        for task in &task_list.tasks {
            sqlx::query(
                "INSERT INTO agent_tasks (id, task_list_id, sequence, text, status, note, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(task.id.to_string())
            .bind(task_list.id.to_string())
            .bind(task.sequence)
            .bind(&task.text)
            .bind(status_name(task.status))
            .bind(&task.note)
            .bind(&task.created_at)
            .bind(&task.updated_at)
            .execute(&mut *transaction)
            .await?;
        }
        update_conversation(&mut transaction, conversation_id, observed_at).await?;
        let event = insert_event(
            &mut transaction,
            conversation_id,
            task_list.id,
            &ServerEvent::AgentTaskListChanged(AgentTaskListChanged {
                task_list: Some(task_list.clone()),
            }),
            observed_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(event)
    }

    pub async fn clear_agent_task_list(
        &self,
        conversation_id: Uuid,
        observed_at: &str,
    ) -> Result<ServerEventEnvelope, StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query("DELETE FROM agent_task_lists WHERE conversation_id = ?")
            .bind(conversation_id.to_string())
            .execute(&mut *transaction)
            .await?;
        update_conversation(&mut transaction, conversation_id, observed_at).await?;
        let event = insert_event(
            &mut transaction,
            conversation_id,
            conversation_id,
            &ServerEvent::AgentTaskListChanged(AgentTaskListChanged { task_list: None }),
            observed_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(event)
    }
}

async fn update_conversation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    conversation_id: Uuid,
    observed_at: &str,
) -> Result<(), StorageError> {
    let updated =
        sqlx::query("UPDATE conversations SET updated_at = ?, version = version + 1 WHERE id = ?")
            .bind(observed_at)
            .bind(conversation_id.to_string())
            .execute(&mut **transaction)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(StorageError::NotFound);
    }
    Ok(())
}

fn status_name(status: AgentTaskStatus) -> &'static str {
    match status {
        AgentTaskStatus::Pending => "pending",
        AgentTaskStatus::InProgress => "in_progress",
        AgentTaskStatus::Completed => "completed",
        AgentTaskStatus::Blocked => "blocked",
        AgentTaskStatus::Skipped => "skipped",
    }
}

fn map_task(row: TaskRow) -> Result<AgentTask, StorageError> {
    let status = match row.status.as_str() {
        "pending" => AgentTaskStatus::Pending,
        "in_progress" => AgentTaskStatus::InProgress,
        "completed" => AgentTaskStatus::Completed,
        "blocked" => AgentTaskStatus::Blocked,
        "skipped" => AgentTaskStatus::Skipped,
        value => return Err(StorageError::InvalidEnum(value.into())),
    };
    Ok(AgentTask {
        id: Uuid::parse_str(&row.id)?,
        sequence: row.sequence,
        text: row.text,
        status,
        note: row.note,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
