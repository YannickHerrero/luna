use luna_protocol::{
    Message, MessageDelivery, MessageRole, MessageStatus, ServerEvent, ServerEventEnvelope,
};
use sqlx::{FromRow, Sqlite, Transaction};
use uuid::Uuid;

use crate::{
    Database, StorageError,
    attachments::{AttachmentRow, map_stored_attachment},
};

pub struct NewUserMessage<'a> {
    pub conversation_id: Uuid,
    pub device_id: Uuid,
    pub client_message_id: Uuid,
    pub text: &'a str,
    pub attachment_ids: &'a [Uuid],
    pub delivery: MessageDelivery,
    pub accepted_at: &'a str,
}

#[derive(Debug)]
pub struct AcceptedDispatch {
    pub dispatch_id: Uuid,
    pub message: Message,
    pub created: bool,
    pub dispatch_required: bool,
    pub event: Option<ServerEventEnvelope>,
}

#[derive(FromRow)]
struct MessageRow {
    id: String,
    conversation_id: String,
    client_message_id: Option<String>,
    role: String,
    status: String,
    delivery: Option<String>,
    text: String,
    sent_by_device_id: Option<String>,
    ordinal: i64,
    created_at: String,
    updated_at: String,
}

impl Database {
    pub async fn accept_user_message(
        &self,
        message: NewUserMessage<'_>,
    ) -> Result<AcceptedDispatch, StorageError> {
        let NewUserMessage {
            conversation_id,
            device_id,
            client_message_id,
            text,
            attachment_ids,
            delivery,
            accepted_at,
        } = message;
        let mut transaction = self.pool().begin().await?;
        if let Some(row) = sqlx::query_as::<_, MessageRow>(
            "SELECT id, conversation_id, client_message_id, role, status, delivery, text, sent_by_device_id, ordinal, created_at, updated_at FROM messages WHERE sent_by_device_id = ? AND client_message_id = ?",
        )
        .bind(device_id.to_string())
        .bind(client_message_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?
        {
            let mut message = map_message(row)?;
            message.attachments =
                attachments_for_message_in_transaction(&mut transaction, message.id).await?;
            let (dispatch_id, state): (String, String) = sqlx::query_as(
                "SELECT id, state FROM dispatches WHERE message_id = ?",
            )
            .bind(message.id.to_string())
            .fetch_one(&mut *transaction)
            .await?;
            transaction.commit().await?;
            return Ok(AcceptedDispatch {
                dispatch_id: Uuid::parse_str(&dispatch_id)?,
                message,
                created: false,
                dispatch_required: matches!(state.as_str(), "accepted" | "failed"),
                event: None,
            });
        }
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ? AND archived_at IS NULL)",
        )
        .bind(conversation_id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        if !active {
            return Err(StorageError::NotFound);
        }
        let mut attachments = Vec::with_capacity(attachment_ids.len());
        for attachment_id in attachment_ids {
            let row = sqlx::query_as::<_, AttachmentRow>(
                "SELECT id, conversation_id, uploaded_by_device_id, storage_key, thumbnail_storage_key, original_name, mime_type, byte_size, sha256, width, height, status, created_at FROM attachments WHERE id = ? AND deleted_at IS NULL AND status = 'ready'",
            )
            .bind(attachment_id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(StorageError::NotFound)?;
            let stored = map_stored_attachment(row)?;
            if stored
                .conversation_id
                .is_some_and(|id| id != conversation_id)
            {
                return Err(StorageError::Conflict);
            }
            attachments.push(stored.attachment);
        }
        let ordinal: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM messages WHERE conversation_id = ?",
        )
        .bind(conversation_id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        let message_id = Uuid::now_v7();
        let dispatch_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, client_message_id, role, status, delivery, text, sent_by_device_id, ordinal, created_at, updated_at) VALUES (?, ?, ?, 'user', 'accepted', ?, ?, ?, ?, ?, ?)",
        )
        .bind(message_id.to_string())
        .bind(conversation_id.to_string())
        .bind(client_message_id.to_string())
        .bind(delivery_name(delivery))
        .bind(text)
        .bind(device_id.to_string())
        .bind(ordinal)
        .bind(accepted_at)
        .bind(accepted_at)
        .execute(&mut *transaction)
        .await?;
        for (position, attachment) in attachments.iter_mut().enumerate() {
            sqlx::query(
                "INSERT INTO message_attachments (message_id, attachment_id, position) VALUES (?, ?, ?)",
            )
            .bind(message_id.to_string())
            .bind(attachment.id.to_string())
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE attachments SET conversation_id = COALESCE(conversation_id, ?), status = 'attached' WHERE id = ?",
            )
            .bind(conversation_id.to_string())
            .bind(attachment.id.to_string())
            .execute(&mut *transaction)
            .await?;
            attachment.status = luna_protocol::AttachmentStatus::Attached;
        }
        sqlx::query(
            "INSERT INTO dispatches (id, message_id, worker_command_id, state, attempts, created_at, updated_at) VALUES (?, ?, ?, 'accepted', 0, ?, ?)",
        )
        .bind(dispatch_id.to_string())
        .bind(message_id.to_string())
        .bind(dispatch_id.to_string())
        .bind(accepted_at)
        .bind(accepted_at)
        .execute(&mut *transaction)
        .await?;
        let message = Message {
            id: message_id,
            conversation_id,
            client_message_id: Some(client_message_id),
            role: MessageRole::User,
            status: MessageStatus::Accepted,
            delivery: Some(delivery),
            text: text.into(),
            attachments,
            sent_by_device_id: Some(device_id),
            ordinal,
            created_at: accepted_at.into(),
            updated_at: accepted_at.into(),
        };
        let event = insert_event(
            &mut transaction,
            conversation_id,
            message_id,
            &ServerEvent::MessageUpserted(message.clone()),
            accepted_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(AcceptedDispatch {
            dispatch_id,
            message,
            created: true,
            dispatch_required: true,
            event: Some(event),
        })
    }

    pub async fn set_dispatch_state(
        &self,
        dispatch_id: Uuid,
        state: &str,
        error_code: Option<&str>,
        updated_at: &str,
    ) -> Result<(), StorageError> {
        let updated = sqlx::query(
            "UPDATE dispatches SET state = ?, attempts = attempts + CASE WHEN ? = 'running' THEN 1 ELSE 0 END, error_code = ?, updated_at = ? WHERE id = ?",
        )
        .bind(state)
        .bind(state)
        .bind(error_code)
        .bind(updated_at)
        .bind(dispatch_id.to_string())
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub async fn begin_assistant_message(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        created_at: &str,
    ) -> Result<(Message, ServerEventEnvelope), StorageError> {
        let mut transaction = self.pool().begin().await?;
        let ordinal: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM messages WHERE conversation_id = ?",
        )
        .bind(conversation_id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, status, text, ordinal, created_at, updated_at) VALUES (?, ?, 'assistant', 'streaming', '', ?, ?, ?)",
        )
        .bind(message_id.to_string())
        .bind(conversation_id.to_string())
        .bind(ordinal)
        .bind(created_at)
        .bind(created_at)
        .execute(&mut *transaction)
        .await?;
        let message = Message {
            id: message_id,
            conversation_id,
            client_message_id: None,
            role: MessageRole::Assistant,
            status: MessageStatus::Streaming,
            delivery: None,
            text: String::new(),
            attachments: vec![],
            sent_by_device_id: None,
            ordinal,
            created_at: created_at.into(),
            updated_at: created_at.into(),
        };
        let event = insert_event(
            &mut transaction,
            conversation_id,
            message_id,
            &ServerEvent::MessageUpserted(message.clone()),
            created_at,
        )
        .await?;
        transaction.commit().await?;
        Ok((message, event))
    }

    pub async fn append_message_delta(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        chunk_index: i64,
        content_index: i64,
        delta: &str,
        created_at: &str,
    ) -> Result<ServerEventEnvelope, StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO message_chunks (message_id, chunk_index, content_index, delta, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(message_id.to_string())
        .bind(chunk_index)
        .bind(content_index)
        .bind(delta)
        .bind(created_at)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("UPDATE messages SET text = text || ?, status = 'streaming', updated_at = ? WHERE id = ?")
            .bind(delta)
            .bind(created_at)
            .bind(message_id.to_string())
            .execute(&mut *transaction)
            .await?;
        let event = insert_event(
            &mut transaction,
            conversation_id,
            message_id,
            &ServerEvent::MessageDelta(luna_protocol::MessageDelta {
                message_id,
                chunk_index,
                delta: delta.into(),
            }),
            created_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(event)
    }

    pub async fn complete_message(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        completed_at: &str,
    ) -> Result<ServerEventEnvelope, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let updated =
            sqlx::query("UPDATE messages SET status = 'completed', updated_at = ? WHERE id = ?")
                .bind(completed_at)
                .bind(message_id.to_string())
                .execute(&mut *transaction)
                .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        let event = insert_event(
            &mut transaction,
            conversation_id,
            message_id,
            &ServerEvent::MessageCompleted(luna_protocol::MessageCompleted { message_id }),
            completed_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(event)
    }

    pub async fn messages(
        &self,
        conversation_id: Uuid,
        before_ordinal: Option<i64>,
        limit: i64,
    ) -> Result<Vec<Message>, StorageError> {
        let rows = match before_ordinal {
            Some(before) => {
                sqlx::query_as::<_, MessageRow>(
                    "SELECT id, conversation_id, client_message_id, role, status, delivery, text, sent_by_device_id, ordinal, created_at, updated_at FROM messages WHERE conversation_id = ? AND ordinal < ? ORDER BY ordinal DESC LIMIT ?",
                )
                .bind(conversation_id.to_string())
                .bind(before)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
            }
            None => {
                sqlx::query_as::<_, MessageRow>(
                    "SELECT id, conversation_id, client_message_id, role, status, delivery, text, sent_by_device_id, ordinal, created_at, updated_at FROM messages WHERE conversation_id = ? ORDER BY ordinal DESC LIMIT ?",
                )
                .bind(conversation_id.to_string())
                .bind(limit)
                .fetch_all(self.pool())
                .await?
            }
        };
        let mut messages = rows
            .into_iter()
            .map(map_message)
            .collect::<Result<Vec<_>, _>>()?;
        messages.reverse();
        for message in &mut messages {
            message.attachments = self.attachments_for_message(message.id).await?;
        }
        Ok(messages)
    }
}

async fn attachments_for_message_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    message_id: Uuid,
) -> Result<Vec<luna_protocol::Attachment>, StorageError> {
    let rows = sqlx::query_as::<_, AttachmentRow>(
        "SELECT a.id, a.conversation_id, a.uploaded_by_device_id, a.storage_key, a.thumbnail_storage_key, a.original_name, a.mime_type, a.byte_size, a.sha256, a.width, a.height, a.status, a.created_at FROM message_attachments ma JOIN attachments a ON a.id = ma.attachment_id WHERE ma.message_id = ? AND a.deleted_at IS NULL ORDER BY ma.position ASC",
    )
    .bind(message_id.to_string())
    .fetch_all(&mut **transaction)
    .await?;
    rows.into_iter()
        .map(|row| map_stored_attachment(row).map(|stored| stored.attachment))
        .collect()
}

async fn insert_event(
    transaction: &mut Transaction<'_, Sqlite>,
    conversation_id: Uuid,
    aggregate_id: Uuid,
    event: &ServerEvent,
    created_at: &str,
) -> Result<ServerEventEnvelope, StorageError> {
    let event_type = serde_json::to_value(event)?["type"]
        .as_str()
        .unwrap_or("unknown")
        .to_owned();
    let result = sqlx::query(
        "INSERT INTO sync_events (type, conversation_id, aggregate_id, payload, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(event_type)
    .bind(conversation_id.to_string())
    .bind(aggregate_id.to_string())
    .bind(serde_json::to_string(event)?)
    .bind(created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(ServerEventEnvelope {
        version: 1,
        event_id: Some(result.last_insert_rowid()),
        conversation_id: Some(conversation_id),
        emitted_at: created_at.into(),
        event: event.clone(),
    })
}

fn map_message(row: MessageRow) -> Result<Message, StorageError> {
    Ok(Message {
        id: Uuid::parse_str(&row.id)?,
        conversation_id: Uuid::parse_str(&row.conversation_id)?,
        client_message_id: row
            .client_message_id
            .map(|id| Uuid::parse_str(&id))
            .transpose()?,
        role: match row.role.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            value => return Err(StorageError::InvalidEnum(value.into())),
        },
        status: parse_status(&row.status)?,
        delivery: row
            .delivery
            .map(|value| parse_delivery(&value))
            .transpose()?,
        text: row.text,
        attachments: vec![],
        sent_by_device_id: row
            .sent_by_device_id
            .map(|id| Uuid::parse_str(&id))
            .transpose()?,
        ordinal: row.ordinal,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn parse_status(value: &str) -> Result<MessageStatus, StorageError> {
    match value {
        "pending" => Ok(MessageStatus::Pending),
        "accepted" => Ok(MessageStatus::Accepted),
        "queued" => Ok(MessageStatus::Queued),
        "streaming" => Ok(MessageStatus::Streaming),
        "completed" => Ok(MessageStatus::Completed),
        "interrupted" => Ok(MessageStatus::Interrupted),
        "failed" => Ok(MessageStatus::Failed),
        value => Err(StorageError::InvalidEnum(value.into())),
    }
}

fn parse_delivery(value: &str) -> Result<MessageDelivery, StorageError> {
    match value {
        "initial" => Ok(MessageDelivery::Initial),
        "steer" => Ok(MessageDelivery::Steer),
        value => Err(StorageError::InvalidEnum(value.into())),
    }
}

fn delivery_name(value: MessageDelivery) -> &'static str {
    match value {
        MessageDelivery::Initial => "initial",
        MessageDelivery::Steer => "steer",
    }
}
