use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AgentActivity, ApiError, Attachment, Conversation, Message, MessageDelivery, Repository,
    SessionState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientHello {
    pub request_id: Uuid,
    pub device_id: Uuid,
    pub last_cursor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageSendCommand {
    pub request_id: Uuid,
    pub conversation_id: Uuid,
    pub client_message_id: Uuid,
    pub text: String,
    pub attachment_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionInterruptCommand {
    pub request_id: Uuid,
    pub conversation_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientPing {
    pub request_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommand {
    #[serde(rename = "client.hello")]
    ClientHello {
        version: u8,
        #[serde(flatten)]
        command: ClientHello,
    },
    #[serde(rename = "message.send")]
    MessageSend {
        version: u8,
        #[serde(flatten)]
        command: MessageSendCommand,
    },
    #[serde(rename = "session.interrupt")]
    SessionInterrupt {
        version: u8,
        #[serde(flatten)]
        command: SessionInterruptCommand,
    },
    #[serde(rename = "client.ping")]
    ClientPing {
        version: u8,
        #[serde(flatten)]
        command: ClientPing,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandAccepted {
    pub request_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandRejected {
    pub request_id: Uuid,
    pub error: ApiError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageDelta {
    pub message_id: Uuid,
    pub chunk_index: i64,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageCompleted {
    pub message_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityPhase {
    Thinking,
    Working,
    Compacting,
    Retrying,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivityChanged {
    pub active: bool,
    pub phase: ActivityPhase,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AgentActivitiesReset {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SteeringQueueChanged {
    pub pending: i64,
    pub delivery: MessageDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUpdated {
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoriesUpdated {
    pub repositories: Vec<Repository>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTitleUpdated {
    pub title: String,
    pub automatic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTargetChanged {
    pub device_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "type", content = "payload")]
pub enum ServerEvent {
    #[serde(rename = "server.welcome")]
    ServerWelcome { cursor: i64, resumed: bool },
    #[serde(rename = "command.accepted")]
    CommandAccepted(CommandAccepted),
    #[serde(rename = "command.rejected")]
    CommandRejected(CommandRejected),
    #[serde(rename = "conversation.upserted")]
    ConversationUpserted(Conversation),
    #[serde(rename = "conversation.title_updated")]
    ConversationTitleUpdated(ConversationTitleUpdated),
    #[serde(rename = "message.upserted")]
    MessageUpserted(Message),
    #[serde(rename = "message.delta")]
    MessageDelta(MessageDelta),
    #[serde(rename = "message.completed")]
    MessageCompleted(MessageCompleted),
    #[serde(rename = "session.state_changed")]
    SessionStateChanged { state: SessionState },
    #[serde(rename = "agent.activity_changed")]
    AgentActivityChanged(AgentActivityChanged),
    #[serde(rename = "agent.activities_reset")]
    AgentActivitiesReset(AgentActivitiesReset),
    #[serde(rename = "agent.activity_upserted")]
    AgentActivityUpserted(AgentActivity),
    #[serde(rename = "steering.queue_changed")]
    SteeringQueueChanged(SteeringQueueChanged),
    #[serde(rename = "workspace.updated")]
    WorkspaceUpdated(WorkspaceUpdated),
    #[serde(rename = "repositories.updated")]
    RepositoriesUpdated(RepositoriesUpdated),
    #[serde(rename = "attachment.updated")]
    AttachmentUpdated(Attachment),
    #[serde(rename = "notification_target.changed")]
    NotificationTargetChanged(NotificationTargetChanged),
    #[serde(rename = "sync.reset_required")]
    SyncResetRequired { cursor: i64 },
    #[serde(rename = "error")]
    Error(ApiError),
    #[serde(rename = "server.pong")]
    ServerPong { request_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerEventEnvelope {
    pub version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,
    pub emitted_at: String,
    #[serde(flatten)]
    pub event: ServerEvent,
}
