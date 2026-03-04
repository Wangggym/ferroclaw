use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Role of a conversation participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A tool call requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call (used to match results).
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// The result of a tool execution, returned to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub output: String,
    pub is_error: bool,
}

/// Flexible message content — plain text or structured blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    ToolCalls(Vec<ToolCall>),
    ToolResults(Vec<ToolResult>),
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn assistant_tool_calls(calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::ToolCalls(calls),
        }
    }

    pub fn tool_results(results: Vec<ToolResult>) -> Self {
        Self {
            role: Role::Tool,
            content: MessageContent::ToolResults(results),
        }
    }
}

/// An ordered list of messages forming a conversation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationHistory {
    pub messages: Vec<Message>,
}

impl ConversationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Append additional text to the system prompt (first message).
    /// No-op if the history is empty or the first message is not a system text.
    pub fn append_to_system_prompt(&mut self, extra: &str) {
        if let Some(sys) = self.messages.first_mut() {
            if matches!(sys.role, Role::System) {
                if let MessageContent::Text(ref mut t) = sys.content {
                    t.push('\n');
                    t.push_str(extra);
                }
            }
        }
    }

    /// Returns a slice of all messages (read-only).
    pub fn as_slice(&self) -> &[Message] {
        &self.messages
    }
}
