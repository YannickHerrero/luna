use luna_protocol::{ConversationScope, ServerEvent, SessionState, TitleMode};
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
    assert_eq!(conversation.last_message_at, None);

    let automatic = database
        .set_automatic_title(id, "Authentication", "2026-01-01T00:00:30Z")
        .await
        .expect("automatic title")
        .expect("updated conversation");
    assert_eq!(automatic.title, "Authentication");
    assert!(
        database
            .set_automatic_title(
                id,
                "Authentication + Password Reset",
                "2026-01-01T00:00:45Z",
            )
            .await
            .expect("one-time automatic title")
            .is_none()
    );

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
async fn lists_and_restores_archived_conversations() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("archives.sqlite"))
        .await
        .expect("database");
    let active_id = Uuid::new_v4();
    let archived_id = Uuid::new_v4();
    database
        .create_conversation(active_id, "/Users/test", "2026-01-01T00:00:00Z")
        .await
        .expect("active conversation");
    database
        .create_conversation(archived_id, "/Users/test", "2026-01-02T00:00:00Z")
        .await
        .expect("archived conversation");
    database
        .archive_conversation(archived_id, "2026-01-03T00:00:00Z")
        .await
        .expect("archive");

    let active = database
        .conversations(ConversationScope::Active)
        .await
        .expect("active list");
    assert_eq!(
        active.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![active_id]
    );
    let archived = database
        .conversations(ConversationScope::Archived)
        .await
        .expect("archived list");
    assert_eq!(
        archived.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![archived_id]
    );
    assert_eq!(
        database
            .conversations(ConversationScope::All)
            .await
            .expect("complete list")
            .len(),
        2
    );

    let restored = database
        .restore_conversation(archived_id, "2026-01-04T00:00:00Z")
        .await
        .expect("restore");
    assert_eq!(restored.archived_at, None);
    assert_eq!(restored.state, SessionState::Stopped);
    assert_eq!(
        database
            .conversations(ConversationScope::Active)
            .await
            .expect("restored active list")
            .len(),
        2
    );
    assert!(
        database
            .conversations(ConversationScope::Archived)
            .await
            .expect("empty archive")
            .is_empty()
    );
}

#[tokio::test]
async fn orders_conversations_by_their_latest_message() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("ordering.sqlite"))
        .await
        .expect("database");
    let older_conversation = Uuid::new_v4();
    let newer_empty_conversation = Uuid::new_v4();
    database
        .create_conversation(older_conversation, "/Users/test", "2026-01-01T00:00:00Z")
        .await
        .expect("older conversation");
    database
        .create_conversation(
            newer_empty_conversation,
            "/Users/test",
            "2026-01-02T00:00:00Z",
        )
        .await
        .expect("newer conversation");
    assert_eq!(
        database
            .conversations(ConversationScope::Active)
            .await
            .expect("initial ordering")
            .iter()
            .map(|conversation| conversation.id)
            .collect::<Vec<_>>(),
        vec![newer_empty_conversation, older_conversation]
    );

    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, status, text, ordinal, created_at, updated_at) VALUES (?, ?, 'user', 'completed', 'Latest work', 1, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(older_conversation.to_string())
    .bind("2026-01-03T00:00:00Z")
    .bind("2026-01-03T00:00:00Z")
    .execute(database.pool())
    .await
    .expect("message");
    database
        .set_conversation_state(
            newer_empty_conversation,
            SessionState::Idle,
            "2026-01-04T00:00:00Z",
        )
        .await
        .expect("state update");

    let conversations = database
        .conversations(ConversationScope::Active)
        .await
        .expect("message ordering");
    assert_eq!(conversations[0].id, older_conversation);
    assert_eq!(
        conversations[0].last_message_at.as_deref(),
        Some("2026-01-03T00:00:00Z")
    );
    assert_eq!(conversations[1].id, newer_empty_conversation);
    assert_eq!(conversations[1].last_message_at, None);
}

#[tokio::test]
async fn marks_active_runtime_states_interrupted_for_graceful_shutdown() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("shutdown.sqlite"))
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
            .interrupt_active_conversations("2026-01-01T00:02:00Z")
            .await
            .expect("graceful interruption"),
        vec![id]
    );
    assert_eq!(
        database
            .conversation(id)
            .await
            .expect("conversation")
            .expect("present")
            .state,
        SessionState::Interrupted
    );
    assert!(
        database
            .recover_interrupted_conversations("2026-01-01T00:03:00Z")
            .await
            .expect("restart recovery")
            .is_empty()
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
