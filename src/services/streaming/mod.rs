//! Streaming support for AI model responses
//!
//! Provides infrastructure for handling Server-Sent Events (SSE) streams
//! from various AI providers (Anthropic, OpenAI, etc.).

pub mod anthropic_stream;
pub mod openai_stream;
pub mod sse_parser;

pub use anthropic_stream::AnthropicStreamHandler;
pub use openai_stream::OpenAIStreamHandler;
pub use sse_parser::{SseEvent, SseParser};

use crate::messages::{AssistantMessage, ContentBlock, Message};
use crate::services::Usage;
use serde::{Deserialize, Serialize};

/// Stream event types for Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    /// Initial message metadata
    MessageStart {
        message: MessageMetadata,
    },

    /// Start of a content block
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStart,
    },

    /// Delta update for content block
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },

    /// End of a content block
    ContentBlockStop {
        index: usize,
    },

    /// Message-level delta (usage, stop reason, etc.)
    MessageDelta {
        delta: MessageDeltaData,
        usage: Option<UsageDelta>,
    },

    /// End of message stream
    MessageStop,

    /// Ping event (keepalive)
    Ping,

    /// Error event
    Error {
        error: ErrorData,
    },
}

/// Message metadata from message_start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub id: String,
    pub model: String,
    pub role: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub usage: Usage,
}

/// Content block start data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockStart {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
    Thinking {
        thinking: String,
    },
}

/// Content delta types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    TextDelta {
        text: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    ThinkingDelta {
        thinking: String,
    },
}

/// Message delta data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

/// Usage delta for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageDelta {
    pub output_tokens: Option<u32>,
}

/// Error data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// OpenAI stream event (chunk)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIStreamChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<Usage>,
}

/// OpenAI choice in stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChoice {
    pub index: usize,
    pub delta: OpenAIDelta,
    pub finish_reason: Option<String>,
}

/// OpenAI delta content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    pub reasoning: Option<String>,
}

/// Tool call delta for OpenAI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub function: Option<FunctionDelta>,
}

/// Function call delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}
