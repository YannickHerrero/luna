use std::{collections::HashMap, fs, path::Path, time::Duration};

use luna_pi::{NormalizedPiEvent, PiProcess, PiProcessConfig};

fn write_fake_pi(path: &Path) {
    fs::write(
        path,
        r#"#!/usr/bin/env node
let buffer = ''
process.stdin.on('data', chunk => {
  buffer += chunk.toString('utf8')
  while (buffer.includes('\n')) {
    const index = buffer.indexOf('\n')
    const line = buffer.slice(0, index)
    buffer = buffer.slice(index + 1)
    const request = JSON.parse(line)
    if (request.type === 'get_state') {
      console.log(JSON.stringify({id: request.id, type:'response', command:'get_state', success:true, data:{isStreaming:false, sessionId:'fake'}}))
    } else if (request.type === 'prompt') {
      console.log(JSON.stringify({id: request.id, type:'response', command:'prompt', success:true}))
      console.log(JSON.stringify({type:'message_update', message:{role:'assistant'}, assistantMessageEvent:{type:'text_delta', contentIndex:0, delta:'Hello'}}))
      console.log(JSON.stringify({type:'agent_settled'}))
    }
  }
})
"#,
    )
    .expect("fake Pi");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("permissions");
    }
}

#[tokio::test]
async fn controls_a_structured_rpc_process() {
    let directory = tempfile::tempdir().expect("temp directory");
    let executable = directory.path().join("fake-pi");
    write_fake_pi(&executable);
    let process = PiProcess::spawn(PiProcessConfig {
        executable,
        working_directory: directory.path().into(),
        session_directory: directory.path().join("sessions"),
        session_path: None,
        extension_path: None,
        environment: HashMap::new(),
        request_timeout: Duration::from_secs(2),
    })
    .await
    .expect("Pi process");
    let mut events = process.subscribe();
    process.prompt("Hello", &[], false).await.expect("prompt");
    let first = events.recv().await.expect("text event");
    assert_eq!(
        first.normalized,
        NormalizedPiEvent::TextDelta {
            content_index: 0,
            delta: "Hello".into(),
        }
    );
    assert_eq!(
        events.recv().await.expect("settled").normalized,
        NormalizedPiEvent::AgentSettled
    );
    process.shutdown().await;
}
