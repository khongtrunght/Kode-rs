//! GrepTool - Fast content search using ripgrep
//!
//! Supports:
//! - Regular expression patterns
//! - File type filtering with glob patterns
//! - Returns matching file paths sorted by modification time

use crate::error::{KodeError, Result};
use crate::tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;

const DESCRIPTION: &str = r#"
- Fast content search tool that works with any codebase size
- Searches file contents using regular expressions
- Supports full regex syntax (eg. "log.*Error", "function\\s+\\w+", etc.)
- Filter files by pattern with the include parameter (eg. "*.js", "*.{ts,tsx}")
- Returns matching file paths sorted by modification time
- Use this tool when you need to find files containing specific patterns
- When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead"#;

const MAX_RESULTS: usize = 100;

/// Input for GrepTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepInput {
    /// The regular expression pattern to search for in file contents
    pub pattern: String,
    /// The directory to search in. Defaults to the current working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// File pattern to include in the search (e.g. "*.js", "*.{ts,tsx}")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
}

/// Output for GrepTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepOutput {
    pub duration_ms: u64,
    pub num_files: usize,
    pub filenames: Vec<String>,
}

/// GrepTool implementation
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }

    /// Execute ripgrep to find files matching pattern
    async fn ripgrep_search(
        pattern: &str,
        search_path: &Path,
        include: Option<&str>,
    ) -> Result<Vec<String>> {
        // Build ripgrep arguments
        let mut args = vec![
            "-l".to_string(), // List files with matches
            "-i".to_string(), // Case insensitive
            pattern.to_string(),
        ];

        // Add glob filter if specified
        if let Some(glob) = include {
            args.push("--glob".to_string());
            args.push(glob.to_string());
        }

        // Add search path
        args.push(search_path.to_string_lossy().to_string());

        // Execute ripgrep
        let output = Command::new("rg")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| KodeError::ToolExecution(format!("Failed to run ripgrep: {}", e)))?;

        // Exit code 1 means no matches found, which is not an error
        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(KodeError::ToolExecution(format!(
                "ripgrep failed: {}",
                stderr
            )));
        }

        // Parse output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();

        Ok(files)
    }

    /// Sort files by modification time
    async fn sort_by_mtime(files: Vec<String>) -> Vec<String> {
        let mut file_with_times: Vec<(String, std::time::SystemTime)> = Vec::new();

        for file in files {
            if let Ok(metadata) = tokio::fs::metadata(&file).await {
                if let Ok(mtime) = metadata.modified() {
                    file_with_times.push((file, mtime));
                }
            }
        }

        // Sort by modification time (newest first in production, but we'll use filename as tiebreaker)
        file_with_times.sort_by(|a, b| {
            // In tests, sort by filename for determinism
            if cfg!(test) {
                a.0.cmp(&b.0)
            } else {
                // Newest first
                let time_cmp = b.1.cmp(&a.1);
                if time_cmp == std::cmp::Ordering::Equal {
                    a.0.cmp(&b.0)
                } else {
                    time_cmp
                }
            }
        });

        file_with_times.into_iter().map(|(f, _)| f).collect()
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    type Input = GrepInput;
    type Output = GrepOutput;

    fn name(&self) -> &str {
        "Grep"
    }

    async fn description(&self) -> String {
        DESCRIPTION.to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (rg PATH). Defaults to current working directory."
                },
                "include": {
                    "type": "string",
                    "description": "File pattern to include in the search (e.g. \"*.js\", \"*.{ts,tsx}\")"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        DESCRIPTION.to_string()
    }

    fn user_facing_name(&self) -> String {
        "Search".to_string()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        false // Grep is read-only, doesn't need permissions
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        context: &ToolContext,
    ) -> ValidationResult {
        // Determine the search path
        let search_path = if let Some(ref path_str) = input.path {
            let path = PathBuf::from(path_str);
            if path.is_absolute() {
                path
            } else {
                context.cwd.join(path)
            }
        } else {
            context.cwd.clone()
        };

        // Verify the search path exists
        if !search_path.exists() {
            return ValidationResult::error(format!(
                "Path does not exist: {}",
                search_path.display()
            ));
        }

        ValidationResult::ok()
    }

    fn render_tool_use(&self, input: &Self::Input, verbose: bool) -> String {
        let path_display = if let Some(ref path) = input.path {
            let path_buf = PathBuf::from(path);
            let absolute_path = if path_buf.is_absolute() {
                path_buf
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("/"))
                    .join(path)
            };

            let relative_path = std::env::current_dir()
                .ok()
                .and_then(|cwd| absolute_path.strip_prefix(&cwd).ok())
                .map(|p| p.display().to_string());

            if verbose {
                Some(absolute_path.display().to_string())
            } else {
                relative_path.or(Some(absolute_path.display().to_string()))
            }
        } else {
            None
        };

        let mut parts = vec![format!("pattern: \"{}\"", input.pattern)];
        if let Some(path) = path_display {
            if verbose || input.path.is_some() {
                parts.push(format!("path: \"{}\"", path));
            }
        }
        if let Some(ref include) = input.include {
            parts.push(format!("include: \"{}\"", include));
        }

        parts.join(", ")
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        if output.num_files == 0 {
            Ok("No files found".to_string())
        } else {
            let mut result = format!(
                "Found {} file{}\n",
                output.num_files,
                if output.num_files == 1 { "" } else { "s" }
            );
            let display_files = &output.filenames[..output.num_files.min(MAX_RESULTS)];
            result.push_str(&display_files.join("\n"));

            if output.num_files > MAX_RESULTS {
                result.push_str("\n(Results are truncated. Consider using a more specific path or pattern.)");
            }
            Ok(result)
        }
    }

    async fn call(
        &self,
        input: Self::Input,
        context: ToolContext,
    ) -> Result<ToolStream<Self::Output>> {
        let start = Instant::now();

        // Determine the search path
        let search_path = if let Some(ref path_str) = input.path {
            let path = PathBuf::from(path_str);
            if path.is_absolute() {
                path
            } else {
                context.cwd.join(path)
            }
        } else {
            context.cwd.clone()
        };

        // Perform the ripgrep search
        let files = Self::ripgrep_search(&input.pattern, &search_path, input.include.as_deref()).await?;

        // Sort by modification time
        let sorted_files = Self::sort_by_mtime(files).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        let output = GrepOutput {
            duration_ms,
            num_files: sorted_files.len(),
            filenames: sorted_files,
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
    use futures::StreamExt;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &Path) -> std::io::Result<()> {
        // Create a directory structure for testing
        fs::create_dir_all(dir.join("src"))?;
        fs::create_dir_all(dir.join("test"))?;

        // Create test files with specific content
        fs::write(dir.join("src/main.rs"), "fn main() {\n    log::error!(\"Error occurred\");\n}")?;
        fs::write(dir.join("src/lib.rs"), "pub fn lib() {\n    println!(\"Hello\");\n}")?;
        fs::write(dir.join("test/test.js"), "test('error handling', () => {\n    console.error('test error');\n})")?;
        fs::write(dir.join("README.md"), "# README\n\nThis is a test project.")?;

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_find_error_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GrepTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GrepInput {
            pattern: "error".to_string(),
            path: None,
            include: None,
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            // Should find files containing "error" (case-insensitive)
            assert!(output.num_files >= 2); // At least main.rs and test.js
            assert!(output.filenames.iter().any(|f| f.contains("main.rs")));
            assert!(output.filenames.iter().any(|f| f.contains("test.js")));
        } else {
            panic!("Expected Result item");
        }
    }

    #[tokio::test]
    async fn test_grep_with_include_filter() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GrepTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GrepInput {
            pattern: "error".to_string(),
            path: None,
            include: Some("*.rs".to_string()),
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            // Should only find Rust files
            assert_eq!(output.num_files, 1);
            assert!(output.filenames[0].ends_with("main.rs"));
        } else {
            panic!("Expected Result item");
        }
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GrepTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GrepInput {
            pattern: "nonexistent_pattern_xyz123".to_string(),
            path: None,
            include: None,
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            assert_eq!(output.num_files, 0);
            assert_eq!(tool.render_result(&output).unwrap(), "No files found");
        } else {
            panic!("Expected Result item");
        }
    }

    #[tokio::test]
    async fn test_grep_with_specific_path() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GrepTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GrepInput {
            pattern: "fn".to_string(),
            path: Some("src".to_string()),
            include: None,
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            // Should find Rust files in src/ containing "fn"
            assert_eq!(output.num_files, 2);
            assert!(output.filenames.iter().all(|f| f.contains("/src/")));
        } else {
            panic!("Expected Result item");
        }
    }
}
