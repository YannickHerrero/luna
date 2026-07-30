use std::time::Duration;

use luna_pi::PiBridge;
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use uuid::Uuid;

#[tokio::test]
async fn coordinates_dispatch_markers_over_a_private_socket() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("bridge.sock");
    let bridge = PiBridge::bind(&path, Duration::from_secs(1))
        .await
        .expect("bridge");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    let stream = UnixStream::connect(&path).await.expect("connect");
    let (reader, mut writer) = stream.into_split();
    writer
        .write_all(b"{\"type\":\"ready\",\"pid\":123,\"cwd\":\"/tmp\"}\n")
        .await
        .expect("ready");
    bridge.wait_until_ready().await.expect("ready state");
    let dispatch_id = Uuid::new_v4();
    let peer = tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        let command: serde_json::Value = serde_json::from_str(
            &lines
                .next_line()
                .await
                .expect("command read")
                .expect("command"),
        )
        .expect("command JSON");
        assert_eq!(
            command,
            json!({ "type": "dispatch", "dispatchId": dispatch_id })
        );
        writer
            .write_all(
                format!(
                    "{{\"type\":\"dispatch_ready\",\"dispatchId\":\"{dispatch_id}\"}}\n\
                     {{\"type\":\"dispatch_recorded\",\"dispatchId\":\"{dispatch_id}\"}}\n"
                )
                .as_bytes(),
            )
            .await
            .expect("dispatch events");
    });
    let events = bridge
        .prepare_dispatch(dispatch_id)
        .await
        .expect("prepare dispatch");
    bridge
        .wait_until_recorded(events, dispatch_id)
        .await
        .expect("recorded dispatch");
    peer.await.expect("peer");
    bridge.shutdown().await;
}
