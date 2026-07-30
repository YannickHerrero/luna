use luna_protocol::{ServerEvent, ServerEventEnvelope};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, StorageError};

#[derive(FromRow)]
struct EventRow {
    id: i64,
    conversation_id: Option<String>,
    payload: String,
    created_at: String,
}

impl Database {
    pub async fn append_event(
        &self,
        conversation_id: Option<Uuid>,
        aggregate_id: Option<Uuid>,
        event: &ServerEvent,
        created_at: &str,
    ) -> Result<ServerEventEnvelope, StorageError> {
        let payload = serde_json::to_string(event)?;
        let event_type = serde_json::to_value(event)?["type"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        let result = sqlx::query(
            "INSERT INTO sync_events (type, conversation_id, aggregate_id, payload, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(event_type)
        .bind(conversation_id.map(|id| id.to_string()))
        .bind(aggregate_id.map(|id| id.to_string()))
        .bind(payload)
        .bind(created_at)
        .execute(self.pool())
        .await?;
        Ok(ServerEventEnvelope {
            version: 1,
            event_id: Some(result.last_insert_rowid()),
            conversation_id,
            emitted_at: created_at.into(),
            event: event.clone(),
        })
    }

    pub async fn cursor_requires_reset(&self, cursor: i64) -> Result<bool, StorageError> {
        if cursor <= 0 {
            return Ok(false);
        }
        let oldest: Option<i64> = sqlx::query_scalar("SELECT MIN(id) FROM sync_events")
            .fetch_one(self.pool())
            .await?;
        Ok(oldest.is_some_and(|oldest| cursor < oldest.saturating_sub(1)))
    }

    pub async fn prune_events_before(&self, cutoff: &str) -> Result<u64, StorageError> {
        let deleted = sqlx::query(
            "DELETE FROM sync_events WHERE created_at < ? AND id < (SELECT COALESCE(MAX(id), 0) FROM sync_events)",
        )
        .bind(cutoff)
        .execute(self.pool())
        .await?;
        Ok(deleted.rows_affected())
    }

    pub async fn latest_cursor(&self) -> Result<i64, StorageError> {
        let value: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM sync_events")
            .fetch_one(self.pool())
            .await?;
        Ok(value.unwrap_or(0))
    }

    pub async fn events_after(
        &self,
        cursor: i64,
        limit: i64,
    ) -> Result<Vec<ServerEventEnvelope>, StorageError> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT id, conversation_id, payload, created_at FROM sync_events WHERE id > ? ORDER BY id ASC LIMIT ?",
        )
        .bind(cursor)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(ServerEventEnvelope {
                    version: 1,
                    event_id: Some(row.id),
                    conversation_id: row
                        .conversation_id
                        .map(|id| Uuid::parse_str(&id))
                        .transpose()?,
                    emitted_at: row.created_at,
                    event: serde_json::from_str(&row.payload)?,
                })
            })
            .collect()
    }
}
