use luna_protocol::{DevicePlatform, MessageDelivery, MessageRole, MessageStatus};
use luna_storage::{Database, NewAttachment, NewDevice, NewPairingCode, NewUserMessage};
use tempfile::TempDir;
use uuid::Uuid;

const NOW: &str = "2026-03-20T12:00:00Z";

async fn setup() -> (TempDir, Database, Uuid, Uuid) {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("messages.sqlite"))
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
        .expect("pairing code");
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
    let conversation_id = Uuid::new_v4();
    database
        .create_conversation(conversation_id, "/Users/test", NOW)
        .await
        .expect("conversation");
    (directory, database, device_id, conversation_id)
}

#[tokio::test]
async fn accepts_user_commands_idempotently() {
    let (_directory, database, device_id, conversation_id) = setup().await;
    let client_message_id = Uuid::new_v4();
    let attachment_id = Uuid::new_v4();
    database
        .create_attachment(NewAttachment {
            id: attachment_id,
            conversation_id: None,
            uploaded_by_device_id: device_id,
            storage_key: "originals/image.png",
            thumbnail_storage_key: "thumbnails/image.jpg",
            original_name: "image.png",
            mime_type: "image/png",
            byte_size: 42,
            sha256: "abc123",
            width: 10,
            height: 10,
            created_at: NOW,
        })
        .await
        .expect("attachment");
    let first = database
        .accept_user_message(NewUserMessage {
            conversation_id,
            device_id,
            client_message_id,
            text: "Hello",
            attachment_ids: &[attachment_id],
            delivery: MessageDelivery::Initial,
            accepted_at: NOW,
        })
        .await
        .expect("first dispatch");
    database
        .set_dispatch_state(first.dispatch_id, "running", None, NOW)
        .await
        .expect("running dispatch");
    assert_eq!(
        database
            .recover_inflight_dispatches("2026-03-20T12:01:00Z")
            .await
            .expect("dispatch recovery"),
        1
    );
    let replay = database
        .accept_user_message(NewUserMessage {
            conversation_id,
            device_id,
            client_message_id,
            text: "Hello",
            attachment_ids: &[attachment_id],
            delivery: MessageDelivery::Initial,
            accepted_at: NOW,
        })
        .await
        .expect("replayed dispatch");
    assert!(first.created);
    assert!(!replay.created);
    assert!(replay.dispatch_required);
    assert_eq!(first.dispatch_id, replay.dispatch_id);
    assert_eq!(first.message.id, replay.message.id);
    assert_eq!(first.message.attachments[0].id, attachment_id);
    assert_eq!(replay.message.attachments[0].id, attachment_id);
    assert_eq!(first.events.len(), 2);
    assert!(replay.events.is_empty());
    assert_eq!(
        database
            .conversation(conversation_id)
            .await
            .expect("conversation")
            .expect("present")
            .notification_target_device_id,
        Some(device_id)
    );
}

#[tokio::test]
async fn marks_streaming_messages_interrupted_after_restart() {
    let (_directory, database, _device_id, conversation_id) = setup().await;
    let message_id = Uuid::new_v4();
    database
        .begin_assistant_message(conversation_id, message_id, NOW)
        .await
        .expect("assistant message");
    let recovered = database
        .recover_streaming_messages("2026-03-20T12:01:00Z")
        .await
        .expect("recovery");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, MessageStatus::Interrupted);
}

#[tokio::test]
async fn persists_assistant_deltas_and_completion() {
    let (_directory, database, _device_id, conversation_id) = setup().await;
    let message_id = Uuid::new_v4();
    let (message, _) = database
        .begin_assistant_message(conversation_id, message_id, NOW)
        .await
        .expect("assistant message");
    assert_eq!(message.role, MessageRole::Assistant);
    database
        .append_message_delta(conversation_id, message_id, 0, 0, "Hel", NOW)
        .await
        .expect("first delta");
    database
        .append_message_delta(conversation_id, message_id, 1, 0, "lo", NOW)
        .await
        .expect("second delta");
    database
        .complete_message(conversation_id, message_id, NOW)
        .await
        .expect("complete");
    let messages = database
        .messages(conversation_id, None, 50)
        .await
        .expect("messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text, "Hello");
    assert_eq!(messages[0].status, MessageStatus::Completed);
    assert_eq!(database.latest_cursor().await.expect("cursor"), 4);
}
