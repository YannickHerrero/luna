use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::Redirect,
    routing::{get, post},
};
use luna_protocol::{DevicePlatform, PairingExchangeRequest};
use luna_tui::api::{ApiClientError, LunaApi, ServerOrigin};

#[derive(Clone)]
struct TestState {
    saw_tui_pairing: Arc<AtomicBool>,
}

#[tokio::test]
async fn pairs_as_a_tui_and_authenticates_bootstrap() {
    let state = TestState {
        saw_tui_pairing: Arc::new(AtomicBool::new(false)),
    };
    let router = Router::new()
        .route(
            "/v1/pairing/request",
            post(|| async {
                (
                    StatusCode::ACCEPTED,
                    Json(serde_json::json!({"expiresAt": "2026-01-01T00:15:00Z"})),
                )
            }),
        )
        .route(
            "/v1/pairing/exchange",
            post(
                |State(state): State<TestState>, Json(body): Json<PairingExchangeRequest>| async move {
                    assert_eq!(body.code, "123456");
                    assert_eq!(body.device_name, "SSH terminal");
                    assert_eq!(body.platform, DevicePlatform::Tui);
                    state.saw_tui_pairing.store(true, Ordering::SeqCst);
                    (
                        StatusCode::CREATED,
                        Json(pairing_response()),
                    )
                },
            ),
        )
        .route(
            "/v1/bootstrap",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()),
                    Some("Bearer test-token")
                );
                Json(bootstrap())
            }),
        )
        .with_state(state.clone());
    let (origin, server) = spawn(router).await;

    let unauthenticated = LunaApi::new(origin.clone(), None).expect("client");
    let pairing = unauthenticated
        .request_pairing_code()
        .await
        .expect("pairing request");
    assert_eq!(pairing.expires_at, "2026-01-01T00:15:00Z");
    let paired = unauthenticated
        .exchange_pairing_code("123456", "SSH terminal")
        .await
        .expect("pairing exchange");
    assert_eq!(paired.token, "test-token");
    assert!(state.saw_tui_pairing.load(Ordering::SeqCst));

    let authenticated = LunaApi::new(origin, Some(paired.token)).expect("client");
    let bootstrap = authenticated.bootstrap().await.expect("bootstrap");
    assert_eq!(bootstrap.device.platform, DevicePlatform::Tui);

    server.abort();
}

#[tokio::test]
async fn rejects_redirects_instead_of_forwarding_requests() {
    let router = Router::new().route(
        "/v1/pairing/request",
        post(|| async { Redirect::temporary("https://example.com/steal") }),
    );
    let (origin, server) = spawn(router).await;
    let api = LunaApi::new(origin, None).expect("client");

    assert!(matches!(
        api.request_pairing_code().await,
        Err(ApiClientError::RedirectRejected(status))
            if status == StatusCode::TEMPORARY_REDIRECT.as_u16()
    ));

    server.abort();
}

async fn spawn(router: Router) -> (ServerOrigin, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("server");
    });
    (
        ServerOrigin::parse(&format!("http://{address}")).expect("origin"),
        server,
    )
}

fn pairing_response() -> serde_json::Value {
    serde_json::json!({
        "deviceId": "00000000-0000-0000-0000-000000000001",
        "token": "test-token",
        "bootstrap": bootstrap()
    })
}

fn bootstrap() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "cursor": 0,
        "device": {
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "SSH terminal",
            "platform": "tui",
            "notificationsEnabled": false,
            "createdAt": "2026-01-01T00:00:00Z",
            "lastSeenAt": "2026-01-01T00:00:00Z"
        },
        "conversations": []
    })
}
