//! Message types for AI conversations
//!
//! This module defines the message types used in conversations with AI models.
//! It includes user messages, assistant messages, progress messages, and tool use/result structures.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Message role in the conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Content block in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
    },
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<Uuid>,
}

impl Message {
    /// Create a new user message
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
            uuid: Some(Uuid::new_v4()),
        }
    }

    /// Create a new assistant message
    #[must_use]
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
            uuid: Some(Uuid::new_v4()),
        }
    }

    /// Create a new system message
    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text { text: text.into() }],
            uuid: None,
        }
    }

    /// Get text content from the message (concatenates all text blocks)
    #[must_use]
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Check if message contains tool use
    #[must_use]
    pub fn has_tool_use(&self) -> bool {
        self.content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }))
    }

    /// Get all tool use blocks
    #[must_use]
    pub fn tool_uses(&self) -> Vec<&ContentBlock> {
        self.content
            .iter()
            .filter(|block| matches!(block, ContentBlock::ToolUse { .. }))
            .collect()
    }
}

/// User message with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub message: Message,
    pub uuid: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<UserMessageOptions>,
    /// Tool use result if this message contains tool results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<FullToolUseResult>,
}

/// Tool use result containing execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullToolUseResult {
    pub tool_use_id: String,
    pub tool_name: String,
    pub result: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Options for user messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_koding_request: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub koding_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_custom_command: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_args: Option<String>,
}

/// Assistant message with cost and duration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub message: Message,
    pub uuid: Uuid,
    pub cost_usd: f64,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_api_error_message: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
}

/// Progress message during tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage {
    pub content: AssistantMessage,
    pub tool_use_id: String,
    pub uuid: Uuid,
    /// Normalized messages for context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_messages: Option<Vec<Message>>,
    /// Sibling tool use IDs executing concurrently
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sibling_tool_use_ids: Option<Vec<String>>,
}

/// Combined message type for the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConversationMessage {
    User(UserMessage),
    Assistant(AssistantMessage),
    Progress(ProgressMessage),
}

impl ConversationMessage {
    /// Get the UUID of the message
    #[must_use]
    pub const fn uuid(&self) -> &Uuid {
        match self {
            Self::User(msg) => &msg.uuid,
            Self::Assistant(msg) => &msg.uuid,
            Self::Progress(msg) => &msg.uuid,
        }
    }

    /// Check if this is a user message
    #[must_use]
    pub const fn is_user(&self) -> bool {
        matches!(self, Self::User(_))
    }

    /// Check if this is an assistant message
    #[must_use]
    pub const fn is_assistant(&self) -> bool {
        matches!(self, Self::Assistant(_))
    }

    /// Check if this is a progress message
    #[must_use]
    pub const fn is_progress(&self) -> bool {
        matches!(self, Self::Progress(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_message() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.text_content(), "Hello");
        assert!(msg.uuid.is_some());
    }

    #[test]
    fn test_create_assistant_message() {
        let msg = Message::assistant("Hi there");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.text_content(), "Hi there");
    }

    #[test]
    fn test_tool_use_detection() {
        let msg = Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me help".into(),
                },
                ContentBlock::ToolUse {
                    id: "tool_1".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
            uuid: None,
        };
        assert!(msg.has_tool_use());
        assert_eq!(msg.tool_uses().len(), 1);
    }
}
