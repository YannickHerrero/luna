use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Query, State, WebSocketUpgrade, ws::Message as WebSocketMessage},
    http::{HeaderMap, StatusCode, header},
    response::Redirect,
    routing::{get, post},
};
use luna_protocol::{DevicePlatform, PairingExchangeRequest, ServerEvent};
use luna_tui::{
    api::{ApiClientError, LunaApi, ServerOrigin},
    realtime::{EventSocket, RealtimeUpdate, spawn_realtime},
};

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
async fn supports_concurrent_event_sockets_with_one_credential() {
    let router = Router::new().route(
        "/v1/events",
        get(
            |headers: HeaderMap, websocket: WebSocketUpgrade| async move {
                assert_eq!(
                    headers
                        .get(header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok()),
                    Some("Bearer shared-token")
                );
                websocket.on_upgrade(|mut socket| async move {
                    socket
                        .send(WebSocketMessage::Text(
                            serde_json::json!({
                                "version": 1,
                                "emittedAt": "2026-01-01T00:00:00Z",
                                "type": "server.welcome",
                                "payload": {"cursor": 7, "resumed": true}
                            })
                            .to_string()
                            .into(),
                        ))
                        .await
                        .expect("welcome");
                })
            },
        ),
    );
    let (origin, server) = spawn(router).await;
    let api = LunaApi::new(origin, Some("shared-token".into())).expect("client");

    let (left, right) = tokio::join!(EventSocket::connect(&api, 4), EventSocket::connect(&api, 4));
    let mut left = left.expect("left socket");
    let mut right = right.expect("right socket");
    let (left_event, right_event) = tokio::join!(left.next(), right.next());
    for event in [left_event, right_event] {
        assert!(matches!(
            event.expect("event").expect("welcome").event,
            ServerEvent::ServerWelcome {
                cursor: 7,
                resumed: true
            }
        ));
    }

    server.abort();
}

#[tokio::test]
async fn reconnects_from_the_latest_applied_cursor() {
    #[derive(Clone)]
    struct ReconnectState {
        connections: Arc<AtomicUsize>,
        cursors: Arc<Mutex<Vec<i64>>>,
    }

    let state = ReconnectState {
        connections: Arc::new(AtomicUsize::new(0)),
        cursors: Arc::new(Mutex::new(vec![])),
    };
    let router = Router::new()
        .route(
            "/v1/events",
            get(
                |State(state): State<ReconnectState>,
                 Query(query): Query<HashMap<String, i64>>,
                 websocket: WebSocketUpgrade| async move {
                    let connection = state.connections.fetch_add(1, Ordering::SeqCst);
                    state
                        .cursors
                        .lock()
                        .expect("cursor lock")
                        .push(query.get("after").copied().unwrap_or_default());
                    websocket.on_upgrade(move |mut socket| async move {
                        socket
                            .send(WebSocketMessage::Text(
                                serde_json::json!({
                                    "version": 1,
                                    "emittedAt": "2026-01-01T00:00:00Z",
                                    "type": "server.welcome",
                                    "payload": {"cursor": 5, "resumed": true}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await
                            .expect("welcome");
                        if connection == 0 {
                            socket
                                .send(WebSocketMessage::Text(
                                    serde_json::json!({
                                        "version": 1,
                                        "eventId": 5,
                                        "emittedAt": "2026-01-01T00:00:01Z",
                                        "type": "server.pong",
                                        "payload": {
                                            "request_id": "00000000-0000-0000-0000-000000000005"
                                        }
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await
                                .expect("cursor event");
                        } else {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    })
                },
            ),
        )
        .with_state(state.clone());
    let (origin, server) = spawn(router).await;
    let api = LunaApi::new(origin, Some("shared-token".into())).expect("client");
    let (realtime, mut updates) = spawn_realtime(api, 4);
    let mut connected = 0;

    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(update) = updates.recv().await {
            match update {
                RealtimeUpdate::Connected => {
                    connected += 1;
                    if connected == 2 {
                        break;
                    }
                }
                RealtimeUpdate::Event(event) if event.event_id == Some(5) => {
                    realtime.set_cursor(5);
                }
                _ => {}
            }
        }
    })
    .await
    .expect("reconnect timeout");

    assert_eq!(*state.cursors.lock().expect("cursor lock"), vec![4, 5]);
    realtime.shutdown().await;
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
