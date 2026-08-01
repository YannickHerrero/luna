use std::{collections::HashMap, fs, path::Path, time::Duration};

use luna_pi::{NormalizedPiEvent, PiProcess, PiProcessConfig, RpcDelivery};
use serde_json::json;

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("permissions");
    }
}

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
      console.log(JSON.stringify({id: request.id, type:'response', command:'get_state', success:true, data:{isStreaming:false, sessionId:'fake', model:{provider:'openai-codex',id:'gpt-5.6-sol'}, thinkingLevel:'high'}}))
    } else if (request.type === 'get_available_models') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true, data:{models:[{provider:'openai-codex',id:'gpt-5.6-sol'}]}}))
    } else if (request.type === 'get_session_stats') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true, data:{contextUsage:{tokens:12000,contextWindow:400000,percent:3}}}))
    } else if (request.type === 'set_model') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true, data:{provider:request.provider,id:request.modelId}}))
    } else if (request.type === 'set_thinking_level') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true}))
    } else if (request.type === 'compact') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true, data:{tokensBefore:12000,estimatedTokensAfter:4000}}))
    } else if (request.type === 'bash') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true, data:{output:'file.txt',exitCode:0,cancelled:false,truncated:false}}))
    } else if (request.type === 'abort' || request.type === 'abort_bash' || request.type === 'abort_retry') {
      console.log(JSON.stringify({id: request.id, type:'response', command:request.type, success:true}))
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
    make_executable(path);
}

fn write_failing_pi(path: &Path) {
    fs::write(
        path,
        r#"#!/usr/bin/env node
const fs = require('node:fs')
const failOn = process.env.FAKE_PI_FAIL_ON
const stoppedPath = process.env.FAKE_PI_STOPPED_PATH
let buffer = ''
process.stdin.on('data', chunk => {
  buffer += chunk.toString('utf8')
  while (buffer.includes('\n')) {
    const index = buffer.indexOf('\n')
    const request = JSON.parse(buffer.slice(0, index))
    buffer = buffer.slice(index + 1)
    if (request.type === failOn) {
      console.log('{')
    } else if (request.type === 'get_state') {
      console.log(JSON.stringify({id: request.id, type:'response', command:'get_state', success:true, data:{isStreaming:false, sessionId:'fake'}}))
    }
  }
})
process.stdin.on('end', () => {
  fs.writeFileSync(stoppedPath, 'stopped')
  process.exit(0)
})
setInterval(() => {}, 1000)
"#,
    )
    .expect("failing fake Pi");
    make_executable(path);
}

async fn wait_for_file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("process cleanup");
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
    assert_eq!(
        process
            .get_available_models()
            .await
            .expect("available models")
            .data,
        Some(json!({
            "models": [{"provider": "openai-codex", "id": "gpt-5.6-sol"}]
        }))
    );
    assert_eq!(
        process
            .get_session_stats()
            .await
            .expect("session stats")
            .data,
        Some(json!({
            "contextUsage": {"tokens": 12000, "contextWindow": 400000, "percent": 3}
        }))
    );
    process
        .set_model("openai-codex", "gpt-5.6-sol")
        .await
        .expect("set model");
    process
        .set_thinking_level("medium")
        .await
        .expect("set thinking level");
    assert_eq!(
        process.compact().await.expect("compact").data,
        Some(json!({"tokensBefore": 12000, "estimatedTokensAfter": 4000}))
    );
    assert_eq!(
        process.bash("ls").await.expect("bash").data,
        Some(json!({
            "output": "file.txt",
            "exitCode": 0,
            "cancelled": false,
            "truncated": false
        }))
    );
    process.abort_retry().await.expect("abort retry");
    process.abort_bash().await.expect("abort bash");
    process.abort().await.expect("abort agent");
    process
        .prompt("Hello", &[], RpcDelivery::Normal)
        .await
        .expect("prompt");
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

#[tokio::test]
async fn stops_the_child_when_initial_state_fails() {
    let directory = tempfile::tempdir().expect("temp directory");
    let executable = directory.path().join("failing-pi");
    let stopped_path = directory.path().join("stopped");
    write_failing_pi(&executable);
    let result = PiProcess::spawn(PiProcessConfig {
        executable,
        working_directory: directory.path().into(),
        session_directory: directory.path().join("sessions"),
        session_path: None,
        extension_path: None,
        environment: HashMap::from([
            ("FAKE_PI_FAIL_ON".into(), "get_state".into()),
            (
                "FAKE_PI_STOPPED_PATH".into(),
                stopped_path.to_string_lossy().into_owned(),
            ),
        ]),
        request_timeout: Duration::from_secs(2),
    })
    .await;

    assert!(result.is_err());
    wait_for_file(&stopped_path).await;
}

#[tokio::test]
async fn stops_the_child_when_rpc_stdout_becomes_invalid() {
    let directory = tempfile::tempdir().expect("temp directory");
    let executable = directory.path().join("failing-pi");
    let stopped_path = directory.path().join("stopped");
    write_failing_pi(&executable);
    let process = PiProcess::spawn(PiProcessConfig {
        executable,
        working_directory: directory.path().into(),
        session_directory: directory.path().join("sessions"),
        session_path: None,
        extension_path: None,
        environment: HashMap::from([
            ("FAKE_PI_FAIL_ON".into(), "get_available_models".into()),
            (
                "FAKE_PI_STOPPED_PATH".into(),
                stopped_path.to_string_lossy().into_owned(),
            ),
        ]),
        request_timeout: Duration::from_secs(2),
    })
    .await
    .expect("Pi process");

    assert!(process.get_available_models().await.is_err());
    wait_for_file(&stopped_path).await;
    let mut status = process.status();
    tokio::time::timeout(Duration::from_secs(3), async {
        while matches!(*status.borrow(), luna_pi::ProcessStatus::Running) {
            status.changed().await.expect("status change");
        }
    })
    .await
    .expect("process exit status");
}
