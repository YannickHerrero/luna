use luna_protocol::{Attachment, AttachmentStatus};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, StorageError};

pub struct NewAttachment<'a> {
    pub id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub uploaded_by_device_id: Uuid,
    pub storage_key: &'a str,
    pub thumbnail_storage_key: &'a str,
    pub original_name: &'a str,
    pub mime_type: &'a str,
    pub byte_size: i64,
    pub sha256: &'a str,
    pub width: u32,
    pub height: u32,
    pub created_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct StoredAttachment {
    pub attachment: Attachment,
    pub conversation_id: Option<Uuid>,
    pub uploaded_by_device_id: Uuid,
    pub storage_key: String,
    pub thumbnail_storage_key: String,
    pub sha256: String,
}

#[derive(FromRow)]
pub(crate) struct AttachmentRow {
    id: String,
    conversation_id: Option<String>,
    uploaded_by_device_id: String,
    storage_key: String,
    thumbnail_storage_key: String,
    original_name: String,
    mime_type: String,
    byte_size: i64,
    sha256: String,
    width: i64,
    height: i64,
    status: String,
    created_at: String,
}

impl Database {
    pub async fn create_attachment(
        &self,
        attachment: NewAttachment<'_>,
    ) -> Result<StoredAttachment, StorageError> {
        sqlx::query(
            "INSERT INTO attachments (id, conversation_id, uploaded_by_device_id, storage_key, thumbnail_storage_key, original_name, mime_type, byte_size, sha256, width, height, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ready', ?)",
        )
        .bind(attachment.id.to_string())
        .bind(attachment.conversation_id.map(|id| id.to_string()))
        .bind(attachment.uploaded_by_device_id.to_string())
        .bind(attachment.storage_key)
        .bind(attachment.thumbnail_storage_key)
        .bind(attachment.original_name)
        .bind(attachment.mime_type)
        .bind(attachment.byte_size)
        .bind(attachment.sha256)
        .bind(i64::from(attachment.width))
        .bind(i64::from(attachment.height))
        .bind(attachment.created_at)
        .execute(self.pool())
        .await?;
        self.stored_attachment(attachment.id)
            .await?
            .ok_or(StorageError::NotFound)
    }

    pub async fn stored_attachment(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAttachment>, StorageError> {
        let row = sqlx::query_as::<_, AttachmentRow>(
            "SELECT id, conversation_id, uploaded_by_device_id, storage_key, thumbnail_storage_key, original_name, mime_type, byte_size, sha256, width, height, status, created_at FROM attachments WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(self.pool())
        .await?;
        row.map(map_stored_attachment).transpose()
    }

    pub async fn attachments_for_message(
        &self,
        message_id: Uuid,
    ) -> Result<Vec<Attachment>, StorageError> {
        let rows = sqlx::query_as::<_, AttachmentRow>(
            "SELECT a.id, a.conversation_id, a.uploaded_by_device_id, a.storage_key, a.thumbnail_storage_key, a.original_name, a.mime_type, a.byte_size, a.sha256, a.width, a.height, a.status, a.created_at FROM message_attachments ma JOIN attachments a ON a.id = ma.attachment_id WHERE ma.message_id = ? AND a.deleted_at IS NULL ORDER BY ma.position ASC",
        )
        .bind(message_id.to_string())
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|row| map_stored_attachment(row).map(|stored| stored.attachment))
            .collect()
    }
}

pub(crate) fn map_stored_attachment(row: AttachmentRow) -> Result<StoredAttachment, StorageError> {
    let id = Uuid::parse_str(&row.id)?;
    Ok(StoredAttachment {
        attachment: Attachment {
            id,
            file_name: row.original_name,
            mime_type: row.mime_type,
            byte_size: row.byte_size,
            width: u32::try_from(row.width)
                .map_err(|_| StorageError::InvalidEnum("attachment width".into()))?,
            height: u32::try_from(row.height)
                .map_err(|_| StorageError::InvalidEnum("attachment height".into()))?,
            status: parse_status(&row.status)?,
            content_url: format!("/v1/attachments/{id}/content"),
            thumbnail_url: format!("/v1/attachments/{id}/thumbnail"),
            created_at: row.created_at,
        },
        conversation_id: row
            .conversation_id
            .map(|id| Uuid::parse_str(&id))
            .transpose()?,
        uploaded_by_device_id: Uuid::parse_str(&row.uploaded_by_device_id)?,
        storage_key: row.storage_key,
        thumbnail_storage_key: row.thumbnail_storage_key,
        sha256: row.sha256,
    })
}

fn parse_status(value: &str) -> Result<AttachmentStatus, StorageError> {
    match value {
        "uploading" => Ok(AttachmentStatus::Uploading),
        "ready" => Ok(AttachmentStatus::Ready),
        "attached" => Ok(AttachmentStatus::Attached),
        "failed" => Ok(AttachmentStatus::Failed),
        "deleted" => Ok(AttachmentStatus::Deleted),
        value => Err(StorageError::InvalidEnum(value.into())),
    }
}
