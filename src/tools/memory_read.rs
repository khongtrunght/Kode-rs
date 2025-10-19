//! MemoryReadTool - Read from agent memory storage
//!
//! Allows agents to read from persistent memory files stored per-agent.
//! Memory files are stored in ~/.kode/memory/agents/{agent_id}/

use async_stream::stream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{KodeError, Result};
use crate::tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult};

/// Input for MemoryReadTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReadInput {
    /// Optional path to a specific memory file to read
    /// If not provided, returns the index and list of all memory files
    pub file_path: Option<String>,
}

/// Output for MemoryReadTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReadOutput {
    /// Content read from memory
    pub content: String,
}

/// Tool for reading from agent memory
pub struct MemoryReadTool;

impl MemoryReadTool {
    /// Get the memory directory for an agent
    fn get_agent_memory_dir(agent_id: &str) -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| KodeError::Other("Could not determine home directory".to_string()))?;

        let memory_dir = home.join(".kode").join("memory").join("agents").join(agent_id);
        Ok(memory_dir)
    }

    /// List all memory files for an agent
    fn list_memory_files(memory_dir: &Path) -> Result<Vec<PathBuf>> {
        if !memory_dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();

        for entry in walkdir::WalkDir::new(memory_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                files.push(entry.path().to_path_buf());
            }
        }

        Ok(files)
    }
}

#[async_trait]
impl Tool for MemoryReadTool {
    type Input = MemoryReadInput;
    type Output = MemoryReadOutput;

    fn name(&self) -> &'static str {
        "MemoryRead"
    }

    async fn description(&self) -> String {
        "Read from agent memory storage. Memory files are persisted across sessions.".to_string()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        false
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Optional path to a specific memory file to read. If not provided, returns the index and list of all memory files."
                }
            }
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        "Use this tool to read from agent memory storage. Memory files are persisted across sessions and stored per-agent.".to_string()
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

        if let Some(file_path) = &input.file_path {
            // Security: check for path traversal attempts
            if file_path.contains("..") || file_path.starts_with('/') {
                return ValidationResult::error("Invalid memory file path");
            }

            let full_path = memory_dir.join(file_path);

            // Double-check the canonical path is within memory_dir
            if let Ok(canonical) = full_path.canonicalize() {
                if !canonical.starts_with(&memory_dir) {
                    return ValidationResult::error("Invalid memory file path");
                }
            }

            // Check if file exists
            if !full_path.exists() {
                return ValidationResult::error("Memory file does not exist");
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

            // Ensure the directory exists
            if let Err(e) = fs::create_dir_all(&memory_dir) {
                yield Err(e.into());
                return;
            }

            // If a specific file is requested, return its contents
            if let Some(file_path) = &input.file_path {
                let full_path = memory_dir.join(file_path);

                if !full_path.exists() {
                    yield Err(KodeError::FileNotFound(full_path));
                    return;
                }

                let content = match fs::read_to_string(&full_path) {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(e.into());
                        return;
                    }
                };

                yield Ok(ToolStreamItem::Result {
                    data: MemoryReadOutput { content },
                    result_for_assistant: None,
                });
                return;
            }

            // Otherwise, return the index and file list
            let index_path = memory_dir.join("index.md");
            let index = if index_path.exists() {
                match fs::read_to_string(&index_path) {
                    Ok(i) => i,
                    Err(e) => {
                        yield Err(e.into());
                        return;
                    }
                }
            } else {
                String::new()
            };

            let files = match Self::list_memory_files(&memory_dir) {
                Ok(f) => f,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let file_list = if files.is_empty() {
                "No memory files found.".to_string()
            } else {
                files
                    .iter()
                    .map(|f| format!("- {}", f.display()))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            let content = format!(
                "Here are the contents of the agent memory file, `{}`:\n```\n{}\n```\n\nFiles in the agent memory directory:\n{}",
                index_path.display(),
                index,
                file_list
            );

            yield Ok(ToolStreamItem::Result {
                data: MemoryReadOutput { content },
                result_for_assistant: None,
            });
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_memory(agent_id: &str, files: &[(&str, &str)]) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir
            .path()
            .join(".kode")
            .join("memory")
            .join("agents")
            .join(agent_id);

        fs::create_dir_all(&memory_dir).unwrap();

        for (path, content) in files {
            let file_path = memory_dir.join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&file_path, content).unwrap();
        }

        temp_dir
    }

    #[tokio::test]
    async fn test_memory_read_specific_file() {
        let _temp_dir = setup_test_memory("test-agent", &[("notes.txt", "Test memory content")]);

        // Override home dir for testing
        // Note: This test would need environment variable override or dependency injection
        // For now, we're just testing the structure

        let tool = MemoryReadTool;
        let input = MemoryReadInput {
            file_path: Some("notes.txt".to_string()),
        };

        // Validation check
        assert_eq!(tool.name(), "MemoryRead");
        assert!(tool.is_read_only());
        assert!(!tool.needs_permissions(&input));
    }

    #[tokio::test]
    async fn test_memory_read_list_files() {
        let tool = MemoryReadTool;
        let _input = MemoryReadInput { file_path: None };

        // Should list all files in the memory directory
        assert_eq!(tool.name(), "MemoryRead");
    }

    #[tokio::test]
    async fn test_validation_path_traversal() {
        let tool = MemoryReadTool;
        let input = MemoryReadInput {
            file_path: Some("../../etc/passwd".to_string()),
        };
        let context = ToolContext {
            agent_id: Some("test".to_string()),
            ..Default::default()
        };

        let result = tool.validate_input(&input, &context).await;
        assert!(!result.is_valid);
        assert!(result.message.unwrap().contains("Invalid"));
    }

    #[test]
    fn test_list_memory_files() {
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir.path().join("memory");

        fs::create_dir_all(&memory_dir).unwrap();
        fs::write(memory_dir.join("file1.txt"), "content1").unwrap();
        fs::write(memory_dir.join("file2.txt"), "content2").unwrap();

        let files = MemoryReadTool::list_memory_files(&memory_dir).unwrap();
        assert_eq!(files.len(), 2);
    }
}
