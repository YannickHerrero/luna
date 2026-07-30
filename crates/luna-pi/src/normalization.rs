use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum NormalizedPiEvent {
    AgentStarted,
    AgentSettled,
    TextDelta { content_index: u64, delta: String },
    MessageEnded { message: Value },
    ToolStarted,
    ToolEnded { failed: bool },
    QueueUpdated { steering: usize, follow_up: usize },
    CompactionStarted,
    CompactionEnded,
    RetryStarted,
    RetryEnded { succeeded: bool },
    ExtensionUiRequest { request: Value },
    Unknown,
}

#[must_use]
pub fn normalize_event(value: &Value) -> NormalizedPiEvent {
    match value.get("type").and_then(Value::as_str) {
        Some("agent_start") => NormalizedPiEvent::AgentStarted,
        Some("agent_settled") => NormalizedPiEvent::AgentSettled,
        Some("message_update") => {
            let Some(delta) = value.get("assistantMessageEvent") else {
                return NormalizedPiEvent::Unknown;
            };
            if delta.get("type").and_then(Value::as_str) != Some("text_delta") {
                return NormalizedPiEvent::Unknown;
            }
            NormalizedPiEvent::TextDelta {
                content_index: delta
                    .get("contentIndex")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                delta: delta
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }
        }
        Some("message_end") => NormalizedPiEvent::MessageEnded {
            message: value.get("message").cloned().unwrap_or(Value::Null),
        },
        Some("tool_execution_start") => NormalizedPiEvent::ToolStarted,
        Some("tool_execution_end") => NormalizedPiEvent::ToolEnded {
            failed: value
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        Some("queue_update") => NormalizedPiEvent::QueueUpdated {
            steering: value
                .get("steering")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            follow_up: value
                .get("followUp")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        },
        Some("compaction_start") => NormalizedPiEvent::CompactionStarted,
        Some("compaction_end") => NormalizedPiEvent::CompactionEnded,
        Some("auto_retry_start") => NormalizedPiEvent::RetryStarted,
        Some("auto_retry_end") => NormalizedPiEvent::RetryEnded {
            succeeded: value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        Some("extension_ui_request") => NormalizedPiEvent::ExtensionUiRequest {
            request: value.clone(),
        },
        _ => NormalizedPiEvent::Unknown,
    }
}
