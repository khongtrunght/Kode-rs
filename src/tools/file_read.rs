//! FileReadTool - Read files from the local filesystem
//!
//! Supports:
//! - Text files with line range support
//! - Image files (converted to base64)
//! - Automatic file size validation
//! - Similar file suggestions on errors

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

use crate::{
    error::{KodeError, Result},
    tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult},
};

const MAX_LINES_TO_READ: usize = 2000;
const MAX_LINE_LENGTH: usize = 2000;
const MAX_OUTPUT_SIZE: usize = 256 * 1024; // 256KB

/// Input for FileReadTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadInput {
    /// The absolute path to the file to read
    pub file_path: String,

    /// The line number to start reading from (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,

    /// The number of lines to read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Output for FileReadTool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FileReadOutput {
    /// Text file content
    Text {
        file: TextFileContent,
    },
    /// Image file content (base64 encoded)
    Image {
        file: ImageFileContent,
    },
}

/// Text file content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFileContent {
    pub file_path: String,
    pub content: String,
    pub num_lines: usize,
    pub start_line: usize,
    pub total_lines: usize,
}

/// Image file content (base64 encoded)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFileContent {
    pub base64: String,
    pub media_type: String,
}

/// FileReadTool implementation
pub struct FileReadTool;

impl FileReadTool {
    /// Check if a file is an image based on extension
    fn is_image(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            matches!(
                ext.to_str().unwrap_or("").to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
            )
        } else {
            false
        }
    }

    /// Read text content from a file with optional line range
    fn read_text_content(
        path: &Path,
        offset: usize,
        limit: Option<usize>,
    ) -> Result<TextFileContent> {
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start_line = offset;
        let end_line = if let Some(limit) = limit {
            (start_line + limit).min(total_lines)
        } else {
            total_lines.min(start_line + MAX_LINES_TO_READ)
        };

        let selected_lines: Vec<String> = lines[start_line..end_line]
            .iter()
            .map(|line| {
                if line.len() > MAX_LINE_LENGTH {
                    format!("{}... [truncated]", &line[..MAX_LINE_LENGTH])
                } else {
                    line.to_string()
                }
            })
            .collect();

        let num_lines = selected_lines.len();
        let content = selected_lines.join("\n");

        Ok(TextFileContent {
            file_path: path.display().to_string(),
            content,
            num_lines,
            start_line: offset + 1, // Convert to 1-indexed for display
            total_lines,
        })
    }

    /// Read image content as base64
    fn read_image_content(path: &Path) -> Result<ImageFileContent> {
        use base64::{Engine as _, engine::general_purpose};

        let data = fs::read(path)?;
        let base64 = general_purpose::STANDARD.encode(&data);

        let media_type = match path.extension().and_then(|e| e.to_str()) {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("bmp") => "image/bmp",
            Some("webp") => "image/webp",
            _ => "image/png", // Default to PNG
        }
        .to_string();

        Ok(ImageFileContent { base64, media_type })
    }

    /// Add line numbers to content
    fn add_line_numbers(file: &TextFileContent) -> String {
        let lines: Vec<&str> = file.content.lines().collect();
        let start = file.start_line;

        lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>5}\t{}", start + i, line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Normalize file path (resolve to absolute path)
    fn normalize_path(file_path: &str) -> PathBuf {
        let path = PathBuf::from(file_path);
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("/"))
                .join(path)
        }
    }
}

#[async_trait]
impl Tool for FileReadTool {
    type Input = FileReadInput;
    type Output = FileReadOutput;

    fn name(&self) -> &str {
        "View"
    }

    async fn description(&self) -> String {
        "Read a file from the local filesystem.".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": format!("The line number to start reading from. Only provide if the file is too large to read at once")
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once."
                }
            },
            "required": ["file_path"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        format!(
            "Reads a file from the local filesystem. The file_path parameter must be an absolute path, not a relative path. \
            By default, it reads up to {} lines starting from the beginning of the file. \
            You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters. \
            Any lines longer than {} characters will be truncated. \
            Results are returned using cat -n format, with line numbers starting at 1.",
            MAX_LINES_TO_READ,
            MAX_LINE_LENGTH
        )
    }

    fn user_facing_name(&self) -> String {
        "Read".to_string()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        false // Read operations don't need special permissions in safe mode
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        _context: &ToolContext,
    ) -> ValidationResult {
        let path = Self::normalize_path(&input.file_path);

        if !path.exists() {
            return ValidationResult::error(format!("File does not exist: {}", path.display()));
        }

        if !path.is_file() {
            return ValidationResult::error(format!("Path is not a file: {}", path.display()));
        }

        // Check file size for text files
        if !Self::is_image(&path) {
            if let Ok(metadata) = fs::metadata(&path) {
                let file_size = metadata.len() as usize;

                // If file is too large and no offset/limit provided
                if file_size > MAX_OUTPUT_SIZE
                    && input.offset.is_none()
                    && input.limit.is_none()
                {
                    return ValidationResult::error(format!(
                        "File content ({}KB) exceeds maximum allowed size ({}KB). \
                        Please use offset and limit parameters to read specific portions of the file.",
                        file_size / 1024,
                        MAX_OUTPUT_SIZE / 1024
                    ));
                }
            }
        }

        ValidationResult::ok()
    }

    fn render_tool_use(&self, input: &Self::Input, verbose: bool) -> String {
        let path = if verbose {
            input.file_path.clone()
        } else {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| {
                    PathBuf::from(&input.file_path)
                        .strip_prefix(&cwd)
                        .ok()
                        .map(|p| p.display().to_string())
                })
                .unwrap_or_else(|| input.file_path.clone())
        };

        let mut parts = vec![format!("file_path: \"{}\"", path)];

        if let Some(offset) = input.offset {
            parts.push(format!("offset: {}", offset));
        }

        if let Some(limit) = input.limit {
            parts.push(format!("limit: {}", limit));
        }

        parts.join(", ")
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        match output {
            FileReadOutput::Text { file } => {
                Ok(format!(
                    "Read {} lines ({}-{} of {}) from {}:\n{}",
                    file.num_lines,
                    file.start_line,
                    file.start_line + file.num_lines - 1,
                    file.total_lines,
                    file.file_path,
                    Self::add_line_numbers(file)
                ))
            }
            FileReadOutput::Image { .. } => Ok("Read image file (base64 encoded)".to_string()),
        }
    }

    async fn call(
        &self,
        input: Self::Input,
        _context: ToolContext,
    ) -> Result<ToolStream<Self::Output>> {
        let path = Self::normalize_path(&input.file_path);
        let offset = input.offset.unwrap_or(1);
        let limit = input.limit;

        // Convert 1-indexed offset to 0-indexed
        let line_offset = if offset == 0 { 0 } else { offset - 1 };

        let output = if Self::is_image(&path) {
            let image = Self::read_image_content(&path)?;
            FileReadOutput::Image { file: image }
        } else {
            let text = Self::read_text_content(&path, line_offset, limit)?;

            // Validate output size
            if text.content.len() > MAX_OUTPUT_SIZE {
                return Err(KodeError::ToolValidation(format!(
                    "File content ({}KB) exceeds maximum allowed size ({}KB). \
                    Please use offset and limit parameters to read specific portions of the file.",
                    text.content.len() / 1024,
                    MAX_OUTPUT_SIZE / 1024
                )));
            }

            FileReadOutput::Text { file: text }
        };

        // Create the stream
        let stream = async_stream::stream! {
            yield Ok(ToolStreamItem::Result {
                data: output,
                result_for_assistant: None,
            });
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_small_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        let tool = FileReadTool;
        let input = FileReadInput {
            file_path: temp_file.path().display().to_string(),
            offset: None,
            limit: None,
        };

        let ctx = ToolContext::default();
        let validation = tool.validate_input(&input, &ctx).await;
        assert!(validation.result);

        let mut stream = tool.call(input, ctx).await.unwrap();
        use futures::StreamExt;

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            match data {
                FileReadOutput::Text { file } => {
                    assert_eq!(file.num_lines, 3);
                    assert_eq!(file.total_lines, 3);
                    assert!(file.content.contains("Line 1"));
                    assert!(file.content.contains("Line 3"));
                }
                _ => panic!("Expected Text output"),
            }
        } else {
            panic!("Expected result from stream");
        }
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let mut temp_file = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(temp_file, "Line {}", i).unwrap();
        }
        temp_file.flush().unwrap();

        let tool = FileReadTool;
        let input = FileReadInput {
            file_path: temp_file.path().display().to_string(),
            offset: Some(5),
            limit: Some(3),
        };

        let ctx = ToolContext::default();
        let mut stream = tool.call(input, ctx).await.unwrap();
        use futures::StreamExt;

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            match data {
                FileReadOutput::Text { file } => {
                    assert_eq!(file.num_lines, 3);
                    assert_eq!(file.start_line, 5);
                    assert!(file.content.contains("Line 5"));
                    assert!(file.content.contains("Line 7"));
                }
                _ => panic!("Expected Text output"),
            }
        }
    }

    #[tokio::test]
    async fn test_validation_file_not_found() {
        let tool = FileReadTool;
        let input = FileReadInput {
            file_path: "/nonexistent/file.txt".to_string(),
            offset: None,
            limit: None,
        };

        let ctx = ToolContext::default();
        let validation = tool.validate_input(&input, &ctx).await;
        assert!(!validation.result);
        assert!(validation
            .message
            .unwrap()
            .contains("File does not exist"));
    }
}
