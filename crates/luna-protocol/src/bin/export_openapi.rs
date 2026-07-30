use std::{env, fs, path::PathBuf};

use luna_protocol::{
    AgentActivityChanged, ApiError, Attachment, AttachmentResponse, Bootstrap, ClientCommand,
    CommandAccepted, CommandRejected, Conversation, ConversationList, ConversationMessages,
    ConversationTitleUpdated, CreateConversationRequest, Device, ErrorCode, ErrorResponse, Message,
    MessageCompleted, MessageDelta, PairingExchangeRequest, PairingExchangeResponse,
    RepositoriesUpdated, Repository, RepositoryIcon, SendMessageRequest, SendMessageResponse,
    ServerEvent, ServerEventEnvelope, SessionState, SteeringQueueChanged, SyncResponse,
    TranscriptionResponse, UpdateConversationRequest, WorkspaceUpdated,
};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Luna API", version = "1.0.0"),
    components(schemas(
        AgentActivityChanged,
        ApiError,
        Attachment,
        AttachmentResponse,
        Bootstrap,
        ClientCommand,
        CommandAccepted,
        CommandRejected,
        Conversation,
        ConversationList,
        ConversationMessages,
        ConversationTitleUpdated,
        CreateConversationRequest,
        Device,
        ErrorCode,
        ErrorResponse,
        Message,
        MessageCompleted,
        MessageDelta,
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
        TranscriptionResponse,
        UpdateConversationRequest,
        WorkspaceUpdated
    ))
)]
struct ApiDoc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("packages/protocol/generated/openapi.json"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_string_pretty(&ApiDoc::openapi())? + "\n",
    )?;
    Ok(())
}
