use std::{path::PathBuf, time::Duration};

use luna_pi::{SessionRuntimeConfig, SessionSupervisor};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the workspace-pinned Pi Node runtime"]
async fn activates_the_workspace_pi_runtime_with_the_bridge() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = tempfile::tempdir().expect("temp directory");
    let bridge_directory = PathBuf::from("/tmp").join(format!("luna-pi-{}", std::process::id()));
    let supervisor = SessionSupervisor::new(SessionRuntimeConfig {
        pi_executable: workspace.join("packages/pi-runtime/node_modules/.bin/pi"),
        bridge_extension: workspace.join("integrations/pi/luna-bridge.ts"),
        session_directory: directory.path().join("sessions"),
        bridge_directory: bridge_directory.clone(),
        request_timeout: Duration::from_secs(10),
    });
    let session = supervisor
        .activate(
            Uuid::new_v4(),
            PathBuf::from(std::env::var_os("HOME").expect("home")),
            None,
        )
        .await
        .expect("active Pi session");
    let state = session.process.get_state().await.expect("Pi state");
    assert_eq!(state.command, "get_state");
    supervisor.shutdown().await;
    tokio::fs::remove_dir_all(bridge_directory)
        .await
        .expect("bridge cleanup");
}
