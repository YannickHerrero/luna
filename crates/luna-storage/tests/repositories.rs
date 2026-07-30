use luna_storage::{Database, RepositoryObservation};
use uuid::Uuid;

#[tokio::test]
async fn tracks_multiple_repositories_and_the_active_root() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("repositories.sqlite"))
        .await
        .expect("database");
    let conversation_id = Uuid::new_v4();
    database
        .create_conversation(conversation_id, "/Users/test", "2026-03-20T12:00:00Z")
        .await
        .expect("conversation");
    let first = database
        .observe_repository(RepositoryObservation {
            conversation_id,
            canonical_root: "/Users/test/first",
            git_directory: "/Users/test/first/.git",
            display_name: "first",
            branch: Some("main"),
            active: true,
            icon_storage_key: Some("first.png"),
            icon_source: Some("ios_app_icon"),
            icon_fingerprint: Some("first-hash"),
            observed_at: "2026-03-20T12:01:00Z",
        })
        .await
        .expect("first repository");
    assert!(first.changed);
    assert!(first.repositories[0].active);
    assert!(first.repositories[0].icon.content_url.is_some());
    let repeated = database
        .observe_repository(RepositoryObservation {
            conversation_id,
            canonical_root: "/Users/test/first",
            git_directory: "/Users/test/first/.git",
            display_name: "first",
            branch: Some("main"),
            active: true,
            icon_storage_key: Some("first.png"),
            icon_source: Some("ios_app_icon"),
            icon_fingerprint: Some("first-hash"),
            observed_at: "2026-03-20T12:02:00Z",
        })
        .await
        .expect("repeated repository");
    assert!(!repeated.changed);
    let second = database
        .observe_repository(RepositoryObservation {
            conversation_id,
            canonical_root: "/Users/test/second",
            git_directory: "/Users/test/second/.git",
            display_name: "second",
            branch: Some("feature"),
            active: true,
            icon_storage_key: None,
            icon_source: None,
            icon_fingerprint: None,
            observed_at: "2026-03-20T12:03:00Z",
        })
        .await
        .expect("second repository");
    assert!(second.changed);
    assert_eq!(second.repositories.len(), 2);
    assert!(second.repositories[0].active);
    assert_eq!(second.repositories[0].display_name, "second");
    assert!(!second.repositories[1].active);
}
