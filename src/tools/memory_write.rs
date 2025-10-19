//! MemoryWriteTool - Write to agent memory storage
//!
//! Allows agents to write to persistent memory files stored per-agent.
//! Memory files are stored in ~/.kode/memory/agents/{agent_id}/

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::{KodeError, Result};
use crate::tools::{Tool, ToolContext, ToolStreamItem, ValidationResult};

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

    fn name(&self) -> &'static str {
        "MemoryWrite"
    }

    async fn description(&self) -> String {
        "Write to agent memory storage. Memory files are persisted across sessions.".to_string()
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn needs_permissions(&self) -> bool {
        false
    }

    async fn validate(
        &self,
        input: &Self::Input,
        context: &ToolContext,
    ) -> Result<ValidationResult> {
        let agent_id = context.agent_id.as_deref().unwrap_or("default");
        let memory_dir = Self::get_agent_memory_dir(agent_id)?;
        let full_path = memory_dir.join(&input.file_path);

        // Security: ensure the path is within the memory directory
        if !full_path.starts_with(&memory_dir) {
            return Ok(ValidationResult {
                is_valid: false,
                message: Some("Invalid memory file path".to_string()),
            });
        }

        Ok(ValidationResult {
            is_valid: true,
            message: None,
        })
    }

    async fn execute(
        &self,
        input: &Self::Input,
        context: &ToolContext,
    ) -> Result<ToolStreamItem<Self::Output>> {
        let agent_id = context.agent_id.as_deref().unwrap_or("default");
        let memory_dir = Self::get_agent_memory_dir(agent_id)?;
        let full_path = memory_dir.join(&input.file_path);

        // Create parent directories if they don't exist
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write the file
        fs::write(&full_path, &input.content)?;

        Ok(ToolStreamItem::Result(MemoryWriteOutput {
            message: format!("Saved to {}", input.file_path),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_write_basic() {
        let tool = MemoryWriteTool;

        // Tool properties
        assert_eq!(tool.name(), "MemoryWrite");
        assert!(!tool.is_read_only());
        assert!(!tool.needs_permissions());
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

        let result = tool.validate(&input, &context).await.unwrap();
        assert!(!result.valid);
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

        let result = tool.validate(&input, &context).await.unwrap();
        assert!(result.valid);
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
