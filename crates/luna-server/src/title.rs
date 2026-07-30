use std::{path::PathBuf, process::Stdio, time::Duration};

use luna_protocol::{Message, MessageRole};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

const TITLE_SYSTEM_PROMPT: &str = "You create concise conversation titles. Identify the actual theme of the conversation instead of copying a request verbatim. Return only a plain-text title of 3 to 7 words, with no quotes, Markdown, label, or trailing punctuation.";
const MAX_CONTEXT_CHARS: usize = 12_000;
const MAX_MESSAGE_CHARS: usize = 2_000;
const MAX_TITLE_CHARS: usize = 80;

#[derive(Clone)]
pub struct TitleGenerator {
    executable: PathBuf,
    model: String,
    request_timeout: Duration,
}

impl TitleGenerator {
    pub fn new(executable: PathBuf, model: String, request_timeout: Duration) -> Self {
        Self {
            executable,
            model,
            request_timeout,
        }
    }

    pub async fn generate(&self, messages: &[Message]) -> Result<Option<String>, TitleError> {
        let Some(context) = title_context(messages) else {
            return Ok(None);
        };
        let prompt = format!(
            "Create a title for this conversation.\n\nConversation:\n{context}\n\nReturn only the title."
        );
        let mut command = Command::new(&self.executable);
        command
            .arg("--print")
            .arg("--no-session")
            .arg("--model")
            .arg(&self.model)
            .arg("--thinking")
            .arg("off")
            .arg("--no-tools")
            .arg("--no-extensions")
            .arg("--no-skills")
            .arg("--no-prompt-templates")
            .arg("--no-context-files")
            .arg("--no-approve")
            .arg("--system-prompt")
            .arg(TITLE_SYSTEM_PROMPT)
            .arg("Create a title from the conversation supplied on stdin. Return only the title.")
            .env("PI_SKIP_VERSION_CHECK", "1")
            .env("PI_TELEMETRY", "0")
            .env_remove("LUNA_BRIDGE_SOCKET")
            .env_remove("LUNA_WORKING_DIRECTORY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let mut stdin = child.stdin.take().ok_or(TitleError::MissingStdin)?;
        stdin.write_all(prompt.as_bytes()).await?;
        drop(stdin);
        let output = timeout(self.request_timeout, child.wait_with_output())
            .await
            .map_err(|_| TitleError::Timeout)??;
        if !output.status.success() {
            return Err(TitleError::Rejected(output.status.code()));
        }
        let response = String::from_utf8(output.stdout)?;
        Ok(sanitize_title(&response))
    }
}

fn title_context(messages: &[Message]) -> Option<String> {
    let mut context = String::new();
    for message in messages.iter().rev().take(8).rev() {
        let text = message.text.trim();
        if text.is_empty() {
            continue;
        }
        let role = match message.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
        };
        let excerpt = text.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
        let entry = format!("{role}: {excerpt}\n");
        let remaining = MAX_CONTEXT_CHARS.saturating_sub(context.chars().count());
        if remaining == 0 {
            break;
        }
        context.extend(entry.chars().take(remaining));
    }
    (!context.trim().is_empty()).then_some(context)
}

fn sanitize_title(value: &str) -> Option<String> {
    let line = value.lines().find(|line| !line.trim().is_empty())?.trim();
    let line = line
        .strip_prefix("Title:")
        .or_else(|| line.strip_prefix("title:"))
        .unwrap_or(line)
        .trim()
        .trim_matches(['"', '\'', '#', '*', '_', '`', ' '])
        .trim_end_matches(['.', '!', '?', ':', ';'])
        .trim_end_matches(['"', '\'', '#', '*', '_', '`', ' '])
        .trim();
    if line.is_empty() || line.eq_ignore_ascii_case("New Conversation") {
        return None;
    }
    let mut title = String::new();
    for word in line.split_whitespace() {
        let separator = usize::from(!title.is_empty());
        if title.chars().count() + word.chars().count() + separator > MAX_TITLE_CHARS {
            break;
        }
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(word);
    }
    (!title.is_empty()).then_some(title)
}

#[derive(Debug, thiserror::Error)]
pub enum TitleError {
    #[error("title generation timed out")]
    Timeout,
    #[error("title model exited unsuccessfully with code {0:?}")]
    Rejected(Option<i32>),
    #[error("title model stdin is unavailable")]
    MissingStdin,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}

#[cfg(test)]
mod tests {
    use luna_protocol::{Message, MessageRole, MessageStatus};
    use uuid::Uuid;

    use super::{sanitize_title, title_context};

    #[test]
    fn sanitizes_plain_model_titles() {
        assert_eq!(
            sanitize_title("Title: **Conversation-Specific Message Drafts.**\n"),
            Some("Conversation-Specific Message Drafts".into())
        );
        assert_eq!(sanitize_title("New Conversation"), None);
    }

    #[test]
    fn builds_bounded_role_labeled_context() {
        let message = Message {
            id: Uuid::nil(),
            conversation_id: Uuid::nil(),
            client_message_id: None,
            role: MessageRole::User,
            status: MessageStatus::Completed,
            delivery: None,
            text: "Improve Luna's session titles".into(),
            attachments: vec![],
            sent_by_device_id: None,
            ordinal: 1,
            created_at: "2026-03-20T12:00:00Z".into(),
            updated_at: "2026-03-20T12:00:00Z".into(),
        };
        assert_eq!(
            title_context(&[message]),
            Some("User: Improve Luna's session titles\n".into())
        );
    }
}
