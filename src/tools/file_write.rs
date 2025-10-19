//! FileWriteTool - Write files to the filesystem
//!
//! This tool creates new files or overwrites existing files. It includes validation
//! to ensure files are read before being written (to prevent unintentional overwrites).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::tools::{Tool, ToolContext, ToolStreamItem, ValidationResult};
use crate::Result;

const MAX_LINES_TO_RENDER_FOR_ASSISTANT: usize = 16000;
const TRUNCATED_MESSAGE: &str = "<response clipped><NOTE>To save on context only part of this file has been shown to you. You should retry this tool after you have searched inside the file with Grep in order to find the line numbers of what you are looking for.</NOTE>";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteInput {
    /// The absolute path to the file to write (must be absolute, not relative)
    pub file_path: String,
    /// The content to write to the file
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteOutput {
    #[serde(rename = "type")]
    pub operation_type: OperationType,
    pub file_path: String,
    pub content: String,
    pub lines_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Create,
    Update,
}

pub struct FileWriteTool;

impl FileWriteTool {
    fn get_prompt() -> &'static str {
        r#"Write a file to the local filesystem. Overwrites the existing file if there is one.

Before using this tool:

1. Use the ReadFile tool to understand the file's contents and context

2. Directory Verification (only applicable when creating new files):
   - Use the LS tool to verify the parent directory exists and is the correct location"#
    }

    fn add_line_numbers(content: &str, start_line: usize) -> String {
        content
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{:6}\t{}", start_line + i, line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn detect_line_ending(content: &str) -> &'static str {
        if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    fn render_result_for_assistant(output: &FileWriteOutput) -> String {
        match output.operation_type {
            OperationType::Create => {
                format!("File created successfully at: {}", output.file_path)
            }
            OperationType::Update => {
                let content = &output.content;
                let lines: Vec<&str> = content.lines().collect();
                let truncated = if lines.len() > MAX_LINES_TO_RENDER_FOR_ASSISTANT {
                    format!(
                        "{}\n{}",
                        lines[..MAX_LINES_TO_RENDER_FOR_ASSISTANT].join("\n"),
                        TRUNCATED_MESSAGE
                    )
                } else {
                    content.to_string()
                };

                format!(
                    "The file {} has been updated. Here's the result of running `cat -n` on a snippet of the edited file:\n{}",
                    output.file_path,
                    Self::add_line_numbers(&truncated, 1)
                )
            }
        }
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    type Input = FileWriteInput;
    type Output = FileWriteOutput;

    fn name(&self) -> &str {
        "Write"
    }

    async fn description(&self) -> String {
        "Write a file to the local filesystem.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write (must be absolute, not relative)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        Self::get_prompt().to_string()
    }

    fn user_facing_name(&self) -> String {
        "Write".to_string()
    }

    async fn is_enabled(&self) -> bool {
        true
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false // FileWriteTool modifies state/files, not safe for concurrent execution
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        // TODO: Implement permission checking based on file path
        true
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        ctx: &ToolContext,
    ) -> ValidationResult {
        let path = Path::new(&input.file_path);

        // Check if path is absolute
        if !path.is_absolute() {
            return ValidationResult {
                is_valid: false,
                message: Some("file_path must be an absolute path, not relative".to_string()),
            };
        }

        // If file exists, check if it was read before writing
        if path.exists() {
            let full_path = path.to_string_lossy().to_string();

            // Check if file was read
            if let Some(read_timestamp) = ctx.read_file_timestamps.get(&full_path) {
                // Get file's last modified time
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                            let last_write_time = duration.as_millis();

                            if last_write_time > *read_timestamp {
                                return ValidationResult {
                                    is_valid: false,
                                    message: Some(
                                        "File has been modified since read, either by the user or by a linter. Read it again before attempting to write it.".to_string()
                                    ),
                                };
                            }
                        }
                    }
                }
            } else {
                return ValidationResult {
                    is_valid: false,
                    message: Some(
                        "File has not been read yet. Read it first before writing to it.".to_string()
                    ),
                };
            }
        }

        ValidationResult {
            is_valid: true,
            message: None,
        }
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        Ok(format!(
            "Wrote {} lines to {}",
            output.lines_written, output.file_path
        ))
    }

    fn render_tool_use(&self, input: &Self::Input, verbose: bool) -> String {
        if verbose {
            format!("file_path: {}", input.file_path)
        } else {
            // Try to make path relative to cwd
            let path = Path::new(&input.file_path);
            if let Ok(cwd) = std::env::current_dir() {
                if let Ok(rel_path) = path.strip_prefix(&cwd) {
                    return format!("file_path: {}", rel_path.display());
                }
            }
            format!("file_path: {}", input.file_path)
        }
    }

    async fn call(
        &self,
        input: Self::Input,
        mut ctx: ToolContext,
    ) -> Result<crate::tools::ToolStream<Self::Output>> {
        use futures::stream::StreamExt;

        let path = PathBuf::from(&input.file_path);
        let old_file_exists = path.exists();

        // Read old content if file exists
        let old_content = if old_file_exists {
            fs::read_to_string(&path).ok()
        } else {
            None
        };

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Detect line ending from old content or use system default
        let line_ending = if let Some(ref old) = old_content {
            Self::detect_line_ending(old)
        } else {
            if cfg!(windows) { "\r\n" } else { "\n" }
        };

        // Normalize line endings in new content
        let normalized_content = if line_ending == "\r\n" {
            input.content.replace("\n", "\r\n")
        } else {
            input.content.replace("\r\n", "\n")
        };

        // Write the file
        fs::write(&path, &normalized_content)?;

        // Update read timestamp to invalidate stale writes
        if let Ok(metadata) = fs::metadata(&path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                    ctx.read_file_timestamps.insert(
                        input.file_path.clone(),
                        duration.as_millis(),
                    );
                }
            }
        }

        let operation_type = if old_content.is_some() {
            OperationType::Update
        } else {
            OperationType::Create
        };

        let lines_written = normalized_content.lines().count();

        let output = FileWriteOutput {
            operation_type,
            file_path: input.file_path,
            content: normalized_content,
            lines_written,
        };

        let result_for_assistant = Self::render_result_for_assistant(&output);

        let stream = futures::stream::once(async move {
            Ok(ToolStreamItem::Result {
                data: output,
                result_for_assistant: Some(result_for_assistant),
            })
        });

        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_new_file() {
        let tool = FileWriteTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let input = FileWriteInput {
            file_path: file_path.to_string_lossy().to_string(),
            content: "Hello, world!\nLine 2\nLine 3".to_string(),
        };

        let ctx = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            read_file_timestamps: HashMap::new(),
            safe_mode: false,
        };

        let mut stream = tool.call(input, ctx).await.unwrap();
        use futures::stream::StreamExt;

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            assert!(matches!(data.operation_type, OperationType::Create));
            assert_eq!(data.lines_written, 3);
            assert!(file_path.exists());

            let written_content = fs::read_to_string(&file_path).unwrap();
            assert_eq!(written_content, "Hello, world!\nLine 2\nLine 3");
        } else {
            panic!("Expected result");
        }
    }

    #[tokio::test]
    async fn test_update_existing_file() {
        let tool = FileWriteTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create initial file
        fs::write(&file_path, "Old content").unwrap();

        // Get the file timestamp
        let metadata = fs::metadata(&file_path).unwrap();
        let modified = metadata.modified().unwrap();
        let timestamp = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();

        let mut read_timestamps = HashMap::new();
        read_timestamps.insert(file_path.to_string_lossy().to_string(), timestamp);

        let input = FileWriteInput {
            file_path: file_path.to_string_lossy().to_string(),
            content: "New content\nLine 2".to_string(),
        };

        let ctx = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            read_file_timestamps: read_timestamps,
            safe_mode: false,
        };

        let mut stream = tool.call(input, ctx).await.unwrap();
        use futures::stream::StreamExt;

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            assert!(matches!(data.operation_type, OperationType::Update));
            assert_eq!(data.lines_written, 2);

            let written_content = fs::read_to_string(&file_path).unwrap();
            assert_eq!(written_content, "New content\nLine 2");
        } else {
            panic!("Expected result");
        }
    }

    #[tokio::test]
    async fn test_validation_requires_absolute_path() {
        let tool = FileWriteTool;
        let input = FileWriteInput {
            file_path: "relative/path.txt".to_string(),
            content: "test".to_string(),
        };

        let ctx = ToolContext {
            cwd: PathBuf::from("/tmp"),
            read_file_timestamps: HashMap::new(),
            safe_mode: false,
        };

        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
        assert!(result.message.unwrap().contains("absolute path"));
    }

    #[tokio::test]
    async fn test_validation_requires_read_before_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "existing content").unwrap();

        let tool = FileWriteTool;
        let input = FileWriteInput {
            file_path: file_path.to_string_lossy().to_string(),
            content: "new content".to_string(),
        };

        let ctx = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            read_file_timestamps: HashMap::new(), // File not read
            safe_mode: false,
        };

        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
        assert!(result.message.unwrap().contains("has not been read yet"));
    }
}
