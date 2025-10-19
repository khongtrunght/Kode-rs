//! Tool system for Kode-rs
//!
//! Provides the core [`Tool`] trait and tool implementations for interacting with
//! the codebase, file system, and external services.

pub mod file_read;

use std::{collections::HashMap, pin::Pin};

use async_trait::async_trait;
use futures::Stream;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{error::Result, messages::Message};

/// Tool execution context
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Unique message ID for this tool use
    pub message_id: Option<String>,

    /// Agent ID if running within an agent
    pub agent_id: Option<String>,

    /// Safe mode enabled (requires more permissions)
    pub safe_mode: bool,

    /// File read timestamps for tracking changes
    pub read_file_timestamps: HashMap<String, u64>,

    /// Verbose output enabled
    pub verbose: bool,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            message_id: None,
            agent_id: None,
            safe_mode: false,
            read_file_timestamps: HashMap::new(),
            verbose: false,
        }
    }
}

/// Tool validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub result: bool,

    /// Optional error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Optional error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl ValidationResult {
    /// Create a successful validation result
    #[must_use]
    pub const fn ok() -> Self {
        Self {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    /// Create a failed validation result with a message
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            result: false,
            message: Some(message.into()),
            error_code: None,
            meta: None,
        }
    }

    /// Create a failed validation result with a message and code
    #[must_use]
    pub fn error_with_code(message: impl Into<String>, code: i32) -> Self {
        Self {
            result: false,
            message: Some(message.into()),
            error_code: Some(code),
            meta: None,
        }
    }
}

/// Tool execution stream item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolStreamItem<T> {
    /// Progress update during execution
    Progress {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        normalized_messages: Option<Vec<Message>>,
    },

    /// Final result
    Result {
        data: T,
        #[serde(skip_serializing_if = "Option::is_none")]
        result_for_assistant: Option<String>,
    },
}

/// Tool execution stream type
pub type ToolStream<T> = Pin<Box<dyn Stream<Item = Result<ToolStreamItem<T>>> + Send>>;

/// Core tool trait
///
/// All tools must implement this trait to be usable in the system.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Input type for this tool (must be serializable and deserializable)
    type Input: Serialize + DeserializeOwned + Send + Sync;

    /// Output type for this tool (must be serializable)
    type Output: Serialize + Send + Sync;

    /// Get the tool name (e.g., "FileRead", "Bash")
    fn name(&self) -> &str;

    /// Get the tool description (async for dynamic descriptions)
    async fn description(&self) -> String;

    /// Get the JSON schema for tool input
    fn input_schema(&self) -> Value;

    /// Get the system prompt for this tool
    async fn prompt(&self, safe_mode: bool) -> String;

    /// Get user-facing name for this tool
    fn user_facing_name(&self) -> String {
        self.name().to_string()
    }

    /// Check if this tool is enabled
    async fn is_enabled(&self) -> bool {
        true
    }

    /// Check if this tool only reads data (no side effects)
    fn is_read_only(&self) -> bool {
        false
    }

    /// Check if this tool is safe to run concurrently
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Check if this tool needs user permission for the given input
    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        !self.is_read_only()
    }

    /// Validate tool input before execution
    async fn validate_input(
        &self,
        _input: &Self::Input,
        _context: &ToolContext,
    ) -> ValidationResult {
        ValidationResult::ok()
    }

    /// Render the tool result for the assistant
    fn render_result(&self, output: &Self::Output) -> Result<String> {
        Ok(serde_json::to_string_pretty(output)?)
    }

    /// Render a message showing tool use to the user
    fn render_tool_use(&self, input: &Self::Input, verbose: bool) -> String {
        if verbose {
            format!(
                "Using {} with input:\n{}",
                self.name(),
                serde_json::to_string_pretty(input).unwrap_or_else(|_| "<??>".to_string())
            )
        } else {
            format!("Using {}", self.name())
        }
    }

    /// Execute the tool with given input
    ///
    /// Returns a stream of progress updates and the final result.
    async fn call(
        &self,
        input: Self::Input,
        context: ToolContext,
    ) -> Result<ToolStream<Self::Output>>;
}

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool<Input = Value, Output = Value>>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Get a tool by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Tool<Input = Value, Output = Value>> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// List all registered tool names
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let ok = ValidationResult::ok();
        assert!(ok.result);
        assert!(ok.message.is_none());

        let error = ValidationResult::error("something went wrong");
        assert!(!error.result);
        assert_eq!(error.message, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_tool_context_default() {
        let ctx = ToolContext::default();
        assert!(!ctx.safe_mode);
        assert!(!ctx.verbose);
        assert!(ctx.message_id.is_none());
    }
}
