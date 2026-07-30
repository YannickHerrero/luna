use luna_protocol::DevicePlatform;
use luna_storage::{Database, NewDevice, NewPairingCode};
use uuid::Uuid;

#[tokio::test]
async fn authenticates_an_active_device_by_hash() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("luna.sqlite"))
        .await
        .expect("database");
    database
        .create_pairing_code(NewPairingCode {
            id: Uuid::new_v4(),
            code_hash: "pairing",
            created_at: "2026-01-01T00:00:00Z",
            expires_at: "2026-01-02T00:00:00Z",
        })
        .await
        .expect("pairing");
    database
        .redeem_pairing_code(
            "pairing",
            NewDevice {
                id: Uuid::new_v4(),
                name: "Web PWA",
                platform: DevicePlatform::Web,
                credential_hash: "secret-hash",
                created_at: "2026-01-01T00:01:00Z",
            },
        )
        .await
        .expect("redeem");

    let device = database
        .authenticate_device("secret-hash", "2026-01-01T00:02:00Z")
        .await
        .expect("authenticate")
        .expect("device");
    assert_eq!(device.name, "Web PWA");
    assert_eq!(device.last_seen_at, "2026-01-01T00:02:00Z");
}
