//! FileEditTool - Edit files with exact string replacement
//!
//! This tool provides precise file editing by replacing an exact old_string with new_string.
//! It includes strong validation to ensure edits are unambiguous and safe.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::tools::{Tool, ToolContext, ToolStreamItem, ValidationResult};
use crate::Result;

const N_LINES_SNIPPET: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditInput {
    /// The absolute path to the file to modify (must be absolute, not relative)
    pub file_path: String,
    /// The text to replace (must be unique within the file)
    pub old_string: String,
    /// The text to replace it with (must be different from old_string)
    pub new_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditOutput {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    pub original_file: String,
    pub snippet: String,
    pub start_line: usize,
}

pub struct FileEditTool;

impl FileEditTool {
    fn get_prompt() -> &'static str {
        r#"This is a tool for editing files. For moving or renaming files, you should generally use the Bash tool with the 'mv' command instead. For larger edits, use the Write tool to overwrite files.

Before using this tool:

1. Use the View tool to understand the file's contents and context

2. Verify the directory path is correct (only applicable when creating new files):
   - Use the LS tool to verify the parent directory exists and is the correct location

To make a file edit, provide the following:
1. file_path: The absolute path to the file to modify (must be absolute, not relative)
2. old_string: The text to replace (must be unique within the file, and must match the file contents exactly, including all whitespace and indentation)
3. new_string: The edited text to replace the old_string

The tool will replace ONE occurrence of old_string with new_string in the specified file.

CRITICAL REQUIREMENTS FOR USING THIS TOOL:

1. UNIQUENESS: The old_string MUST uniquely identify the specific instance you want to change. This means:
   - Include AT LEAST 3-5 lines of context BEFORE the change point
   - Include AT LEAST 3-5 lines of context AFTER the change point
   - Include all whitespace, indentation, and surrounding code exactly as it appears in the file

2. SINGLE INSTANCE: This tool can only change ONE instance at a time. If you need to change multiple instances:
   - Make separate calls to this tool for each instance
   - Each call must uniquely identify its specific instance using extensive context

3. VERIFICATION: Before using this tool:
   - Check how many instances of the target text exist in the file
   - If multiple instances exist, gather enough context to uniquely identify each one
   - Plan separate tool calls for each instance

WARNING: If you do not follow these requirements:
   - The tool will fail if old_string matches multiple locations
   - The tool will fail if old_string doesn't match exactly (including whitespace)
   - You may change the wrong instance if you don't include enough context

When making edits:
   - Ensure the edit results in idiomatic, correct code
   - Do not leave the code in a broken state
   - Always use absolute file paths (starting with /)

If you want to create a new file, use:
   - A new file path, including dir name if needed
   - An empty old_string
   - The new file's contents as new_string

Remember: when making multiple file edits in a row to the same file, you should prefer to send all edits in a single message with multiple calls to this tool, rather than multiple messages with a single call each."#
    }

    /// Apply an edit to file content
    fn apply_edit(
        original: &str,
        old_string: &str,
        new_string: &str,
    ) -> std::result::Result<String, String> {
        if old_string.is_empty() {
            // Create new file
            return Ok(new_string.to_string());
        }

        // Check if old_string exists in the file
        if !original.contains(old_string) {
            return Err("String to replace not found in file.".to_string());
        }

        // Special handling for deletion (new_string is empty)
        let updated = if new_string.is_empty() {
            // If old_string doesn't end with newline but it exists with newline in file, include it
            if !old_string.ends_with('\n') && original.contains(&format!("{}\n", old_string)) {
                original.replacen(&format!("{}\n", old_string), new_string, 1)
            } else {
                original.replacen(old_string, new_string, 1)
            }
        } else {
            original.replacen(old_string, new_string, 1)
        };

        // Verify the replacement actually changed the file
        if updated == original {
            return Err("Original and edited file match exactly. Failed to apply edit.".to_string());
        }

        Ok(updated)
    }

    /// Get a snippet of the edited file around the change
    fn get_snippet(original: &str, old_string: &str, new_string: &str) -> (String, usize) {
        if old_string.is_empty() {
            // For new files, show first N lines
            let lines: Vec<&str> = new_string.lines().collect();
            let snippet_lines: Vec<&str> = lines.iter().take(N_LINES_SNIPPET * 2).copied().collect();
            return (snippet_lines.join("\n"), 1);
        }

        let original_lines: Vec<&str> = original.lines().collect();

        // Find the line where old_string starts
        let mut start_line = 0;
        let old_lines: Vec<&str> = old_string.lines().collect();

        for (i, window) in original_lines.windows(old_lines.len()).enumerate() {
            let window_str = window.join("\n");
            if window_str.contains(old_string) {
                start_line = i;
                break;
            }
        }

        // Calculate snippet range (context before and after)
        let context_start = start_line.saturating_sub(N_LINES_SNIPPET);
        let context_end = (start_line + old_lines.len() + N_LINES_SNIPPET).min(original_lines.len());

        // Get the updated file content
        if let Ok(updated) = Self::apply_edit(original, old_string, new_string) {
            let updated_lines: Vec<&str> = updated.lines().collect();
            let snippet_lines: Vec<&str> = updated_lines
                .iter()
                .skip(context_start)
                .take(context_end - context_start)
                .copied()
                .collect();

            (snippet_lines.join("\n"), context_start + 1)
        } else {
            // Fallback to showing original
            let snippet_lines: Vec<&str> = original_lines
                .iter()
                .skip(context_start)
                .take(context_end - context_start)
                .copied()
                .collect();

            (snippet_lines.join("\n"), context_start + 1)
        }
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
}

#[async_trait]
impl Tool for FileEditTool {
    type Input = FileEditInput;
    type Output = FileEditOutput;

    fn name(&self) -> &str {
        "Edit"
    }

    async fn description(&self) -> String {
        "A tool for editing files".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify (must be absolute, not relative)"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace (must be unique within the file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        Self::get_prompt().to_string()
    }

    fn user_facing_name(&self) -> String {
        "Edit".to_string()
    }

    async fn is_enabled(&self) -> bool {
        true
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false // FileEdit modifies files, not safe for concurrent execution
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
        // Check if old_string and new_string are the same
        if input.old_string == input.new_string {
            return ValidationResult::error(
                "No changes to make: old_string and new_string are exactly the same."
            );
        }

        let path = Path::new(&input.file_path);

        // Check if path is absolute
        if !path.is_absolute() {
            return ValidationResult::error(
                "file_path must be an absolute path, not relative"
            );
        }

        let full_path = path.to_string_lossy().to_string();

        // Special case: creating a new file (old_string is empty)
        if input.old_string.is_empty() {
            if path.exists() {
                return ValidationResult::error(
                    "Cannot create new file - file already exists."
                );
            }
            return ValidationResult::ok();
        }

        // File must exist for editing
        if !path.exists() {
            return ValidationResult::error(format!(
                "File does not exist: {}",
                path.display()
            ));
        }

        // Check if file is a Jupyter notebook
        if let Some(ext) = path.extension() {
            if ext == "ipynb" {
                return ValidationResult::error(
                    "File is a Jupyter Notebook. Use the NotebookEdit tool to edit this file."
                );
            }
        }

        // Check if file was read before editing
        if let Some(read_timestamp) = ctx.read_file_timestamps.get(&full_path) {
            // Get file's last modified time
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                        let last_write_time = duration.as_millis();

                        if last_write_time > *read_timestamp {
                            return ValidationResult::error(
                                "File has been modified since read, either by the user or by a linter. Read it again before attempting to write it."
                            );
                        }
                    }
                }
            }
        } else {
            return ValidationResult::error(
                "File has not been read yet. Read it first before writing to it."
            );
        }

        // Read file content and validate old_string
        match fs::read_to_string(&path) {
            Ok(content) => {
                if !content.contains(&input.old_string) {
                    return ValidationResult::error(
                        "String to replace not found in file."
                    );
                }

                // Check for multiple matches
                let matches = content.matches(&input.old_string).count();
                if matches > 1 {
                    return ValidationResult::error(format!(
                        "Found {} matches of the string to replace. For safety, this tool only supports replacing exactly one occurrence at a time. Add more lines of context to your edit and try again.",
                        matches
                    ));
                }

                ValidationResult::ok()
            }
            Err(e) => ValidationResult::error(format!(
                "Failed to read file: {}",
                e
            )),
        }
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        Ok(format!(
            "Edited file {} (replaced {} chars with {} chars)",
            output.file_path,
            output.old_string.len(),
            output.new_string.len()
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

        // Read original file content or use empty string for new files
        let original_file = if input.old_string.is_empty() {
            String::new()
        } else {
            fs::read_to_string(&path)?
        };

        // Apply the edit
        let updated_file = Self::apply_edit(&original_file, &input.old_string, &input.new_string)
            .map_err(|e| crate::error::KodeError::ToolValidation(e))?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Detect line ending from original or use system default
        let line_ending = if !original_file.is_empty() {
            Self::detect_line_ending(&original_file)
        } else if cfg!(windows) {
            "\r\n"
        } else {
            "\n"
        };

        // Normalize line endings in updated content
        let normalized_content = if line_ending == "\r\n" {
            updated_file.replace("\n", "\r\n")
        } else {
            updated_file.replace("\r\n", "\n")
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

        // Get snippet for assistant
        let (snippet, start_line) = Self::get_snippet(
            &original_file,
            &input.old_string,
            &input.new_string,
        );

        let output = FileEditOutput {
            file_path: input.file_path.clone(),
            old_string: input.old_string.clone(),
            new_string: input.new_string.clone(),
            original_file,
            snippet: snippet.clone(),
            start_line,
        };

        let result_for_assistant = format!(
            "The file {} has been updated. Here's the result of running `cat -n` on a snippet of the edited file:\n{}",
            input.file_path,
            Self::add_line_numbers(&snippet, start_line)
        );

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
    use futures::stream::StreamExt;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_edit_existing_file() {
        let tool = FileEditTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create initial file
        let initial_content = "Line 1\nLine 2\nLine 3\nLine 4\n";
        fs::write(&file_path, initial_content).unwrap();

        // Get the file timestamp
        let metadata = fs::metadata(&file_path).unwrap();
        let modified = metadata.modified().unwrap();
        let timestamp = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();

        let mut read_timestamps = HashMap::new();
        read_timestamps.insert(file_path.to_string_lossy().to_string(), timestamp);

        let input = FileEditInput {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "Line 2".to_string(),
            new_string: "Line 2 Modified".to_string(),
        };

        let ctx = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            read_file_timestamps: read_timestamps,
            safe_mode: false,
        };

        let mut stream = tool.call(input, ctx).await.unwrap();

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            assert_eq!(data.old_string, "Line 2");
            assert_eq!(data.new_string, "Line 2 Modified");

            let written_content = fs::read_to_string(&file_path).unwrap();
            assert!(written_content.contains("Line 2 Modified"));
            assert!(!written_content.contains("Line 2\n"));
        } else {
            panic!("Expected result");
        }
    }

    #[tokio::test]
    async fn test_create_new_file() {
        let tool = FileEditTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        let input = FileEditInput {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: String::new(),
            new_string: "New file content\nLine 2".to_string(),
        };

        let ctx = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            read_file_timestamps: HashMap::new(),
            safe_mode: false,
        };

        let validation = tool.validate_input(&input, &ctx).await;
        assert!(validation.is_valid);

        let mut stream = tool.call(input, ctx).await.unwrap();

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            assert!(file_path.exists());

            let written_content = fs::read_to_string(&file_path).unwrap();
            assert_eq!(written_content, "New file content\nLine 2");
        } else {
            panic!("Expected result");
        }
    }

    #[tokio::test]
    async fn test_validation_requires_unique_match() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create file with duplicate content
        let content = "duplicate\nduplicate\n";
        fs::write(&file_path, content).unwrap();

        let metadata = fs::metadata(&file_path).unwrap();
        let modified = metadata.modified().unwrap();
        let timestamp = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();

        let mut read_timestamps = HashMap::new();
        read_timestamps.insert(file_path.to_string_lossy().to_string(), timestamp);

        let tool = FileEditTool;
        let input = FileEditInput {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "duplicate".to_string(),
            new_string: "changed".to_string(),
        };

        let ctx = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            read_file_timestamps: read_timestamps,
            safe_mode: false,
        };

        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
        assert!(result.message.unwrap().contains("Found 2 matches"));
    }

    #[tokio::test]
    async fn test_validation_requires_read_before_edit() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "existing content").unwrap();

        let tool = FileEditTool;
        let input = FileEditInput {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "existing".to_string(),
            new_string: "new".to_string(),
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
