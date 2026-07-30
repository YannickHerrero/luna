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

    let renamed = database
        .rename_conversation(id, "Rust server", "2026-01-01T00:01:00Z")
        .await
        .expect("rename");
    assert_eq!(renamed.title, "Rust server");
    assert_eq!(renamed.title_mode, TitleMode::Manual);

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
