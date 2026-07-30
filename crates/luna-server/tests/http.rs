use std::path::PathBuf;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use luna_protocol::{Conversation, PairingExchangeResponse};
use luna_server::{app, config::Config};
use tower::ServiceExt;

fn config(directory: &std::path::Path) -> Config {
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
        pi_executable: PathBuf::from("pi"),
        pi_bridge_path: PathBuf::from("bridge.ts"),
        event_retention_days: 30,
        transcription_model: "gpt-4o-mini-transcribe".into(),
    }
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
async fn pairs_a_device_and_creates_a_conversation() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let pairing_request = Request::builder()
        .method("POST")
        .uri("/v1/pairing/exchange")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "code": built.pairing_code,
                "deviceName": "Test iPhone",
                "platform": "ios"
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
}

#[tokio::test]
async fn health_does_not_expose_private_state() {
    let directory = tempfile::tempdir().expect("temp directory");
    let built = app::build(config(directory.path())).await.expect("app");
    let response = built
        .router
        .oneshot(
            Request::builder()
                .uri("/v1/health/live")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json::<serde_json::Value>(response).await,
        serde_json::json!({"status":"ok"})
    );
}
