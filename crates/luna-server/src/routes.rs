use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query, State,
        ws::{Message as WebSocketMessage, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::IntoResponse,
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use luna_protocol::{
    ApiError, Bootstrap, ClientCommand, CommandAccepted, CommandRejected, ConversationList,
    ConversationMessages, CreateConversationRequest, ErrorCode, Message, MessageDelivery,
    PROTOCOL_VERSION, PairingExchangeRequest, PairingExchangeResponse, SendMessageRequest,
    SendMessageResponse, ServerEvent, ServerEventEnvelope, SessionState, SyncResponse,
    UpdateConversationRequest,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::now,
    error::AppError,
    extract::{AuthenticatedDevice, validate_origin, validate_tailnet},
    media,
    state::AppState,
    transcription,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health/live", get(health))
        .route("/v1/pairing/exchange", post(pair))
        .route("/v1/bootstrap", get(bootstrap))
        .route("/v1/sync", get(sync))
        .route("/v1/events", get(events_socket))
        .route(
            "/v1/transcriptions",
            post(transcription::transcribe).layer(DefaultBodyLimit::max(27 * 1024 * 1024)),
        )
        .route(
            "/v1/attachments",
            post(media::upload_attachment).layer(DefaultBodyLimit::max(22 * 1024 * 1024)),
        )
        .route(
            "/v1/attachments/{id}/content",
            get(media::attachment_content),
        )
        .route(
            "/v1/attachments/{id}/thumbnail",
            get(media::attachment_thumbnail),
        )
        .route(
            "/v1/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/v1/conversations/{id}",
            get(get_conversation).patch(update_conversation),
        )
        .route(
            "/v1/conversations/{id}/messages",
            get(list_messages).post(send_message),
        )
        .route("/v1/conversations/{id}/abort", post(abort_conversation))
        .route("/v1/conversations/{id}/archive", post(archive_conversation))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn pair(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PairingExchangeRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_tailnet(&headers, &state)?;
    validate_origin(&axum::http::Method::POST, &headers, &state)?;
    let name = request.device_name.trim();
    if name.is_empty() || name.len() > 80 || request.code.len() < 6 {
        return Err(AppError::InvalidRequest(
            "The pairing request is invalid.".into(),
        ));
    }
    let paired = state
        .auth
        .exchange(&request.code, name, request.platform)
        .await?
        .ok_or(AppError::AuthenticationRequired)?;
    let cursor = state.database.latest_cursor().await?;
    let conversations = state.database.conversations(false).await?;
    let body = PairingExchangeResponse {
        device_id: paired.device.id,
        token: paired.token.clone(),
        bootstrap: Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            cursor,
            device: paired.device.clone(),
            conversations,
        },
    };
    let mut response = (StatusCode::CREATED, Json(body)).into_response();
    if paired.device.platform == luna_protocol::DevicePlatform::Web {
        let secure = state
            .config
            .public_origin
            .as_ref()
            .is_some_and(|origin| origin.starts_with("https://"));
        let cookie = format!(
            "luna_device={}; Path=/; HttpOnly; SameSite=Strict{}",
            paired.token,
            if secure { "; Secure" } else { "" }
        );
        response.headers_mut().insert(
            SET_COOKIE,
            HeaderValue::from_str(&cookie)
                .map_err(|_| AppError::InvalidRequest("Unable to set device cookie.".into()))?,
        );
    }
    Ok(response)
}

async fn bootstrap(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
) -> Result<Json<Bootstrap>, AppError> {
    Ok(Json(Bootstrap {
        protocol_version: PROTOCOL_VERSION,
        cursor: state.database.latest_cursor().await?,
        device,
        conversations: state.database.conversations(false).await?,
    }))
}

async fn list_conversations(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
) -> Result<Json<ConversationList>, AppError> {
    Ok(Json(ConversationList {
        conversations: state.database.conversations(false).await?,
    }))
}

async fn create_conversation(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Json(_request): Json<CreateConversationRequest>,
) -> Result<(StatusCode, Json<luna_protocol::Conversation>), AppError> {
    let created_at = now()?;
    let home = directories::BaseDirs::new()
        .ok_or_else(|| AppError::InvalidRequest("Home directory is unavailable.".into()))?;
    let conversation = state
        .database
        .create_conversation(
            Uuid::now_v7(),
            &home.home_dir().to_string_lossy(),
            &created_at,
        )
        .await?;
    state
        .events
        .append(
            Some(conversation.id),
            Some(conversation.id),
            &ServerEvent::ConversationUpserted(conversation.clone()),
            &created_at,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(conversation)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessagePageQuery {
    before_ordinal: Option<i64>,
    #[serde(default = "default_message_limit")]
    limit: i64,
}

const fn default_message_limit() -> i64 {
    50
}

async fn get_conversation(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
) -> Result<Json<luna_protocol::Conversation>, AppError> {
    Ok(Json(
        state
            .database
            .conversation(id)
            .await?
            .ok_or(AppError::NotFound)?,
    ))
}

async fn list_messages(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
    Query(query): Query<MessagePageQuery>,
) -> Result<Json<ConversationMessages>, AppError> {
    if state.database.conversation(id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    let limit = query.limit.clamp(1, 100);
    let messages = state
        .database
        .messages(id, query.before_ordinal, limit)
        .await?;
    let next_before_ordinal = (messages.len() == usize::try_from(limit).unwrap_or(100))
        .then(|| messages.first().map(|message| message.ordinal))
        .flatten();
    Ok(Json(ConversationMessages {
        messages,
        next_before_ordinal,
    }))
}

async fn update_conversation(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateConversationRequest>,
) -> Result<Json<luna_protocol::Conversation>, AppError> {
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty() && title.len() <= 120)
        .ok_or_else(|| AppError::InvalidRequest("A valid title is required.".into()))?;
    let updated_at = now()?;
    let conversation = state
        .database
        .rename_conversation(id, title, &updated_at)
        .await?;
    state
        .events
        .append(
            Some(id),
            Some(id),
            &ServerEvent::ConversationUpserted(conversation.clone()),
            &updated_at,
        )
        .await?;
    Ok(Json(conversation))
}

async fn send_message(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
    Json(request): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<SendMessageResponse>), AppError> {
    let message = accept_message(
        &state,
        device.id,
        id,
        request.client_message_id,
        request.text,
        request.attachment_ids,
    )
    .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(SendMessageResponse {
            accepted: true,
            message,
        }),
    ))
}

async fn accept_message(
    state: &AppState,
    device_id: Uuid,
    conversation_id: Uuid,
    client_message_id: Uuid,
    text: String,
    attachment_ids: Vec<Uuid>,
) -> Result<Message, AppError> {
    let text = text.trim();
    if text.is_empty() || text.len() > 100_000 {
        return Err(AppError::InvalidRequest(
            "A message between 1 and 100,000 characters is required.".into(),
        ));
    }
    let conversation = state
        .database
        .conversation(conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let delivery = if matches!(
        conversation.state,
        SessionState::Working | SessionState::Compacting | SessionState::Retrying
    ) {
        MessageDelivery::Steer
    } else {
        MessageDelivery::Initial
    };
    let accepted = state
        .database
        .accept_user_message(luna_storage::NewUserMessage {
            conversation_id,
            device_id,
            client_message_id,
            text,
            attachment_ids: &attachment_ids,
            delivery,
            accepted_at: &now()?,
        })
        .await?;
    for event in &accepted.events {
        state.events.publish(event.clone());
    }
    if accepted.dispatch_required {
        let runtime = state.runtime.clone();
        let dispatch_id = accepted.dispatch_id;
        let attachment_ids = accepted
            .message
            .attachments
            .iter()
            .map(|attachment| attachment.id)
            .collect();
        let text = text.to_owned();
        tokio::spawn(async move {
            runtime
                .dispatch(conversation_id, dispatch_id, text, attachment_ids, delivery)
                .await;
        });
    }
    Ok(accepted.message)
}

async fn abort_conversation(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    state.runtime.abort(id).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn archive_conversation(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let timestamp = now()?;
    state.database.archive_conversation(id, &timestamp).await?;
    state.runtime.deactivate(id).await;
    let conversation = state
        .database
        .conversation(id)
        .await?
        .ok_or(AppError::NotFound)?;
    state
        .events
        .append(
            Some(id),
            Some(id),
            &ServerEvent::ConversationUpserted(conversation),
            &timestamp,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SyncQuery {
    #[serde(default)]
    after: i64,
}

async fn sync(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncResponse>, AppError> {
    let events = state
        .database
        .events_after(query.after.max(0), 1_000)
        .await?;
    let cursor = events
        .last()
        .and_then(|event| event.event_id)
        .unwrap_or(query.after.max(0));
    Ok(Json(SyncResponse {
        cursor,
        events: events
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?,
        reset_required: false,
    }))
}

async fn events_socket(
    websocket: WebSocketUpgrade,
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
    Query(query): Query<SyncQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    validate_origin(&axum::http::Method::POST, &headers, &state)?;
    Ok(websocket
        .on_upgrade(move |socket| stream_events(socket, state, device.id, query.after.max(0))))
}

async fn stream_events(socket: WebSocket, state: AppState, device_id: Uuid, after: i64) {
    let (mut sender, mut receiver) = socket.split();
    let mut live = state.events.subscribe();
    let mut cursor = after;
    let latest = state.database.latest_cursor().await.unwrap_or(cursor);
    let welcome = ServerEventEnvelope {
        version: 1,
        event_id: None,
        conversation_id: None,
        emitted_at: now().unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        event: ServerEvent::ServerWelcome {
            cursor: latest,
            resumed: after > 0,
        },
    };
    if send_socket_event(&mut sender, &welcome).await.is_err() {
        return;
    }
    if send_catchup(&state, &mut sender, &mut cursor)
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            event = live.recv() => {
                match event {
                    Ok(event) => {
                        let event_cursor = event.event_id.unwrap_or(cursor);
                        if event_cursor <= cursor { continue; }
                        if send_socket_event(&mut sender, &event).await.is_err() { break; }
                        cursor = event_cursor;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if send_catchup(&state, &mut sender, &mut cursor).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            message = receiver.next() => {
                let Some(Ok(message)) = message else { break };
                match message {
                    WebSocketMessage::Text(text) => {
                        let response = match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(command) => handle_client_command(&state, device_id, command).await,
                            Err(_) => ServerEvent::CommandRejected(CommandRejected {
                                request_id: Uuid::nil(),
                                error: ApiError::new(ErrorCode::InvalidRequest, "The client command is invalid.", false),
                            }),
                        };
                        let envelope = ServerEventEnvelope {
                            version: 1,
                            event_id: None,
                            conversation_id: None,
                            emitted_at: now().unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
                            event: response,
                        };
                        if send_socket_event(&mut sender, &envelope).await.is_err() { break; }
                    }
                    WebSocketMessage::Close(_) => break,
                    WebSocketMessage::Ping(value) => {
                        let _ = sender.send(WebSocketMessage::Pong(value)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn handle_client_command(
    state: &AppState,
    device_id: Uuid,
    command: ClientCommand,
) -> ServerEvent {
    let (request_id, result): (Uuid, Result<Option<Message>, AppError>) = match command {
        ClientCommand::ClientPing { version, command } => {
            if version != PROTOCOL_VERSION {
                (
                    command.request_id,
                    Err(AppError::InvalidRequest(
                        "Protocol version mismatch.".into(),
                    )),
                )
            } else {
                return ServerEvent::ServerPong {
                    request_id: command.request_id,
                };
            }
        }
        ClientCommand::ClientHello { version, command } => {
            if version != PROTOCOL_VERSION {
                (
                    command.request_id,
                    Err(AppError::InvalidRequest(
                        "Protocol version mismatch.".into(),
                    )),
                )
            } else {
                return ServerEvent::ServerWelcome {
                    cursor: state
                        .database
                        .latest_cursor()
                        .await
                        .unwrap_or(command.last_cursor),
                    resumed: command.last_cursor > 0,
                };
            }
        }
        ClientCommand::MessageSend { version, command } => {
            let request_id = command.request_id;
            let result = if version != PROTOCOL_VERSION {
                Err(AppError::InvalidRequest(
                    "Protocol version mismatch.".into(),
                ))
            } else {
                accept_message(
                    state,
                    device_id,
                    command.conversation_id,
                    command.client_message_id,
                    command.text,
                    command.attachment_ids,
                )
                .await
                .map(Some)
            };
            (request_id, result)
        }
        ClientCommand::SessionInterrupt { version, command } => {
            let request_id = command.request_id;
            let result = if version != PROTOCOL_VERSION {
                Err(AppError::InvalidRequest(
                    "Protocol version mismatch.".into(),
                ))
            } else {
                state
                    .runtime
                    .abort(command.conversation_id)
                    .await
                    .map(|()| None)
            };
            (request_id, result)
        }
    };
    match result {
        Ok(message) => ServerEvent::CommandAccepted(CommandAccepted {
            request_id,
            message,
        }),
        Err(_) => ServerEvent::CommandRejected(CommandRejected {
            request_id,
            error: ApiError::new(
                ErrorCode::InternalError,
                "Luna could not accept the command.",
                true,
            ),
        }),
    }
}

async fn send_catchup(
    state: &AppState,
    sender: &mut futures_util::stream::SplitSink<WebSocket, WebSocketMessage>,
    cursor: &mut i64,
) -> Result<(), ()> {
    loop {
        let events = state
            .database
            .events_after(*cursor, 1_000)
            .await
            .map_err(|_| ())?;
        if events.is_empty() {
            return Ok(());
        }
        let count = events.len();
        for event in events {
            if let Some(event_id) = event.event_id {
                if event_id <= *cursor {
                    continue;
                }
                send_socket_event(sender, &event).await?;
                *cursor = event_id;
            }
        }
        if count < 1_000 {
            return Ok(());
        }
    }
}

async fn send_socket_event(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WebSocketMessage>,
    event: &ServerEventEnvelope,
) -> Result<(), ()> {
    let text = serde_json::to_string(event).map_err(|_| ())?;
    sender
        .send(WebSocketMessage::Text(text.into()))
        .await
        .map_err(|_| ())
}
