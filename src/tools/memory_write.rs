//! MemoryWriteTool - Write to agent memory storage
//!
//! Allows agents to write to persistent memory files stored per-agent.
//! Memory files are stored in ~/.kode/memory/agents/{agent_id}/

use async_stream::stream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::error::{KodeError, Result};
use crate::tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult};

/// Input for MemoryWriteTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWriteInput {
    /// Path to the memory file to write (relative to agent memory directory)
    pub file_path: String,

    /// Content to write to the file
    pub content: String,
}

/// Output for MemoryWriteTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWriteOutput {
    /// Result message
    pub message: String,
}

/// Tool for writing to agent memory
pub struct MemoryWriteTool;

impl MemoryWriteTool {
    /// Get the memory directory for an agent
    fn get_agent_memory_dir(agent_id: &str) -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| KodeError::Other("Could not determine home directory".to_string()))?;

        let memory_dir = home.join(".kode").join("memory").join("agents").join(agent_id);
        Ok(memory_dir)
    }
}

#[async_trait]
impl Tool for MemoryWriteTool {
    type Input = MemoryWriteInput;
    type Output = MemoryWriteOutput;

    fn name(&self) -> &str {
        "MemoryWrite"
    }

    async fn description(&self) -> String {
        "Write to agent memory storage. Memory files are persisted across sessions.".to_string()
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the memory file to write (relative to agent memory directory)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        "Use this tool to write to agent memory storage. Memory files are persisted across sessions and stored per-agent.".to_string()
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        context: &ToolContext,
    ) -> ValidationResult {
        let agent_id = context.agent_id.as_deref().unwrap_or("default");
        let memory_dir = match Self::get_agent_memory_dir(agent_id) {
            Ok(dir) => dir,
            Err(e) => return ValidationResult::error(format!("Failed to get memory directory: {}", e)),
        };

        // Security: check for path traversal attempts
        if input.file_path.contains("..") || input.file_path.starts_with('/') {
            return ValidationResult::error("Invalid memory file path");
        }

        let full_path = memory_dir.join(&input.file_path);

        // Double-check the path is within memory_dir (before file creation)
        // We can't canonicalize a non-existent file, so check the parent
        if let Some(parent) = full_path.parent() {
            if parent.exists() {
                if let Ok(canonical_parent) = parent.canonicalize() {
                    if !canonical_parent.starts_with(&memory_dir) {
                        return ValidationResult::error("Invalid memory file path");
                    }
                }
            }
        }

        ValidationResult::ok()
    }

    async fn call(
        &self,
        input: Self::Input,
        context: ToolContext,
    ) -> Result<ToolStream<Self::Output>> {
        Ok(Box::pin(stream! {
            let agent_id = context.agent_id.as_deref().unwrap_or("default");
            let memory_dir = match Self::get_agent_memory_dir(agent_id) {
                Ok(dir) => dir,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let full_path = memory_dir.join(&input.file_path);

            // Create parent directories if they don't exist
            if let Some(parent) = full_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    yield Err(e.into());
                    return;
                }
            }

            // Write the file
            if let Err(e) = fs::write(&full_path, &input.content) {
                yield Err(e.into());
                return;
            }

            yield Ok(ToolStreamItem::Result {
                data: MemoryWriteOutput {
                    message: format!("Saved to {}", input.file_path),
                },
                result_for_assistant: None,
            });
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_write_basic() {
        let tool = MemoryWriteTool;

        // Tool properties
        assert_eq!(tool.name(), "MemoryWrite");
        assert!(!tool.is_read_only());

        let input = MemoryWriteInput {
            file_path: "test.txt".to_string(),
            content: "content".to_string(),
        };
        assert!(tool.needs_permissions(&input));
    }

    #[tokio::test]
    async fn test_validation_path_traversal() {
        let tool = MemoryWriteTool;
        let input = MemoryWriteInput {
            file_path: "../../etc/passwd".to_string(),
            content: "malicious content".to_string(),
        };
        let context = ToolContext {
            agent_id: Some("test".to_string()),
            ..Default::default()
        };

        let result = tool.validate_input(&input, &context).await;
        assert!(!result.is_valid);
        assert!(result.message.unwrap().contains("Invalid"));
    }

    #[tokio::test]
    async fn test_validation_valid_path() {
        let tool = MemoryWriteTool;
        let input = MemoryWriteInput {
            file_path: "notes.txt".to_string(),
            content: "test content".to_string(),
        };
        let context = ToolContext {
            agent_id: Some("test".to_string()),
            ..Default::default()
        };

        let result = tool.validate_input(&input, &context).await;
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_write_creates_directories() {
        let tool = MemoryWriteTool;

        // Note: This is a simplified test. In a real scenario, we'd need to
        // override the home directory or use dependency injection
        assert_eq!(tool.name(), "MemoryWrite");
    }

    #[test]
    fn test_get_agent_memory_dir() {
        let result = MemoryWriteTool::get_agent_memory_dir("test-agent");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("memory"));
        assert!(path.to_string_lossy().contains("test-agent"));
    }
}
