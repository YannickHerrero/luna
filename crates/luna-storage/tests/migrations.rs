use luna_storage::{Database, SQLITE_APPLICATION_ID};
use sqlx::Row;

#[tokio::test]
async fn migrates_a_new_database_with_required_pragmas() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("luna.sqlite"))
        .await
        .expect("database");

    let tables: i64 =
        sqlx::query("SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table'")
            .fetch_one(database.pool())
            .await
            .expect("table count")
            .get("count");
    let agent_cycles: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'agent_cycles'",
    )
    .fetch_one(database.pool())
    .await
    .expect("agent cycle table")
    .get("count");
    let dispatch_cycle_columns: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM pragma_table_info('dispatches') WHERE name = 'cycle_id'",
    )
    .fetch_one(database.pool())
    .await
    .expect("dispatch cycle column")
    .get("count");
    let delivery_attempt_columns: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM pragma_table_info('notification_deliveries') WHERE name IN ('cycle_id', 'attempts')",
    )
    .fetch_one(database.pool())
    .await
    .expect("notification delivery columns")
    .get("count");
    let notification_indexes: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'index' AND name IN ('agent_cycles_active_conversation_unique', 'notification_delivery_cycle_unique', 'apns_device_environment_topic_unique')",
    )
    .fetch_one(database.pool())
    .await
    .expect("notification indexes")
    .get("count");
    let application_id: i32 = sqlx::query("PRAGMA application_id")
        .fetch_one(database.pool())
        .await
        .expect("application id")
        .get(0);
    let foreign_keys: i32 = sqlx::query("PRAGMA foreign_keys")
        .fetch_one(database.pool())
        .await
        .expect("foreign keys")
        .get(0);

    assert!(tables >= 16);
    assert_eq!(agent_cycles, 1);
    assert_eq!(dispatch_cycle_columns, 1);
    assert_eq!(delivery_attempt_columns, 2);
    assert_eq!(notification_indexes, 3);
    assert_eq!(application_id, SQLITE_APPLICATION_ID);
    assert_eq!(foreign_keys, 1);
    database.close().await;
}
