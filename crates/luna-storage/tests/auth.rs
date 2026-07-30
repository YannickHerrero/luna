use luna_protocol::DevicePlatform;
use luna_storage::{Database, NewDevice, NewPairingCode};
use uuid::Uuid;

#[tokio::test]
async fn pairing_codes_are_single_use() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("luna.sqlite"))
        .await
        .expect("database");
    database
        .create_pairing_code(NewPairingCode {
            id: Uuid::new_v4(),
            code_hash: "pairing-hash",
            created_at: "2026-01-01T00:00:00Z",
            expires_at: "2026-01-01T01:00:00Z",
        })
        .await
        .expect("pairing code");

    let first = database
        .redeem_pairing_code(
            "pairing-hash",
            NewDevice {
                id: Uuid::new_v4(),
                name: "iPhone",
                platform: DevicePlatform::Ios,
                credential_hash: "credential-hash",
                created_at: "2026-01-01T00:10:00Z",
            },
        )
        .await
        .expect("redeem");
    let second = database
        .redeem_pairing_code(
            "pairing-hash",
            NewDevice {
                id: Uuid::new_v4(),
                name: "iPad",
                platform: DevicePlatform::Ipados,
                credential_hash: "another-hash",
                created_at: "2026-01-01T00:20:00Z",
            },
        )
        .await
        .expect("second redeem");

    assert_eq!(first.expect("device").name, "iPhone");
    assert!(second.is_none());
}

#[tokio::test]
async fn a_new_pairing_code_invalidates_older_unused_codes() {
    let directory = tempfile::tempdir().expect("temp directory");
    let database = Database::connect(&directory.path().join("luna.sqlite"))
        .await
        .expect("database");
    for (hash, created_at, expires_at) in [
        (
            "older-pairing-hash",
            "2026-01-01T00:00:00Z",
            "2026-01-01T01:00:00Z",
        ),
        (
            "newer-pairing-hash",
            "2026-01-01T00:05:00Z",
            "2026-01-01T01:05:00Z",
        ),
    ] {
        database
            .create_pairing_code(NewPairingCode {
                id: Uuid::new_v4(),
                code_hash: hash,
                created_at,
                expires_at,
            })
            .await
            .expect("pairing code");
    }

    let device = |name: &'static str| NewDevice {
        id: Uuid::new_v4(),
        name,
        platform: DevicePlatform::Web,
        credential_hash: name,
        created_at: "2026-01-01T00:10:00Z",
    };
    assert!(
        database
            .redeem_pairing_code("older-pairing-hash", device("Old browser"))
            .await
            .expect("old redemption")
            .is_none()
    );
    assert_eq!(
        database
            .redeem_pairing_code("newer-pairing-hash", device("New browser"))
            .await
            .expect("new redemption")
            .expect("new device")
            .name,
        "New browser"
    );
}
