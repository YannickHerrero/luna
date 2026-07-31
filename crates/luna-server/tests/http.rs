use std::{fs, path::PathBuf, time::Duration};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use luna_protocol::{
    ApiError, AttachmentResponse, CompactConversationResponse, Conversation,
    ConversationAgentState, ConversationList, ConversationMessages, ErrorCode,
    PairingCodeRequestResponse, PairingExchangeResponse, SendMessageResponse,
};
use luna_server::{app, config::Config};
use tower::ServiceExt;

fn config(directory: &std::path::Path) -> Config {
    let pi_executable = directory.join("fake-pi");
    fs::write(
        &pi_executable,
        r#"#!/usr/bin/env node
if (process.argv.includes('--print')) {
  if (!process.argv.includes('openai-codex/gpt-5.6-luna') || !process.argv.includes('--no-tools')) process.exit(3)
  require('node:fs').readFileSync(0, 'utf8')
  console.log('Persistent Message Delivery')
  process.exit(0)
}
const net = require('node:net')
const bridge = net.createConnection(process.env.LUNA_BRIDGE_SOCKET)
let dispatchId
bridge.on('connect', () => bridge.write(JSON.stringify({type:'ready', pid:process.pid, cwd:process.cwd()}) + '\n'))
let bridgeBuffer = ''
bridge.on('data', chunk => {
  bridgeBuffer += chunk.toString('utf8')
  while (bridgeBuffer.includes('\n')) {
    const index = bridgeBuffer.indexOf('\n')
    const command = JSON.parse(bridgeBuffer.slice(0, index))
    bridgeBuffer = bridgeBuffer.slice(index + 1)
    if (command.type === 'dispatch') {
      dispatchId = command.dispatchId
      bridge.write(JSON.stringify({type:'dispatch_ready', dispatchId}) + '\n')
    }
  }
})
let input = ''
const models = [
  {provider:'openai-codex',id:'gpt-5.6-sol',name:'GPT-5.6 Sol',reasoning:true,contextWindow:400000,thinkingLevelMap:{xhigh:'xhigh',max:'max'}},
  {provider:'local',id:'fast',name:'Fast Local',reasoning:false,contextWindow:32000}
]
let currentModel = models[0]
let thinkingLevel = 'xhigh'
process.stdin.on('data', chunk => {
  input += chunk.toString('utf8')
  while (input.includes('\n')) {
    const index = input.indexOf('\n')
    const request = JSON.parse(input.slice(0, index))
    input = input.slice(index + 1)
    if (request.type === 'get_state') {
      console.log(JSON.stringify({id:request.id,type:'response',command:'get_state',success:true,data:{sessionId:'fake-session',sessionFile:'/tmp/fake-luna-session.jsonl',model:currentModel,thinkingLevel,isStreaming:false,isCompacting:false,autoCompactionEnabled:true}}))
    } else if (request.type === 'get_available_models') {
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true,data:{models}}))
    } else if (request.type === 'get_session_stats') {
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true,data:{contextUsage:{tokens:120000,contextWindow:currentModel.contextWindow,percent:30}}}))
    } else if (request.type === 'set_model') {
      currentModel = models.find(model => model.provider === request.provider && model.id === request.modelId)
      thinkingLevel = currentModel.reasoning ? thinkingLevel : 'off'
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true,data:currentModel}))
    } else if (request.type === 'set_thinking_level') {
      thinkingLevel = request.level
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true}))
    } else if (request.type === 'bash') {
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true,data:{output:'file.txt\n',exitCode:0,cancelled:false,truncated:false}}))
    } else if (request.type === 'compact') {
      console.log(JSON.stringify({type:'compaction_start',reason:'manual'}))
      const result = {summary:'Compacted',firstKeptEntryId:'entry',tokensBefore:120000,estimatedTokensAfter:24000,details:{}}
      console.log(JSON.stringify({type:'compaction_end',reason:'manual',result,aborted:false,willRetry:false}))
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true,data:result}))
    } else if (request.type === 'abort' || request.type === 'abort_bash' || request.type === 'abort_retry') {
      console.log(JSON.stringify({id:request.id,type:'response',command:request.type,success:true}))
    } else if (request.type === 'prompt') {
      if (!request.images || request.images.length !== 1 || !request.images[0].data) process.exit(2)
      console.log(JSON.stringify({id:request.id,type:'response',command:'prompt',success:true}))
      bridge.write(JSON.stringify({type:'dispatch_recorded',dispatchId}) + '\n')
      bridge.write(JSON.stringify({type:'path_observed',path:__dirname + '/repository/file.txt',toolName:'read'}) + '\n')
      bridge.write(JSON.stringify({type:'task_list_updated',taskList:{id:'018f0000-0000-7000-8000-000000000001',title:'Ship Luna progress',revision:2,tasks:[{id:'018f0000-0000-7000-8000-000000000002',sequence:1,text:'Persist structured progress',status:'completed',note:'HTTP integration passed',createdAt:'2026-03-20T12:00:00Z',updatedAt:'2026-03-20T12:00:01Z'},{id:'018f0000-0000-7000-8000-000000000003',sequence:2,text:'Render progress in Luna',status:'in_progress',createdAt:'2026-03-20T12:00:00Z',updatedAt:'2026-03-20T12:00:01Z'}],createdAt:'2026-03-20T12:00:00Z',updatedAt:'2026-03-20T12:00:01Z'}}) + '\n')
      console.log(JSON.stringify({type:'agent_start'}))
      console.log(JSON.stringify({type:'message_update',assistantMessageEvent:{type:'text_delta',contentIndex:0,delta:'Fake response'}}))
      console.log(JSON.stringify({type:'agent_settled'}))
    }
  }
})
process.stdin.on('end', () => process.exit(0))
"#,
    )
    .expect("fake Pi");
    fs::create_dir_all(directory.join("repository/.git")).expect("fake repository");
    fs::write(directory.join("repository/file.txt"), "repository file").expect("repository file");
    let mut repository_icon = Vec::new();
    image::DynamicImage::new_rgba8(4, 4)
        .write_to(
            &mut std::io::Cursor::new(&mut repository_icon),
            image::ImageFormat::Png,
        )
        .expect("repository icon");
    fs::write(directory.join("repository/icon.png"), repository_icon)
        .expect("repository icon file");
    fs::create_dir_all(directory.join("web/_next/static")).expect("web directory");
    fs::write(directory.join("web/index.html"), "<h1>Luna PWA</h1>").expect("web index");
    fs::write(directory.join("web/_next/static/chunk.js"), "export {}").expect("web asset");
    fs::write(directory.join("bridge.ts"), "export default () => {}").expect("bridge extension");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&pi_executable)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&pi_executable, permissions).expect("permissions");
    }
    Config {
        bind_host: "127.0.0.1".into(),
        port: 9870,
        public_origin: None,
        allowed_tailnet_logins: vec![],
        data_directory: directory.into(),
        credentials_directory: directory.into(),
        database_path: directory.join("luna.sqlite"),
        pi_session_directory: directory.join("pi-sessions"),
        attachment_directory: directory.join("attachments"),
        repository_icon_directory: directory.join("repository-icons"),
        bridge_directory: PathBuf::from("/tmp")
            .join(format!("luna-http-test-{}", std::process::id())),
        web_directory: directory.join("web"),
        pi_executable,
        pi_bridge_path: directory.join("bridge.ts"),
        title_model: "openai-codex/gpt-5.6-luna".into(),
        event_retention_days: 30,
        attachment_retention_days: 30,
        transcription_model: "gpt-transcribe".into(),
        transcription_api_key: None,
        transcription_base_url: "https://api.openai.com/v1".into(),
    }
}

fn assert_pairing_code_format(code: &str) {
    assert_eq!(code.len(), 6);
    assert!(code.bytes().all(|byte| byte.is_ascii_digit()));
}

async fn response_json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("response body"),
    )
    .expect("response JSON")
}

#[tokio::test]
async fn requests_a_new_pairing_code_without_exposing_it() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let requested = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pairing/request")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("pairing code response");
    assert_eq!(requested.status(), StatusCode::ACCEPTED);
    let requested: PairingCodeRequestResponse = response_json(requested).await;
    assert!(!requested.expires_at.is_empty());

    let stale = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pairing/exchange")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "code": built.pairing_code,
                        "deviceName": "Stale browser",
                        "platform": "web"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("stale pairing response");
    assert_eq!(stale.status(), StatusCode::BAD_REQUEST);
    let error: ApiError = response_json(stale).await;
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    assert!(error.message.contains("invalid, expired, or already used"));
    built.runtime.shutdown().await;
}

#[tokio::test]
async fn pairs_a_device_and_creates_a_conversation() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    assert_pairing_code_format(&built.pairing_code);
    let pairing_request = Request::builder()
        .method("POST")
        .uri("/v1/pairing/exchange")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "code": built.pairing_code,
                "deviceName": "Test browser",
                "platform": "web"
            })
            .to_string(),
        ))
        .expect("request");
    let pairing_response = built
        .router
        .clone()
        .oneshot(pairing_request)
        .await
        .expect("pairing response");
    assert_eq!(pairing_response.status(), StatusCode::CREATED);
    let cookie = pairing_response
        .headers()
        .get(header::SET_COOKIE)
        .expect("device cookie")
        .to_str()
        .expect("cookie text");
    assert!(cookie.contains("Max-Age=31536000"));
    assert!(cookie.contains("HttpOnly"));
    let paired: PairingExchangeResponse = response_json(pairing_response).await;

    let create_request = Request::builder()
        .method("POST")
        .uri("/v1/conversations")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
        .body(Body::from("{}"))
        .expect("request");
    let create_response = built
        .router
        .clone()
        .oneshot(create_request)
        .await
        .expect("create response");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let conversation: Conversation = response_json(create_response).await;
    assert_eq!(conversation.title, "New Conversation");

    let mut png = Vec::new();
    image::DynamicImage::new_rgba8(2, 2)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("PNG");
    let boundary = "luna-test-boundary";
    let mut multipart = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"conversationId\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.png\"\r\nContent-Type: image/png\r\n\r\n",
        conversation.id
    )
    .into_bytes();
    multipart.extend_from_slice(&png);
    multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let upload_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/attachments")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from(multipart))
                .expect("request"),
        )
        .await
        .expect("upload response");
    assert_eq!(upload_response.status(), StatusCode::CREATED);
    let uploaded: AttachmentResponse = response_json(upload_response).await;
    assert_eq!(uploaded.attachment.width, 2);
    assert_eq!(uploaded.attachment.height, 2);

    let content_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&uploaded.attachment.content_url)
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("content response");
    assert_eq!(content_response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(content_response.into_body(), 1_000_000)
            .await
            .expect("content"),
        png
    );

    let client_message_id = uuid::Uuid::new_v4();
    let send_request = Request::builder()
        .method("POST")
        .uri(format!("/v1/conversations/{}/messages", conversation.id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
        .body(Body::from(
            serde_json::json!({
                "clientMessageId": client_message_id,
                "text": "Persist this message",
                "attachmentIds": [uploaded.attachment.id]
            })
            .to_string(),
        ))
        .expect("request");
    let send_response = built
        .router
        .clone()
        .oneshot(send_request)
        .await
        .expect("send response");
    assert_eq!(send_response.status(), StatusCode::ACCEPTED);
    let sent: SendMessageResponse = response_json(send_response).await;
    assert!(sent.accepted);
    assert_eq!(sent.message.client_message_id, Some(client_message_id));

    let messages_request = Request::builder()
        .uri(format!("/v1/conversations/{}/messages", conversation.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
        .body(Body::empty())
        .expect("request");
    let messages_response = built
        .router
        .clone()
        .oneshot(messages_request)
        .await
        .expect("messages response");
    let messages: ConversationMessages = response_json(messages_response).await;
    assert_eq!(messages.messages[0].text, "Persist this message");
    assert_eq!(
        messages.messages[0].attachments[0].id,
        uploaded.attachment.id
    );

    let mut completed_messages = messages;
    for _ in 0..40 {
        if completed_messages.messages.len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
        let request = Request::builder()
            .uri(format!("/v1/conversations/{}/messages", conversation.id))
            .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
            .body(Body::empty())
            .expect("request");
        completed_messages = response_json(
            built
                .router
                .clone()
                .oneshot(request)
                .await
                .expect("messages response"),
        )
        .await;
    }
    assert_eq!(completed_messages.messages.len(), 2);
    assert_eq!(completed_messages.messages[1].text, "Fake response");
    assert_eq!(
        completed_messages.messages[1].status,
        luna_protocol::MessageStatus::Completed
    );
    let mut observed_repositories = Vec::new();
    let mut observed_title = String::new();
    let mut observed_task_list = None;
    for _ in 0..80 {
        let request = Request::builder()
            .uri(format!("/v1/conversations/{}", conversation.id))
            .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
            .body(Body::empty())
            .expect("request");
        let response = built
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("conversation response");
        let current = response_json::<Conversation>(response).await;
        observed_repositories = current.repositories;
        observed_title = current.title;
        observed_task_list = current.task_list;
        if !observed_repositories.is_empty()
            && observed_title == "Persistent Message Delivery"
            && observed_task_list.is_some()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(observed_title, "Persistent Message Delivery");
    assert_eq!(observed_repositories[0].display_name, "repository");
    let task_list = observed_task_list.expect("task list");
    assert_eq!(task_list.revision, 2);
    assert_eq!(task_list.tasks.len(), 2);
    assert_eq!(
        task_list.tasks[0].status,
        luna_protocol::AgentTaskStatus::Completed
    );
    let icon_url = observed_repositories[0]
        .icon
        .content_url
        .as_deref()
        .expect("repository icon URL");
    let icon_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(icon_url)
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("repository icon response");
    assert_eq!(icon_response.status(), StatusCode::OK);
    assert_eq!(
        icon_response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("icon content type"),
        "image/png"
    );

    let bootstrap_request = Request::builder()
        .uri("/v1/bootstrap")
        .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
        .body(Body::empty())
        .expect("request");
    let bootstrap_response = built
        .router
        .clone()
        .oneshot(bootstrap_request)
        .await
        .expect("bootstrap response");
    assert_eq!(bootstrap_response.status(), StatusCode::OK);
    let bootstrap: luna_protocol::Bootstrap = response_json(bootstrap_response).await;
    assert_eq!(bootstrap.conversations.len(), 1);

    let archive_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/conversations/{}/archive", conversation.id))
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("archive request"),
        )
        .await
        .expect("archive response");
    assert_eq!(archive_response.status(), StatusCode::NO_CONTENT);

    let active_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/conversations")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("active list request"),
        )
        .await
        .expect("active list response");
    assert!(
        response_json::<ConversationList>(active_response)
            .await
            .conversations
            .is_empty()
    );
    let archived_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/conversations?scope=archived")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("archive list request"),
        )
        .await
        .expect("archive list response");
    let archived = response_json::<ConversationList>(archived_response).await;
    assert_eq!(archived.conversations.len(), 1);
    assert_eq!(archived.conversations[0].id, conversation.id);
    assert!(archived.conversations[0].archived_at.is_some());

    let restore_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/conversations/{}/restore", conversation.id))
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("restore request"),
        )
        .await
        .expect("restore response");
    assert_eq!(restore_response.status(), StatusCode::OK);
    let restored: Conversation = response_json(restore_response).await;
    assert_eq!(restored.archived_at, None);
    assert_eq!(restored.state, luna_protocol::SessionState::Stopped);

    built.runtime.shutdown().await;
}

#[tokio::test]
async fn health_does_not_expose_private_state() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/health/live")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    assert!(
        response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .is_some()
    );
    assert_eq!(
        response_json::<serde_json::Value>(response).await,
        serde_json::json!({"status":"ok"})
    );
    let ready_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/health/ready")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("ready response");
    assert_eq!(ready_response.status(), StatusCode::OK);
    assert_eq!(
        response_json::<serde_json::Value>(ready_response).await,
        serde_json::json!({"status":"ready"})
    );
    let static_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("static response");
    assert_eq!(static_response.status(), StatusCode::OK);
    assert_eq!(
        static_response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache control"),
        "no-cache"
    );
    assert!(
        static_response
            .headers()
            .contains_key(header::STRICT_TRANSPORT_SECURITY)
    );
    assert_eq!(
        to_bytes(static_response.into_body(), 1_000)
            .await
            .expect("static body"),
        "<h1>Luna PWA</h1>"
    );
    let asset_response = built
        .router
        .oneshot(
            Request::builder()
                .uri("/_next/static/chunk.js")
                .body(Body::empty())
                .expect("asset request"),
        )
        .await
        .expect("asset response");
    assert_eq!(
        asset_response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("asset cache control"),
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn websocket_replays_and_streams_persistent_events() {
    use futures_util::StreamExt;
    use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let pairing_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pairing/exchange")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "code": built.pairing_code,
                        "deviceName": "WebSocket Client",
                        "platform": "ios"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("pairing response");
    let paired: PairingExchangeResponse = response_json(pairing_response).await;
    let replay_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("replay conversation response");
    let replay_conversation: Conversation = response_json(replay_response).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let server_router = built.router.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_router)
            .with_graceful_shutdown(std::future::pending())
            .await
            .expect("server");
    });
    let mut request = format!("ws://{address}/v1/events?after=0")
        .into_client_request()
        .expect("WebSocket request");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {}", paired.token)
            .parse()
            .expect("authorization"),
    );
    let (mut socket, _) = connect_async(request).await.expect("WebSocket");
    let welcome = socket
        .next()
        .await
        .expect("welcome frame")
        .expect("welcome");
    let welcome: luna_protocol::ServerEventEnvelope =
        serde_json::from_str(welcome.to_text().expect("text")).expect("welcome event");
    assert!(matches!(
        welcome.event,
        luna_protocol::ServerEvent::ServerWelcome { .. }
    ));
    let replay = socket
        .next()
        .await
        .expect("replay frame")
        .expect("replay event");
    let replay: luna_protocol::ServerEventEnvelope =
        serde_json::from_str(replay.to_text().expect("text")).expect("replayed event");
    assert_eq!(replay.conversation_id, Some(replay_conversation.id));

    let create_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("create response");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let event = tokio::time::timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("event timeout")
        .expect("event frame")
        .expect("event");
    let event: luna_protocol::ServerEventEnvelope =
        serde_json::from_str(event.to_text().expect("text")).expect("server event");
    assert!(event.event_id.is_some());
    assert!(matches!(
        event.event,
        luna_protocol::ServerEvent::ConversationUpserted(_)
    ));

    socket.close(None).await.expect("close");
    server.abort();
    built.runtime.shutdown().await;
}

#[tokio::test]
async fn executes_bang_messages_through_the_pi_session() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let pairing_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pairing/exchange")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "code": built.pairing_code,
                        "deviceName": "Shell commands",
                        "platform": "web"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("pairing response");
    let paired: PairingExchangeResponse = response_json(pairing_response).await;
    let create_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("create response");
    let conversation: Conversation = response_json(create_response).await;
    let messages_endpoint = format!("/v1/conversations/{}/messages", conversation.id);
    let send_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&messages_endpoint)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from(
                    serde_json::json!({
                        "clientMessageId": uuid::Uuid::new_v4(),
                        "text": "!ls",
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("send response");
    assert_eq!(send_response.status(), StatusCode::ACCEPTED);
    let sent: SendMessageResponse = response_json(send_response).await;
    assert_eq!(
        sent.message.delivery,
        Some(luna_protocol::MessageDelivery::Bash)
    );

    let mut messages = None;
    for _ in 0..40 {
        let response = built
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&messages_endpoint)
                    .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("messages response");
        let current: ConversationMessages = response_json(response).await;
        if current.messages.len() == 2 {
            messages = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let messages = messages.expect("shell output message").messages;
    assert_eq!(messages[0].text, "!ls");
    assert_eq!(messages[1].role, luna_protocol::MessageRole::Assistant);
    assert!(messages[1].text.contains("file.txt"));
    assert!(messages[1].text.contains("Exit code: `0`"));
    built.runtime.shutdown().await;
}

#[tokio::test]
async fn reads_updates_and_compacts_conversation_agent_state() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let pairing_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pairing/exchange")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "code": built.pairing_code,
                        "deviceName": "Agent controls",
                        "platform": "web"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("pairing response");
    let paired: PairingExchangeResponse = response_json(pairing_response).await;
    let create_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("create response");
    let conversation: Conversation = response_json(create_response).await;
    let endpoint = format!("/v1/conversations/{}/agent", conversation.id);
    let state_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&endpoint)
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("agent state response");
    assert_eq!(state_response.status(), StatusCode::OK);
    let state: ConversationAgentState = response_json(state_response).await;
    assert_eq!(
        state.model.as_ref().map(|model| model.id.as_str()),
        Some("gpt-5.6-sol")
    );
    assert_eq!(state.available_models.len(), 2);
    assert_eq!(
        state.context_usage.as_ref().and_then(|usage| usage.tokens),
        Some(120_000)
    );
    let local = state
        .available_models
        .iter()
        .find(|model| model.id == "fast")
        .expect("local model");
    assert_eq!(
        local.supported_thinking_levels,
        vec![luna_protocol::ThinkingLevel::Off]
    );
    let gpt = state
        .available_models
        .iter()
        .find(|model| model.id == "gpt-5.6-sol")
        .expect("GPT model");
    assert!(
        gpt.supported_thinking_levels
            .contains(&luna_protocol::ThinkingLevel::Max)
    );

    let update_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&endpoint)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from(
                    serde_json::json!({
                        "model": {"provider": "local", "modelId": "fast"},
                        "thinkingLevel": "off"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("agent update response");
    assert_eq!(update_response.status(), StatusCode::OK);
    let state: ConversationAgentState = response_json(update_response).await;
    assert_eq!(
        state.model.as_ref().map(|model| model.id.as_str()),
        Some("fast")
    );
    assert_eq!(state.thinking_level, luna_protocol::ThinkingLevel::Off);

    let compact_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/conversations/{}/compact", conversation.id))
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("compact response");
    assert_eq!(compact_response.status(), StatusCode::OK);
    let compacted: CompactConversationResponse = response_json(compact_response).await;
    assert_eq!(compacted.tokens_before, 120_000);
    assert_eq!(compacted.estimated_tokens_after, 24_000);
    built.runtime.shutdown().await;
}

#[tokio::test]
async fn proxies_voice_transcription_without_persisting_audio() {
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let upstream_address = upstream_listener.local_addr().expect("upstream address");
    let (request_sender, mut request_receiver) = tokio::sync::mpsc::channel(1);
    let upstream = tokio::spawn(async move {
        axum::serve(
            upstream_listener,
            axum::Router::new().route(
                "/audio/transcriptions",
                axum::routing::post(move |mut multipart: axum::extract::Multipart| {
                    let request_sender = request_sender.clone();
                    async move {
                        let mut model = None;
                        let mut file = None;
                        while let Some(field) = multipart
                            .next_field()
                            .await
                            .expect("upstream multipart field")
                        {
                            let field_name = field.name().map(str::to_owned);
                            match field_name.as_deref() {
                                Some("model") => {
                                    model = Some(field.text().await.expect("upstream model"));
                                }
                                Some("file") => {
                                    let file_name = field.file_name().map(str::to_owned);
                                    let mime_type = field.content_type().map(str::to_owned);
                                    let bytes = field.bytes().await.expect("upstream audio");
                                    file = Some((file_name, mime_type, bytes));
                                }
                                _ => {}
                            }
                        }
                        request_sender
                            .send((model, file))
                            .await
                            .expect("capture upstream request");
                        axum::Json(serde_json::json!({ "text": "Transcribed locally" }))
                    }
                }),
            ),
        )
        .await
        .expect("upstream");
    });
    let directory = tempfile::tempdir().expect("temp directory");
    let mut server_config = config(directory.path());
    server_config.transcription_api_key = Some("test-key".into());
    server_config.transcription_base_url = format!("http://{upstream_address}");
    let built = app::build(server_config).await.expect("app");
    let pairing_response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pairing/exchange")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "code": built.pairing_code,
                        "deviceName": "Recorder",
                        "platform": "ios"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("pairing response");
    let paired: PairingExchangeResponse = response_json(pairing_response).await;
    let boundary = "luna-audio-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"recording.webm\"\r\nContent-Type: audio/webm\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(b"temporary audio bytes");
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let response = built
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/transcriptions")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header(header::AUTHORIZATION, format!("Bearer {}", paired.token))
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("transcription response");
    assert_eq!(response.status(), StatusCode::OK);
    let transcription: luna_protocol::TranscriptionResponse = response_json(response).await;
    assert_eq!(transcription.text, "Transcribed locally");
    let (model, file) = tokio::time::timeout(Duration::from_secs(1), request_receiver.recv())
        .await
        .expect("upstream request timeout")
        .expect("upstream request");
    assert_eq!(model.as_deref(), Some("gpt-transcribe"));
    let (file_name, mime_type, bytes) = file.expect("upstream audio file");
    assert_eq!(file_name.as_deref(), Some("recording.webm"));
    assert_eq!(mime_type.as_deref(), Some("audio/webm"));
    assert_eq!(bytes.as_ref(), b"temporary audio bytes");
    assert!(!directory.path().join("attachments").exists());
    upstream.abort();
}
