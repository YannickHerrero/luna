#![allow(dead_code)]

use utoipa::OpenApi;

use crate::{
    AgentActivitiesReset, AgentActivity, AgentActivityChanged, AgentModel, AgentModelSelection,
    AgentTask, AgentTaskList, AgentTaskListChanged, AgentTaskStatus, ApiError, Attachment,
    AttachmentResponse, Bootstrap, ClientCommand, CommandAccepted, CommandRejected,
    CompactConversationResponse, ContextUsage, Conversation, ConversationAgentState,
    ConversationList, ConversationMessages, ConversationScope, ConversationTitleUpdated,
    CreateConversationRequest, Device, ErrorCode, ErrorResponse, Message, MessageCompleted,
    MessageDelta, OpenAiUsageAvailability, OpenAiWeeklyUsage, PairingCodeRequestResponse,
    PairingExchangeRequest, PairingExchangeResponse, RepositoriesUpdated, Repository,
    RepositoryIcon, SendMessageRequest, SendMessageResponse, ServerEvent, ServerEventEnvelope,
    SessionState, SteeringQueueChanged, SyncResponse, ThinkingLevel, TranscriptionResponse,
    UpdateConversationAgentRequest, UpdateConversationRequest, WorkspaceUpdated,
};

#[utoipa::path(
    get,
    path = "/v1/health/live",
    responses((status = 200, description = "The Luna process is alive"))
)]
fn health_live() {}

#[utoipa::path(
    get,
    path = "/v1/health/ready",
    responses((status = 200, description = "Luna storage and runtime dependencies are ready"))
)]
fn health_ready() {}

#[utoipa::path(
    post,
    path = "/v1/pairing/request",
    responses(
        (status = 202, body = PairingCodeRequestResponse),
        (status = 403, body = ApiError)
    )
)]
fn pairing_request() {}

#[utoipa::path(
    post,
    path = "/v1/pairing/exchange",
    request_body = PairingExchangeRequest,
    responses(
        (status = 201, body = PairingExchangeResponse),
        (status = 400, body = ApiError)
    )
)]
fn pairing_exchange() {}

#[utoipa::path(
    get,
    path = "/v1/bootstrap",
    responses((status = 200, body = Bootstrap), (status = 401, body = ApiError)),
    security(("deviceToken" = []))
)]
fn bootstrap() {}

#[utoipa::path(
    get,
    path = "/v1/account/openai-usage",
    responses((status = 200, body = OpenAiWeeklyUsage), (status = 401, body = ApiError)),
    security(("deviceToken" = []))
)]
fn openai_weekly_usage() {}

#[utoipa::path(
    get,
    path = "/v1/sync",
    params(("after" = Option<i64>, Query, description = "Last applied event cursor")),
    responses((status = 200, body = SyncResponse)),
    security(("deviceToken" = []))
)]
fn sync() {}

#[utoipa::path(
    get,
    path = "/v1/conversations",
    params(("scope" = Option<ConversationScope>, Query, description = "Active, archived, or all conversations")),
    responses((status = 200, body = ConversationList)),
    security(("deviceToken" = []))
)]
fn conversations_list() {}

#[utoipa::path(
    post,
    path = "/v1/conversations",
    request_body = CreateConversationRequest,
    responses((status = 201, body = Conversation)),
    security(("deviceToken" = []))
)]
fn conversations_create() {}

#[utoipa::path(
    get,
    path = "/v1/conversations/{id}",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 200, body = Conversation), (status = 404, body = ApiError)),
    security(("deviceToken" = []))
)]
fn conversation_get() {}

#[utoipa::path(
    patch,
    path = "/v1/conversations/{id}",
    params(("id" = uuid::Uuid, Path)),
    request_body = UpdateConversationRequest,
    responses((status = 200, body = Conversation), (status = 404, body = ApiError)),
    security(("deviceToken" = []))
)]
fn conversation_update() {}

#[utoipa::path(
    get,
    path = "/v1/conversations/{id}/messages",
    params(
        ("id" = uuid::Uuid, Path),
        ("beforeOrdinal" = Option<i64>, Query),
        ("limit" = Option<i64>, Query)
    ),
    responses((status = 200, body = ConversationMessages)),
    security(("deviceToken" = []))
)]
fn messages_list() {}

#[utoipa::path(
    post,
    path = "/v1/conversations/{id}/messages",
    params(("id" = uuid::Uuid, Path)),
    request_body = SendMessageRequest,
    responses((status = 202, body = SendMessageResponse)),
    security(("deviceToken" = []))
)]
fn messages_send() {}

#[utoipa::path(
    get,
    path = "/v1/conversations/{id}/agent",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 200, body = ConversationAgentState), (status = 404, body = ApiError)),
    security(("deviceToken" = []))
)]
fn conversation_agent_get() {}

#[utoipa::path(
    patch,
    path = "/v1/conversations/{id}/agent",
    params(("id" = uuid::Uuid, Path)),
    request_body = UpdateConversationAgentRequest,
    responses(
        (status = 200, body = ConversationAgentState),
        (status = 400, body = ApiError),
        (status = 409, body = ApiError)
    ),
    security(("deviceToken" = []))
)]
fn conversation_agent_update() {}

#[utoipa::path(
    post,
    path = "/v1/conversations/{id}/compact",
    params(("id" = uuid::Uuid, Path)),
    responses(
        (status = 200, body = CompactConversationResponse),
        (status = 409, body = ApiError)
    ),
    security(("deviceToken" = []))
)]
fn conversation_compact() {}

#[utoipa::path(
    post,
    path = "/v1/conversations/{id}/abort",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 202, description = "The active Pi operation was interrupted")),
    security(("deviceToken" = []))
)]
fn conversation_abort() {}

#[utoipa::path(
    post,
    path = "/v1/conversations/{id}/archive",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 204, description = "The conversation was archived")),
    security(("deviceToken" = []))
)]
fn conversation_archive() {}

#[utoipa::path(
    post,
    path = "/v1/conversations/{id}/restore",
    params(("id" = uuid::Uuid, Path)),
    responses(
        (status = 200, body = Conversation),
        (status = 404, body = ApiError)
    ),
    security(("deviceToken" = []))
)]
fn conversation_restore() {}

#[utoipa::path(
    post,
    path = "/v1/attachments",
    request_body(content = String, content_type = "multipart/form-data"),
    responses((status = 201, body = AttachmentResponse)),
    security(("deviceToken" = []))
)]
fn attachments_upload() {}

#[utoipa::path(
    get,
    path = "/v1/attachments/{id}/content",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 200, description = "Original image bytes")),
    security(("deviceToken" = []))
)]
fn attachment_content() {}

#[utoipa::path(
    get,
    path = "/v1/attachments/{id}/thumbnail",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 200, description = "JPEG thumbnail bytes")),
    security(("deviceToken" = []))
)]
fn attachment_thumbnail() {}

#[utoipa::path(
    get,
    path = "/v1/repositories/{id}/icon",
    params(("id" = uuid::Uuid, Path)),
    responses((status = 200, description = "Detected repository icon bytes")),
    security(("deviceToken" = []))
)]
fn repository_icon() {}

#[utoipa::path(
    post,
    path = "/v1/transcriptions",
    request_body(content = String, content_type = "multipart/form-data"),
    responses((status = 200, body = TranscriptionResponse)),
    security(("deviceToken" = []))
)]
fn transcriptions_create() {}

#[derive(OpenApi)]
#[openapi(
    info(title = "Luna API", version = "1.0.0"),
    paths(
        health_live,
        health_ready,
        pairing_request,
        pairing_exchange,
        bootstrap,
        openai_weekly_usage,
        sync,
        conversations_list,
        conversations_create,
        conversation_get,
        conversation_update,
        messages_list,
        messages_send,
        conversation_agent_get,
        conversation_agent_update,
        conversation_compact,
        conversation_abort,
        conversation_archive,
        conversation_restore,
        attachments_upload,
        attachment_content,
        attachment_thumbnail,
        repository_icon,
        transcriptions_create
    ),
    components(schemas(
        AgentActivitiesReset,
        AgentActivity,
        AgentActivityChanged,
        AgentModel,
        AgentModelSelection,
        AgentTask,
        AgentTaskList,
        AgentTaskListChanged,
        AgentTaskStatus,
        ApiError,
        Attachment,
        AttachmentResponse,
        Bootstrap,
        ClientCommand,
        CommandAccepted,
        CommandRejected,
        CompactConversationResponse,
        ContextUsage,
        Conversation,
        ConversationAgentState,
        ConversationList,
        ConversationMessages,
        ConversationScope,
        ConversationTitleUpdated,
        CreateConversationRequest,
        Device,
        ErrorCode,
        ErrorResponse,
        Message,
        MessageCompleted,
        MessageDelta,
        OpenAiUsageAvailability,
        OpenAiWeeklyUsage,
        PairingCodeRequestResponse,
        PairingExchangeRequest,
        PairingExchangeResponse,
        RepositoriesUpdated,
        Repository,
        RepositoryIcon,
        SendMessageRequest,
        SendMessageResponse,
        ServerEvent,
        ServerEventEnvelope,
        SessionState,
        SteeringQueueChanged,
        SyncResponse,
        ThinkingLevel,
        TranscriptionResponse,
        UpdateConversationAgentRequest,
        UpdateConversationRequest,
        WorkspaceUpdated
    )),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "deviceToken",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization"))),
            );
        }
    }
}

#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
