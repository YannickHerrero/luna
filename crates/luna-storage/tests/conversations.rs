use luna_protocol::{ServerEvent, SessionState, TitleMode};
use luna_storage::Database;
use uuid::Uuid;

#[tokio::test]
async fn conversations_and_sync_events_round_trip() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("luna.sqlite"))
        .await
        .expect("database");
    let id = Uuid::new_v4();
    let conversation = database
        .create_conversation(id, "/Users/test", "2026-01-01T00:00:00Z")
        .await
        .expect("conversation");
    assert_eq!(conversation.title_mode, TitleMode::Automatic);
    assert_eq!(conversation.state, SessionState::Creating);

    let automatic = database
        .set_automatic_title(id, "Authentication", "2026-01-01T00:00:30Z")
        .await
        .expect("automatic title")
        .expect("updated conversation");
    assert_eq!(automatic.title, "Authentication");
    let evolved = database
        .set_automatic_title(
            id,
            "Authentication + Password Reset",
            "2026-01-01T00:00:45Z",
        )
        .await
        .expect("evolved title")
        .expect("updated conversation");
    assert_eq!(evolved.title, "Authentication + Password Reset");

    let renamed = database
        .rename_conversation(id, "Rust server", "2026-01-01T00:01:00Z")
        .await
        .expect("rename");
    assert_eq!(renamed.title, "Rust server");
    assert_eq!(renamed.title_mode, TitleMode::Manual);
    assert!(
        database
            .set_automatic_title(id, "Overwritten", "2026-01-01T00:02:00Z")
            .await
            .expect("protected title")
            .is_none()
    );

    let envelope = database
        .append_event(
            Some(id),
            Some(id),
            &ServerEvent::ConversationUpserted(renamed),
            "2026-01-01T00:01:00Z",
        )
        .await
        .expect("event");
    assert_eq!(envelope.event_id, Some(1));
    assert_eq!(
        database.events_after(0, 100).await.expect("replay").len(),
        1
    );
}

#[tokio::test]
async fn marks_interrupted_runtime_states_as_crashed() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("recovery.sqlite"))
        .await
        .expect("database");
    let id = Uuid::new_v4();
    database
        .create_conversation(id, "/Users/test", "2026-01-01T00:00:00Z")
        .await
        .expect("conversation");
    database
        .set_conversation_state(id, SessionState::Working, "2026-01-01T00:01:00Z")
        .await
        .expect("working state");
    assert_eq!(
        database
            .recover_interrupted_conversations("2026-01-01T00:02:00Z")
            .await
            .expect("recovery"),
        vec![id]
    );
    assert_eq!(
        database
            .conversation(id)
            .await
            .expect("conversation")
            .expect("present")
            .state,
        SessionState::Crashed
    );
}
