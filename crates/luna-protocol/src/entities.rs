use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::common::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Latte,
    Mocha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum DevicePlatform {
    Ios,
    Ipados,
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub name: String,
    pub platform: DevicePlatform,
    pub notifications_enabled: bool,
    pub created_at: Timestamp,
    pub last_seen_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Creating,
    Starting,
    Idle,
    Working,
    Compacting,
    Retrying,
    Crashed,
    Restoring,
    Interrupted,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Accepted,
    Queued,
    Streaming,
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MessageDelivery {
    Initial,
    Steer,
    Bash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentStatus {
    Uploading,
    Ready,
    Attached,
    Failed,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub width: u32,
    pub height: u32,
    pub status: AttachmentStatus,
    pub content_url: String,
    pub thumbnail_url: String,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_message_id: Option<Uuid>,
    pub role: MessageRole,
    pub status: MessageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<MessageDelivery>,
    pub text: String,
    pub attachments: Vec<Attachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_by_device_id: Option<Uuid>,
    pub ordinal: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryIcon {
    pub repository_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_url: Option<String>,
    pub fallback_text: String,
    pub fallback_color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub id: Uuid,
    pub display_name: String,
    pub root_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub active: bool,
    pub icon: RepositoryIcon,
    pub first_seen_at: Timestamp,
    pub last_seen_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentTask {
    pub id: Uuid,
    pub sequence: i64,
    pub text: String,
    pub status: AgentTaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentTaskList {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub revision: i64,
    pub tasks: Vec<AgentTask>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivity {
    pub id: Uuid,
    pub sequence: i64,
    pub summary: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TitleMode {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub title_mode: TitleMode,
    pub state: SessionState,
    pub preview: String,
    pub active_working_directory: String,
    pub repositories: Vec<Repository>,
    pub activities: Vec<AgentActivity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_list: Option<AgentTaskList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_target_device_id: Option<Uuid>,
    pub unread_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub protocol_version: u8,
    pub cursor: i64,
    pub device: Device,
    pub conversations: Vec<Conversation>,
}
