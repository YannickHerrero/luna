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

    assert!(tables >= 15);
    assert_eq!(application_id, SQLITE_APPLICATION_ID);
    assert_eq!(foreign_keys, 1);
    database.close().await;
}
