use luna_protocol::ServerEvent;
use luna_storage::Database;
use uuid::Uuid;

const NOW: &str = "2026-03-20T12:00:00Z";

#[tokio::test]
async fn persists_and_resets_active_progress_summaries() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("activities.sqlite"))
        .await
        .expect("database");
    let conversation_id = Uuid::new_v4();
    let activity_id = Uuid::new_v4();
    database
        .create_conversation(conversation_id, "/Users/test", NOW)
        .await
        .expect("conversation");

    let (_, started) = database
        .upsert_agent_activity(conversation_id, activity_id, 0, "Planning Luna", NOW)
        .await
        .expect("started activity");
    assert!(matches!(
        started.event,
        ServerEvent::AgentActivityUpserted(_)
    ));
    database
        .upsert_agent_activity(
            conversation_id,
            activity_id,
            0,
            "Planning Luna deployment",
            "2026-03-20T12:00:01Z",
        )
        .await
        .expect("updated activity");

    let conversation = database
        .conversation(conversation_id)
        .await
        .expect("conversation query")
        .expect("conversation");
    assert_eq!(conversation.activities.len(), 1);
    assert_eq!(
        conversation.activities[0].summary,
        "Planning Luna deployment"
    );

    let reset = database
        .reset_agent_activities(conversation_id, "2026-03-20T12:00:02Z")
        .await
        .expect("reset activities");
    assert!(matches!(reset.event, ServerEvent::AgentActivitiesReset(_)));
    assert!(
        database
            .conversation(conversation_id)
            .await
            .expect("conversation query")
            .expect("conversation")
            .activities
            .is_empty()
    );
    assert_eq!(database.latest_cursor().await.expect("cursor"), 3);
}
