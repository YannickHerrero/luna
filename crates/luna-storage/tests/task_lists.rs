use luna_protocol::{AgentTask, AgentTaskList, AgentTaskListChanged, AgentTaskStatus, ServerEvent};
use luna_storage::Database;
use uuid::Uuid;

const NOW: &str = "2026-03-20T12:00:00Z";

#[tokio::test]
async fn replaces_restores_and_clears_conversation_task_lists() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("task-lists.sqlite"))
        .await
        .expect("database");
    let conversation_id = Uuid::new_v4();
    database
        .create_conversation(conversation_id, "/Users/test", NOW)
        .await
        .expect("conversation");

    let initial = task_list(1, AgentTaskStatus::InProgress);
    let changed = database
        .replace_agent_task_list(conversation_id, &initial, NOW)
        .await
        .expect("replace task list");
    assert_eq!(
        changed.event,
        ServerEvent::AgentTaskListChanged(AgentTaskListChanged {
            task_list: Some(initial.clone()),
        })
    );
    assert_eq!(
        database
            .conversation(conversation_id)
            .await
            .expect("conversation query")
            .expect("conversation")
            .task_list,
        Some(initial.clone())
    );

    let replacement = task_list(2, AgentTaskStatus::Completed);
    database
        .replace_agent_task_list(conversation_id, &replacement, "2026-03-20T12:00:01Z")
        .await
        .expect("update task list");
    assert_eq!(
        database
            .agent_task_list(conversation_id)
            .await
            .expect("task list query"),
        Some(replacement)
    );

    let cleared = database
        .clear_agent_task_list(conversation_id, "2026-03-20T12:00:02Z")
        .await
        .expect("clear task list");
    assert_eq!(
        cleared.event,
        ServerEvent::AgentTaskListChanged(AgentTaskListChanged { task_list: None })
    );
    assert!(
        database
            .agent_task_list(conversation_id)
            .await
            .expect("task list query")
            .is_none()
    );
    assert_eq!(database.latest_cursor().await.expect("cursor"), 3);
}

fn task_list(revision: i64, status: AgentTaskStatus) -> AgentTaskList {
    AgentTaskList {
        id: Uuid::new_v4(),
        title: Some("Ship task progress".into()),
        revision,
        tasks: vec![AgentTask {
            id: Uuid::new_v4(),
            sequence: 1,
            text: "Verify persistent progress".into(),
            status,
            note: Some("Storage test".into()),
            created_at: NOW.into(),
            updated_at: NOW.into(),
        }],
        created_at: NOW.into(),
        updated_at: NOW.into(),
    }
}
