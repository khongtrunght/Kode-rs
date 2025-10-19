//! Core Tool trait for Kode's extensible tool system
//!
//! Provides standardized contract for all tool implementations.
//! Ported from TypeScript's Tool.ts interface.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Result;

/// Context for tool execution
#[derive(Debug, Clone)]
pub struct ToolUseContext {
    /// Unique message ID
    pub message_id: Option<String>,
    /// Agent ID if running in agent context
    pub agent_id: Option<String>,
    /// Safe mode flag (restricted permissions)
    pub safe_mode: bool,
    /// Abort signal for cancellation
    pub abort_signal: Arc<tokio::sync::Notify>,
    /// File read timestamps for change detection
    pub read_file_timestamps: Arc<RwLock<HashMap<String, u64>>>,
    /// Additional options
    pub options: ToolOptions,
    /// Response state for stateful APIs (e.g., GPT-5)
    pub response_state: Option<ResponseState>,
}

/// Tool execution options
#[derive(Debug, Clone, Default)]
pub struct ToolOptions {
    /// Available commands
    pub commands: Vec<String>,
    /// Available tools
    pub tools: Vec<String>,
    /// Verbose output flag
    pub verbose: bool,
    /// Slow but capable model name
    pub slow_and_capable_model: Option<String>,
    /// Safe mode flag
    pub safe_mode: bool,
    /// Fork number for parallel execution
    pub fork_number: usize,
    /// Message log name for debugging
    pub message_log_name: Option<String>,
    /// Maximum thinking tokens
    pub max_thinking_tokens: Option<usize>,
    /// Koding mode request
    pub is_koding_request: bool,
    /// Koding context
    pub koding_context: Option<String>,
    /// Custom command flag
    pub is_custom_command: bool,
}

/// Response state for stateful APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseState {
    pub previous_response_id: Option<String>,
    pub conversation_id: Option<String>,
}

/// Validation result from tool input validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub result: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            result: false,
            message: Some(message.into()),
            error_code: None,
            meta: None,
        }
    }
}

/// Tool execution result types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolStreamEvent {
    /// Final result with data for assistant
    Result {
        data: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result_for_assistant: Option<String>,
    },
    /// Progress update during execution
    Progress {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        normalized_messages: Option<Vec<Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tools: Option<Vec<String>>,
    },
}

/// Stream of tool execution events
pub type ToolStream = futures::stream::BoxStream<'static, Result<ToolStreamEvent>>;

/// Core Tool trait for all tools in Kode
///
/// This trait defines the interface that all tools must implement.
/// Tools are the primary way the AI assistant interacts with the system.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name (e.g., "FileRead", "Bash")
    fn name(&self) -> &str;

    /// Get the tool description (async to support dynamic descriptions)
    ///
    /// IMPORTANT: This is async because some tools may need to read files
    /// or check system state to generate their description.
    async fn description(&self) -> String;

    /// Get the JSON schema for tool input
    ///
    /// This schema is used for validation and is sent to the AI model.
    fn input_schema(&self) -> Value;

    /// Get the system prompt for this tool
    ///
    /// The prompt explains how to use the tool and what it does.
    /// It may differ based on safe_mode.
    async fn prompt(&self, safe_mode: bool) -> String;

    /// Get user-facing name (for display in UI)
    fn user_facing_name(&self) -> String {
        self.name().to_string()
    }

    /// Check if the tool is currently enabled
    async fn is_enabled(&self) -> bool {
        true
    }

    /// Check if the tool only reads data (doesn't modify state)
    fn is_read_only(&self) -> bool {
        false
    }

    /// Check if the tool is safe to run concurrently
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Check if the tool needs permission for the given input
    fn needs_permissions(&self, input: &Value) -> bool;

    /// Validate tool input before execution
    ///
    /// Returns a ValidationResult indicating if the input is valid.
    async fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        let _ = (input, context);
        ValidationResult::valid()
    }

    /// Render the result for the assistant to see
    ///
    /// Converts the tool output into a format the AI can understand.
    fn render_result_for_assistant(&self, output: &Value) -> String;

    /// Render the "tool use" message shown to the user
    ///
    /// This is displayed when the tool is invoked.
    fn render_tool_use_message(&self, input: &Value, verbose: bool) -> String;

    /// Execute the tool with the given input
    ///
    /// Returns a stream of events (progress updates and final result).
    /// This allows tools to provide real-time feedback during execution.
    async fn call(&self, input: Value, context: ToolUseContext) -> ToolStream;
}

/// Trait object type for dynamic dispatch
pub type DynTool = Arc<dyn Tool>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let valid = ValidationResult::valid();
        assert!(valid.result);
        assert!(valid.message.is_none());

        let invalid = ValidationResult::invalid("Bad input");
        assert!(!invalid.result);
        assert_eq!(invalid.message.as_deref(), Some("Bad input"));
    }

    #[test]
    fn test_tool_use_context() {
        let ctx = ToolUseContext {
            message_id: Some("msg_123".to_string()),
            agent_id: None,
            safe_mode: false,
            abort_signal: Arc::new(tokio::sync::Notify::new()),
            read_file_timestamps: Arc::new(RwLock::new(HashMap::new())),
            options: ToolOptions::default(),
            response_state: None,
        };

        assert_eq!(ctx.message_id.as_deref(), Some("msg_123"));
        assert!(!ctx.safe_mode);
    }
}
