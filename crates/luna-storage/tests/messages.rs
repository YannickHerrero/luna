use luna_protocol::{DevicePlatform, MessageDelivery, MessageRole, MessageStatus};
use luna_storage::{Database, NewDevice, NewPairingCode};
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
    let first = database
        .accept_user_message(
            conversation_id,
            device_id,
            client_message_id,
            "Hello",
            MessageDelivery::Initial,
            NOW,
        )
        .await
        .expect("first dispatch");
    let replay = database
        .accept_user_message(
            conversation_id,
            device_id,
            client_message_id,
            "Hello",
            MessageDelivery::Initial,
            NOW,
        )
        .await
        .expect("replayed dispatch");
    assert!(first.created);
    assert!(!replay.created);
    assert_eq!(first.dispatch_id, replay.dispatch_id);
    assert_eq!(first.message.id, replay.message.id);
    assert!(first.event.is_some());
    assert!(replay.event.is_none());
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
