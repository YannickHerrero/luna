use luna_protocol::{AgentActivitiesReset, AgentActivity, ServerEvent, ServerEventEnvelope};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, StorageError, events::insert_event};

#[derive(FromRow)]
struct AgentActivityRow {
    id: String,
    sequence: i64,
    summary: String,
    created_at: String,
    updated_at: String,
}

impl Database {
    pub async fn agent_activities(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<AgentActivity>, StorageError> {
        sqlx::query_as::<_, AgentActivityRow>(
            "SELECT id, sequence, summary, created_at, updated_at FROM agent_activities WHERE conversation_id = ? ORDER BY sequence ASC",
        )
        .bind(conversation_id.to_string())
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(map_activity)
        .collect()
    }

    pub async fn upsert_agent_activity(
        &self,
        conversation_id: Uuid,
        activity_id: Uuid,
        sequence: i64,
        summary: &str,
        updated_at: &str,
    ) -> Result<(AgentActivity, ServerEventEnvelope), StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO agent_activities (id, conversation_id, sequence, summary, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET summary = excluded.summary, updated_at = excluded.updated_at WHERE conversation_id = excluded.conversation_id",
        )
        .bind(activity_id.to_string())
        .bind(conversation_id.to_string())
        .bind(sequence)
        .bind(summary)
        .bind(updated_at)
        .bind(updated_at)
        .execute(&mut *transaction)
        .await?;
        let row = sqlx::query_as::<_, AgentActivityRow>(
            "SELECT id, sequence, summary, created_at, updated_at FROM agent_activities WHERE id = ? AND conversation_id = ?",
        )
        .bind(activity_id.to_string())
        .bind(conversation_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(StorageError::NotFound)?;
        let activity = map_activity(row)?;
        let event = insert_event(
            &mut transaction,
            conversation_id,
            activity_id,
            &ServerEvent::AgentActivityUpserted(activity.clone()),
            updated_at,
        )
        .await?;
        transaction.commit().await?;
        Ok((activity, event))
    }

    pub async fn reset_agent_activities(
        &self,
        conversation_id: Uuid,
        updated_at: &str,
    ) -> Result<ServerEventEnvelope, StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query("DELETE FROM agent_activities WHERE conversation_id = ?")
            .bind(conversation_id.to_string())
            .execute(&mut *transaction)
            .await?;
        let event = insert_event(
            &mut transaction,
            conversation_id,
            conversation_id,
            &ServerEvent::AgentActivitiesReset(AgentActivitiesReset::default()),
            updated_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(event)
    }
}

fn map_activity(row: AgentActivityRow) -> Result<AgentActivity, StorageError> {
    Ok(AgentActivity {
        id: Uuid::parse_str(&row.id)?,
        sequence: row.sequence,
        summary: row.summary,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
