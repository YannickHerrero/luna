use luna_protocol::{ServerEvent, SessionState};
use luna_storage::Database;
use uuid::Uuid;

#[tokio::test]
async fn prunes_old_events_and_detects_stale_cursors() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("events.sqlite"))
        .await
        .expect("database");
    let conversation_id = Uuid::new_v4();
    database
        .create_conversation(conversation_id, "/Users/test", "2026-01-01T00:00:00Z")
        .await
        .expect("conversation");
    for (state, timestamp) in [
        (SessionState::Starting, "2026-01-01T00:00:00Z"),
        (SessionState::Working, "2026-01-02T00:00:00Z"),
        (SessionState::Idle, "2026-02-01T00:00:00Z"),
    ] {
        database
            .append_event(
                Some(conversation_id),
                Some(conversation_id),
                &ServerEvent::SessionStateChanged { state },
                timestamp,
            )
            .await
            .expect("event");
    }
    assert_eq!(
        database
            .prune_events_before("2026-01-15T00:00:00Z")
            .await
            .expect("prune"),
        2
    );
    assert!(
        database
            .cursor_requires_reset(1)
            .await
            .expect("stale cursor")
    );
    assert!(
        !database
            .cursor_requires_reset(2)
            .await
            .expect("contiguous cursor")
    );
    assert_eq!(database.events_after(0, 10).await.expect("events").len(), 1);
}
