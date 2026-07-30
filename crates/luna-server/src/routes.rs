use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::IntoResponse,
    routing::{get, post},
};
use luna_protocol::{
    Bootstrap, ConversationList, CreateConversationRequest, PROTOCOL_VERSION,
    PairingExchangeRequest, PairingExchangeResponse, ServerEvent, SyncResponse,
    UpdateConversationRequest,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::now,
    error::AppError,
    extract::{AuthenticatedDevice, validate_origin, validate_tailnet},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health/live", get(health))
        .route("/v1/pairing/exchange", post(pair))
        .route("/v1/bootstrap", get(bootstrap))
        .route("/v1/sync", get(sync))
        .route(
            "/v1/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/v1/conversations/{id}",
            get(get_conversation).patch(update_conversation),
        )
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
        .database
        .append_event(
            Some(conversation.id),
            Some(conversation.id),
            &ServerEvent::ConversationUpserted(conversation.clone()),
            &created_at,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(conversation)))
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
        .database
        .append_event(
            Some(id),
            Some(id),
            &ServerEvent::ConversationUpserted(conversation.clone()),
            &updated_at,
        )
        .await?;
    Ok(Json(conversation))
}

async fn archive_conversation(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    state.database.archive_conversation(id, &now()?).await?;
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
