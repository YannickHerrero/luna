use std::collections::HashMap;

use luna_protocol::{
    AgentActivity, Bootstrap, Conversation, ConversationMessages, Message, MessageStatus,
    ServerEvent, ServerEventEnvelope,
};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct ClientState {
    pub conversations: Vec<Conversation>,
    pub messages: HashMap<Uuid, Vec<Message>>,
    pub next_before_ordinal: HashMap<Uuid, i64>,
    pub selected_conversation_id: Option<Uuid>,
    pub cursor: i64,
}

impl ClientState {
    #[must_use]
    pub fn from_bootstrap(bootstrap: Bootstrap) -> Self {
        let mut state = Self::default();
        state.install(bootstrap);
        state
    }

    pub fn install(&mut self, bootstrap: Bootstrap) {
        let previous_selection = self.selected_conversation_id;
        self.conversations = bootstrap.conversations;
        sort_conversations(&mut self.conversations);
        self.messages.clear();
        self.next_before_ordinal.clear();
        self.selected_conversation_id = previous_selection
            .filter(|id| self.conversations.iter().any(|item| item.id == *id))
            .or_else(|| self.conversations.first().map(|item| item.id));
        self.cursor = bootstrap.cursor;
    }

    #[must_use]
    pub fn selected_conversation(&self) -> Option<&Conversation> {
        let selected = self.selected_conversation_id?;
        self.conversations
            .iter()
            .find(|conversation| conversation.id == selected)
    }

    #[must_use]
    pub fn selected_messages(&self) -> &[Message] {
        self.selected_conversation_id
            .and_then(|id| self.messages.get(&id).map(Vec::as_slice))
            .unwrap_or_default()
    }

    pub fn select(&mut self, conversation_id: Uuid) -> bool {
        if self
            .conversations
            .iter()
            .any(|conversation| conversation.id == conversation_id)
        {
            self.selected_conversation_id = Some(conversation_id);
            true
        } else {
            false
        }
    }

    pub fn upsert_conversation(&mut self, conversation: Conversation) {
        if conversation.archived_at.is_some() {
            self.remove_conversation(conversation.id);
            return;
        }
        if let Some(index) = self
            .conversations
            .iter()
            .position(|item| item.id == conversation.id)
        {
            self.conversations[index] = conversation;
        } else {
            self.conversations.push(conversation);
        }
        sort_conversations(&mut self.conversations);
        if self.selected_conversation_id.is_none() {
            self.selected_conversation_id = self.conversations.first().map(|item| item.id);
        }
    }

    pub fn set_messages(&mut self, conversation_id: Uuid, page: ConversationMessages) {
        let current = self.messages.entry(conversation_id).or_default();
        for message in page.messages {
            upsert_message(current, message);
        }
        current.sort_by_key(|message| message.ordinal);
        if let Some(next) = page.next_before_ordinal {
            self.next_before_ordinal.insert(conversation_id, next);
        } else {
            self.next_before_ordinal.remove(&conversation_id);
        }
    }

    pub fn upsert_message(&mut self, message: Message) {
        self.update_conversation_recency(&message);
        let should_store = self.selected_conversation_id == Some(message.conversation_id)
            || self.messages.contains_key(&message.conversation_id);
        if should_store {
            let messages = self.messages.entry(message.conversation_id).or_default();
            upsert_message(messages, message);
            messages.sort_by_key(|item| item.ordinal);
        }
    }

    pub fn apply(&mut self, envelope: ServerEventEnvelope) -> StateEffect {
        if let Some(event_id) = envelope.event_id {
            self.cursor = self.cursor.max(event_id);
        }
        let conversation_id = envelope.conversation_id;
        match envelope.event {
            ServerEvent::ConversationUpserted(conversation) => {
                self.upsert_conversation(conversation);
            }
            ServerEvent::ConversationTitleUpdated(update) => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.title = update.title;
                });
            }
            ServerEvent::MessageUpserted(message) => self.upsert_message(message),
            ServerEvent::MessageDelta(delta) => {
                self.update_message(conversation_id, delta.message_id, |message| {
                    message.text.push_str(&delta.delta);
                    message.status = MessageStatus::Streaming;
                });
            }
            ServerEvent::MessageCompleted(completed) => {
                self.update_message(conversation_id, completed.message_id, |message| {
                    message.status = MessageStatus::Completed;
                });
            }
            ServerEvent::SessionStateChanged { state } => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.state = state;
                });
            }
            ServerEvent::AgentActivitiesReset(_) => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.activities.clear();
                });
            }
            ServerEvent::AgentActivityUpserted(activity) => {
                self.update_conversation(conversation_id, |conversation| {
                    upsert_activity(&mut conversation.activities, activity);
                });
            }
            ServerEvent::AgentTaskListChanged(change) => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.task_list = change.task_list;
                });
            }
            ServerEvent::WorkspaceUpdated(update) => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.active_working_directory = update.working_directory;
                });
            }
            ServerEvent::RepositoriesUpdated(update) => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.repositories = update.repositories;
                });
            }
            ServerEvent::NotificationTargetChanged(update) => {
                self.update_conversation(conversation_id, |conversation| {
                    conversation.notification_target_device_id = update.device_id;
                });
            }
            ServerEvent::SyncResetRequired { .. } => return StateEffect::ResetRequired,
            ServerEvent::CommandRejected(rejection) => {
                return StateEffect::Error(rejection.error.message);
            }
            ServerEvent::Error(error) => return StateEffect::Error(error.message),
            ServerEvent::ServerWelcome { .. }
            | ServerEvent::CommandAccepted(_)
            | ServerEvent::AgentActivityChanged(_)
            | ServerEvent::SteeringQueueChanged(_)
            | ServerEvent::AttachmentUpdated(_)
            | ServerEvent::ServerPong { .. } => {}
        }
        StateEffect::None
    }

    fn remove_conversation(&mut self, conversation_id: Uuid) {
        self.conversations
            .retain(|conversation| conversation.id != conversation_id);
        self.messages.remove(&conversation_id);
        self.next_before_ordinal.remove(&conversation_id);
        if self.selected_conversation_id == Some(conversation_id) {
            self.selected_conversation_id = self.conversations.first().map(|item| item.id);
        }
    }

    fn update_conversation(
        &mut self,
        conversation_id: Option<Uuid>,
        update: impl FnOnce(&mut Conversation),
    ) {
        let Some(conversation_id) = conversation_id else {
            return;
        };
        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == conversation_id)
        {
            update(conversation);
        }
    }

    fn update_message(
        &mut self,
        conversation_id: Option<Uuid>,
        message_id: Uuid,
        update: impl FnOnce(&mut Message),
    ) {
        if let Some(conversation_id) = conversation_id
            && let Some(message) = self
                .messages
                .get_mut(&conversation_id)
                .and_then(|messages| messages.iter_mut().find(|message| message.id == message_id))
        {
            update(message);
            return;
        }
        for messages in self.messages.values_mut() {
            if let Some(message) = messages.iter_mut().find(|message| message.id == message_id) {
                update(message);
                return;
            }
        }
    }

    fn update_conversation_recency(&mut self, message: &Message) {
        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == message.conversation_id)
            && conversation
                .last_message_at
                .as_ref()
                .is_none_or(|current| current < &message.created_at)
        {
            conversation.last_message_at = Some(message.created_at.clone());
            sort_conversations(&mut self.conversations);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateEffect {
    None,
    ResetRequired,
    Error(String),
}

fn upsert_message(messages: &mut Vec<Message>, message: Message) {
    if let Some(index) = messages.iter().position(|item| item.id == message.id) {
        messages[index] = message;
    } else {
        messages.push(message);
    }
}

fn upsert_activity(activities: &mut Vec<AgentActivity>, activity: AgentActivity) {
    if let Some(index) = activities.iter().position(|item| item.id == activity.id) {
        activities[index] = activity;
    } else {
        activities.push(activity);
    }
    activities.sort_by_key(|item| item.sequence);
}

fn sort_conversations(conversations: &mut [Conversation]) {
    conversations.sort_by(|left, right| {
        let left_date = left.last_message_at.as_ref().unwrap_or(&left.created_at);
        let right_date = right.last_message_at.as_ref().unwrap_or(&right.created_at);
        right_date
            .cmp(left_date)
            .then_with(|| right.id.cmp(&left.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_bootstrap_and_merges_message_pages() {
        let mut state = ClientState::from_bootstrap(bootstrap());
        let selected = state.selected_conversation_id.expect("selection");
        assert_eq!(state.conversations[0].title, "Recent");

        state.set_messages(
            selected,
            serde_json::from_value(serde_json::json!({
                "messages": [message(selected, 2, "two", "completed")],
                "nextBeforeOrdinal": 2
            }))
            .expect("page"),
        );
        state.set_messages(
            selected,
            serde_json::from_value(serde_json::json!({
                "messages": [message(selected, 1, "one", "completed")]
            }))
            .expect("page"),
        );

        assert_eq!(
            state
                .selected_messages()
                .iter()
                .map(|message| message.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(!state.next_before_ordinal.contains_key(&selected));
    }

    #[test]
    fn applies_streaming_completion_and_archival_events() {
        let mut state = ClientState::from_bootstrap(bootstrap());
        let selected = state.selected_conversation_id.expect("selection");
        state.set_messages(
            selected,
            serde_json::from_value(serde_json::json!({
                "messages": [message(selected, 1, "Hello", "streaming")]
            }))
            .expect("page"),
        );
        let message_id = state.selected_messages()[0].id;

        assert_eq!(
            state.apply(event(
                11,
                selected,
                "message.delta",
                serde_json::json!({
                    "messageId": message_id,
                    "chunkIndex": 1,
                    "delta": " world"
                })
            )),
            StateEffect::None
        );
        state.apply(event(
            12,
            selected,
            "message.completed",
            serde_json::json!({"messageId": message_id}),
        ));
        assert_eq!(state.selected_messages()[0].text, "Hello world");
        assert_eq!(
            state.selected_messages()[0].status,
            MessageStatus::Completed
        );
        assert_eq!(state.cursor, 12);

        let mut archived = state.selected_conversation().expect("conversation").clone();
        archived.archived_at = Some("2026-01-03T00:00:00Z".into());
        state.apply(event(
            13,
            selected,
            "conversation.upserted",
            serde_json::to_value(archived).expect("conversation JSON"),
        ));
        assert!(!state.conversations.iter().any(|item| item.id == selected));
    }

    #[test]
    fn reports_reset_and_server_errors() {
        let mut state = ClientState::from_bootstrap(bootstrap());
        let selected = state.selected_conversation_id.expect("selection");
        assert_eq!(
            state.apply(event(
                0,
                selected,
                "sync.reset_required",
                serde_json::json!({"cursor": 99})
            )),
            StateEffect::ResetRequired
        );
        assert_eq!(
            state.apply(event(
                14,
                selected,
                "error",
                serde_json::json!({
                    "code": "internal_error",
                    "message": "try again",
                    "retryable": true
                })
            )),
            StateEffect::Error("try again".into())
        );
    }

    fn bootstrap() -> Bootstrap {
        serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "cursor": 10,
            "device": {
                "id": Uuid::nil(),
                "name": "Terminal",
                "platform": "tui",
                "notificationsEnabled": false,
                "createdAt": "2026-01-01T00:00:00Z",
                "lastSeenAt": "2026-01-01T00:00:00Z"
            },
            "conversations": [
                conversation("00000000-0000-0000-0000-000000000001", "Older", "2026-01-01T00:00:00Z"),
                conversation("00000000-0000-0000-0000-000000000002", "Recent", "2026-01-02T00:00:00Z")
            ]
        }))
        .expect("bootstrap")
    }

    fn conversation(id: &str, title: &str, updated_at: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": title,
            "titleMode": "automatic",
            "state": "idle",
            "preview": "",
            "activeWorkingDirectory": "/tmp",
            "repositories": [],
            "activities": [],
            "unreadCount": 0,
            "createdAt": updated_at,
            "updatedAt": updated_at,
            "version": 1
        })
    }

    fn message(conversation_id: Uuid, ordinal: i64, text: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "id": Uuid::now_v7(),
            "conversationId": conversation_id,
            "role": "assistant",
            "status": status,
            "text": text,
            "attachments": [],
            "ordinal": ordinal,
            "createdAt": format!("2026-01-02T00:00:0{ordinal}Z"),
            "updatedAt": format!("2026-01-02T00:00:0{ordinal}Z")
        })
    }

    fn event(
        event_id: i64,
        conversation_id: Uuid,
        event_type: &str,
        payload: serde_json::Value,
    ) -> ServerEventEnvelope {
        serde_json::from_value(serde_json::json!({
            "version": 1,
            "eventId": (event_id > 0).then_some(event_id),
            "conversationId": conversation_id,
            "emittedAt": "2026-01-02T00:00:00Z",
            "type": event_type,
            "payload": payload
        }))
        .expect("event")
    }
}
