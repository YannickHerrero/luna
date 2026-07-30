use luna_protocol::DevicePlatform;
use luna_storage::{Database, NewAttachment, NewDevice, NewPairingCode};
use uuid::Uuid;

const NOW: &str = "2026-03-20T12:00:00Z";

#[tokio::test]
async fn stores_private_attachment_metadata() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("attachments.sqlite"))
        .await
        .expect("database");
    let device_id = Uuid::new_v4();
    database
        .create_pairing_code(NewPairingCode {
            id: Uuid::new_v4(),
            code_hash: "pairing",
            created_at: NOW,
            expires_at: "2027-03-20T12:00:00Z",
        })
        .await
        .expect("pairing");
    database
        .redeem_pairing_code(
            "pairing",
            NewDevice {
                id: device_id,
                name: "iPhone",
                platform: DevicePlatform::Ios,
                credential_hash: "hash",
                created_at: NOW,
            },
        )
        .await
        .expect("device");
    let id = Uuid::new_v4();
    let stored = database
        .create_attachment(NewAttachment {
            id,
            conversation_id: None,
            uploaded_by_device_id: device_id,
            storage_key: "originals/file.png",
            thumbnail_storage_key: "thumbnails/file.jpg",
            original_name: "Screenshot.png",
            mime_type: "image/png",
            byte_size: 42,
            sha256: "abc123",
            width: 320,
            height: 240,
            created_at: NOW,
        })
        .await
        .expect("attachment");
    assert_eq!(stored.attachment.id, id);
    assert_eq!(stored.attachment.width, 320);
    assert_eq!(stored.storage_key, "originals/file.png");
    assert_eq!(
        stored.attachment.content_url,
        format!("/v1/attachments/{id}/content")
    );
    let expired = database
        .expired_attachment_files("2027-03-20T12:00:00Z")
        .await
        .expect("expired attachments");
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].id, id);
    database
        .mark_attachment_deleted(id, "2027-03-20T12:00:00Z")
        .await
        .expect("delete attachment");
    assert!(
        database
            .stored_attachment(id)
            .await
            .expect("attachment query")
            .is_none()
    );
}
