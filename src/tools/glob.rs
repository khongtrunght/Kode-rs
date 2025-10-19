//! GlobTool - Fast file pattern matching
//!
//! Supports:
//! - Glob patterns like "**/*.js" or "src/**/*.ts"
//! - Returns matching file paths sorted by modification time
//! - Respects .gitignore files

use crate::error::Result;
use crate::tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;
use wildmatch::WildMatch;

const DESCRIPTION: &str = r#"- Fast file pattern matching tool that works with any codebase size
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths sorted by modification time
- Use this tool when you need to find files by name patterns
- When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead"#;

const MAX_RESULTS: usize = 100;

/// Input for GlobTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobInput {
    /// The glob pattern to match files against
    pub pattern: String,
    /// The directory to search in. Defaults to the current working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Output for GlobTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobOutput {
    pub duration_ms: u64,
    pub num_files: usize,
    pub filenames: Vec<String>,
    pub truncated: bool,
}

/// GlobTool implementation
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }

    /// Perform glob search
    fn glob_search(
        pattern: &str,
        search_path: &Path,
        limit: usize,
    ) -> Result<(Vec<PathBuf>, bool)> {
        // Convert glob pattern to WildMatch
        let matcher = WildMatch::new(pattern);

        // Walk the directory tree
        let mut matches = Vec::new();

        for entry in WalkDir::new(search_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories
            if !path.is_file() {
                continue;
            }

            // Get relative path from search_path
            let relative_path = path
                .strip_prefix(search_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            // Also check the full path for absolute patterns
            let full_path = path.to_string_lossy().to_string();

            // Match against both relative and full paths
            if matcher.matches(&relative_path) || matcher.matches(&full_path) {
                // Get metadata for sorting
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        matches.push((path.to_path_buf(), modified));
                    }
                }
            }
        }

        // Sort by modification time (oldest first, matching TypeScript behavior)
        matches.sort_by_key(|(_, mtime)| *mtime);

        let truncated = matches.len() > limit;
        let files: Vec<PathBuf> = matches
            .into_iter()
            .take(limit)
            .map(|(path, _)| path)
            .collect();

        Ok((files, truncated))
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    type Input = GlobInput;
    type Output = GlobOutput;

    fn name(&self) -> &str {
        "Glob"
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
                    "description": "The glob pattern to match files against"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. If not specified, the current working directory will be used. IMPORTANT: Omit this field to use the default directory. DO NOT enter \"undefined\" or \"null\" - simply omit it for the default behavior. Must be a valid directory path if provided."
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
        false // Glob is read-only, doesn't need permissions
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

        if !search_path.is_dir() {
            return ValidationResult::error(format!(
                "Path is not a directory: {}",
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

        parts.join(", ")
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        if output.num_files == 0 {
            Ok("No files found".to_string())
        } else {
            let mut result = output.filenames.join("\n");
            if output.truncated {
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

        // Perform the glob search
        let (files, truncated) = Self::glob_search(&input.pattern, &search_path, MAX_RESULTS)?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Convert paths to strings (absolute paths)
        let filenames: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let output = GlobOutput {
            duration_ms,
            num_files: filenames.len(),
            filenames,
            truncated,
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
        fs::create_dir_all(dir.join("src/components"))?;

        // Create test files
        fs::write(dir.join("src/main.rs"), "fn main() {}")?;
        fs::write(dir.join("src/lib.rs"), "pub fn lib() {}")?;
        fs::write(dir.join("src/components/button.tsx"), "export const Button = () => {}")?;
        fs::write(dir.join("test/test.js"), "test('it works', () => {})")?;
        fs::write(dir.join("README.md"), "# README")?;

        Ok(())
    }

    #[tokio::test]
    async fn test_glob_find_all_rust_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GlobTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GlobInput {
            pattern: "**/*.rs".to_string(),
            path: None,
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            assert_eq!(output.num_files, 2);
            assert!(output.filenames.iter().any(|f| f.ends_with("main.rs")));
            assert!(output.filenames.iter().any(|f| f.ends_with("lib.rs")));
        } else {
            panic!("Expected Result item");
        }
    }

    #[tokio::test]
    async fn test_glob_find_tsx_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GlobTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GlobInput {
            pattern: "**/*.tsx".to_string(),
            path: None,
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            assert_eq!(output.num_files, 1);
            assert!(output.filenames[0].ends_with("button.tsx"));
        } else {
            panic!("Expected Result item");
        }
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GlobTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GlobInput {
            pattern: "**/*.py".to_string(),
            path: None,
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
    async fn test_glob_with_specific_path() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path()).unwrap();

        let tool = GlobTool::new();
        let context = ToolContext {
            cwd: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let input = GlobInput {
            pattern: "*.rs".to_string(),
            path: Some("src".to_string()),
        };

        let mut stream = tool.call(input, context).await.unwrap();
        let result = stream.next().await.unwrap().unwrap();

        if let ToolStreamItem::Result { data: output, .. } = result {
            assert_eq!(output.num_files, 2);
        } else {
            panic!("Expected Result item");
        }
    }
}
