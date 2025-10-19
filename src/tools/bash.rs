//! BashTool - Execute shell commands
//!
//! Executes bash commands with timeout support and output capture.
//! This is a simplified implementation - full persistent shell support will be added later.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::tools::{Tool, ToolContext, ToolStreamItem, ValidationResult};
use crate::Result;

const MAX_OUTPUT_LENGTH: usize = 30000;
const DEFAULT_TIMEOUT_MS: u64 = 120000; // 2 minutes
const MAX_TIMEOUT_MS: u64 = 600000; // 10 minutes

/// Banned commands for security
const BANNED_COMMANDS: &[&str] = &[
    "alias", "curl", "curlie", "wget", "axel", "aria2c", "nc", "telnet", "lynx", "w3m", "links",
    "httpie", "xh", "http-prompt", "chrome", "firefox", "safari",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashInput {
    /// The command to execute
    pub command: String,
    /// Optional timeout in milliseconds (max 600000ms / 10 minutes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stdout_lines: usize,
    pub stderr: String,
    pub stderr_lines: usize,
    pub exit_code: i32,
    pub interrupted: bool,
}

pub struct BashTool;

impl BashTool {
    fn get_prompt() -> &'static str {
        r#"Executes a given bash command with optional timeout, ensuring proper handling and security measures.

IMPORTANT: This tool is for terminal operations like git, npm, docker, etc. DO NOT use it for file operations (reading, writing, editing, searching, finding files) - use the specialized tools for this instead.

Before executing the command, please follow these steps:

1. Directory Verification:
   - If the command will create new directories or files, first use `ls` to verify the parent directory exists and is the correct location
   - For example, before running "mkdir foo/bar", first use `ls foo` to check that "foo" exists and is the intended parent directory

2. Command Execution:
   - Always quote file paths that contain spaces with double quotes (e.g., cd "path with spaces/file.txt")
   - Examples of proper quoting:
     - cd "/Users/name/My Documents" (correct)
     - cd /Users/name/My Documents (incorrect - will fail)
     - python "/path/with spaces/script.py" (correct)
     - python /path/with spaces/script.py (incorrect - will fail)
   - After ensuring proper quoting, execute the command.
   - Capture the output of the command.

Usage notes:
  - The command argument is required.
  - You can specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). If not specified, commands will timeout after 120000ms (2 minutes).
  - If the output exceeds 30000 characters, output will be truncated before being returned to you.

  - Avoid using Bash with the `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed or when these commands are truly necessary for the task. Instead, always prefer using the dedicated tools for these commands:
    - File search: Use Glob (NOT find or ls)
    - Content search: Use Grep (NOT grep or rg)
    - Read files: Use Read (NOT cat/head/tail)
    - Edit files: Use Edit (NOT sed/awk)
    - Write files: Use Write (NOT echo >/cat <<EOF)
    - Communication: Output text directly (NOT echo/printf)

  - When issuing multiple commands:
    - If the commands are independent and can run in parallel, make multiple Bash tool calls in a single message.
    - If the commands depend on each other and must run sequentially, use a single Bash call with '&&' to chain them together (e.g., `git add . && git commit -m "message" && git push`).
    - Use ';' only when you need to run commands sequentially but don't care if earlier commands fail
    - DO NOT use newlines to separate commands (newlines are ok in quoted strings)

Security:
  - Some commands are banned for security reasons. If you use a disallowed command, you will receive an error message."#
    }

    /// Format output by truncating if needed
    fn format_output(output: String) -> (String, usize) {
        let lines: Vec<&str> = output.lines().collect();
        let total_lines = lines.len();

        if output.len() > MAX_OUTPUT_LENGTH {
            let truncated = format!(
                "{}...\n\n<output truncated - showed first {} of {} chars>",
                &output[..MAX_OUTPUT_LENGTH],
                MAX_OUTPUT_LENGTH,
                output.len()
            );
            (truncated, total_lines)
        } else {
            (output, total_lines)
        }
    }

    /// Extract the base command from a command string
    fn extract_base_command(command: &str) -> Option<String> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Split by common shell operators
        let parts: Vec<&str> = trimmed
            .split(&['|', '&', ';', '\n'][..])
            .next()?
            .trim()
            .split_whitespace()
            .collect();

        parts.first().map(|s| s.to_string())
    }
}

#[async_trait]
impl Tool for BashTool {
    type Input = BashInput;
    type Output = BashOutput;

    fn name(&self) -> &str {
        "Bash"
    }

    async fn description(&self) -> String {
        "Executes shell commands on your computer".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (max 600000)"
                }
            },
            "required": ["command"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        Self::get_prompt().to_string()
    }

    fn user_facing_name(&self) -> String {
        "Bash".to_string()
    }

    async fn is_enabled(&self) -> bool {
        true
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false // BashTool modifies state/files, not safe for concurrent execution
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        // Always check permissions for bash commands
        true
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        _ctx: &ToolContext,
    ) -> ValidationResult {
        // Check timeout
        if let Some(timeout) = input.timeout {
            if timeout > MAX_TIMEOUT_MS {
                return ValidationResult::error(format!(
                    "Timeout cannot exceed {} milliseconds",
                    MAX_TIMEOUT_MS
                ));
            }
        }

        // Extract and check for banned commands
        if let Some(base_cmd) = Self::extract_base_command(&input.command) {
            let base_cmd_lower = base_cmd.to_lowercase();
            if BANNED_COMMANDS.contains(&base_cmd_lower.as_str()) {
                return ValidationResult::error(format!(
                    "Command '{}' is not allowed for security reasons",
                    base_cmd
                ));
            }
        }

        ValidationResult::ok()
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        let mut result = String::new();

        if !output.stdout.is_empty() {
            result.push_str(&output.stdout);
        }

        if !output.stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&output.stderr);
        }

        if output.exit_code != 0 {
            result.push_str(&format!("\nExit code: {}", output.exit_code));
        }

        Ok(result)
    }

    fn render_tool_use(&self, input: &Self::Input, _verbose: bool) -> String {
        // Clean up HEREDOC patterns for display
        if input.command.contains("\"$(cat <<'EOF'") {
            // Simplified pattern matching - just show the command name
            format!("command: {}", input.command.lines().next().unwrap_or(&input.command))
        } else {
            format!("command: {}", input.command)
        }
    }

    async fn call(
        &self,
        input: Self::Input,
        ctx: ToolContext,
    ) -> Result<crate::tools::ToolStream<Self::Output>> {
        let timeout = Duration::from_millis(input.timeout.unwrap_or(DEFAULT_TIMEOUT_MS));
        let command_str = input.command.clone();

        // Spawn the command asynchronously
        let mut child = if cfg!(windows) {
            Command::new("cmd")
                .args(&["/C", &command_str])
                .current_dir(&ctx.cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        } else {
            Command::new("sh")
                .arg("-c")
                .arg(&command_str)
                .current_dir(&ctx.cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        };

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        // Read output
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let mut stdout_lines_vec = Vec::new();
        let mut stderr_lines_vec = Vec::new();

        let mut stdout_reader_lines = stdout_reader.lines();
        let mut stderr_reader_lines = stderr_reader.lines();

        // Read all lines from stdout
        while let Ok(Some(line)) = stdout_reader_lines.next_line().await {
            stdout_lines_vec.push(line);
        }

        // Read all lines from stderr
        while let Ok(Some(line)) = stderr_reader_lines.next_line().await {
            stderr_lines_vec.push(line);
        }

        // Wait for command to complete with timeout
        let status = tokio::time::timeout(timeout, child.wait()).await;

        let (exit_code, interrupted) = match status {
            Ok(Ok(status)) => (status.code().unwrap_or(-1), false),
            Ok(Err(_)) => (-1, false),
            Err(_) => {
                // Timeout occurred, kill the process
                let _ = child.kill().await;
                (-1, true)
            }
        };

        let stdout_full = stdout_lines_vec.join("\n");
        let stderr_full = stderr_lines_vec.join("\n");

        let (stdout_formatted, stdout_lines) = Self::format_output(stdout_full);
        let (stderr_formatted, stderr_lines) = Self::format_output(stderr_full);

        let output = BashOutput {
            stdout: stdout_formatted.clone(),
            stdout_lines,
            stderr: stderr_formatted.clone(),
            stderr_lines,
            exit_code,
            interrupted,
        };

        // Render result for assistant
        let mut result_for_assistant = String::new();
        if !stdout_formatted.trim().is_empty() {
            result_for_assistant.push_str(&stdout_formatted.trim());
        }
        if !stderr_formatted.trim().is_empty() {
            if !result_for_assistant.is_empty() {
                result_for_assistant.push('\n');
            }
            result_for_assistant.push_str(&stderr_formatted.trim());
        }
        if interrupted {
            if !result_for_assistant.is_empty() {
                result_for_assistant.push('\n');
            }
            result_for_assistant.push_str("<error>Command was aborted before completion</error>");
        }

        let stream = futures::stream::once(async move {
            Ok(ToolStreamItem::Result {
                data: output,
                result_for_assistant: if result_for_assistant.is_empty() {
                    None
                } else {
                    Some(result_for_assistant)
                },
            })
        });

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_simple_command() {
        let tool = BashTool;
        let input = BashInput {
            command: "echo 'Hello, World!'".to_string(),
            timeout: None,
        };

        let ctx = ToolContext {
            cwd: std::env::current_dir().unwrap(),
            read_file_timestamps: HashMap::new(),
            safe_mode: false,
            agent_id: None,
        };

        let mut stream = tool.call(input, ctx).await.unwrap();
        use futures::stream::StreamExt;

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            assert!(data.stdout.contains("Hello, World!"));
            assert_eq!(data.exit_code, 0);
            assert!(!data.interrupted);
        } else {
            panic!("Expected result");
        }
    }

    #[tokio::test]
    async fn test_validation_banned_command() {
        let tool = BashTool;
        let input = BashInput {
            command: "curl https://example.com".to_string(),
            timeout: None,
        };

        let ctx = ToolContext {
            cwd: std::env::current_dir().unwrap(),
            read_file_timestamps: HashMap::new(),
            safe_mode: false,
            agent_id: None,
        };

        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
        assert!(result.message.unwrap().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_command_with_error() {
        let tool = BashTool;
        let input = BashInput {
            command: "ls /nonexistent_directory_12345".to_string(),
            timeout: None,
        };

        let ctx = ToolContext {
            cwd: std::env::current_dir().unwrap(),
            read_file_timestamps: HashMap::new(),
            safe_mode: false,
            agent_id: None,
        };

        let mut stream = tool.call(input, ctx).await.unwrap();
        use futures::stream::StreamExt;

        if let Some(Ok(ToolStreamItem::Result { data, .. })) = stream.next().await {
            assert!(!data.stderr.is_empty() || data.exit_code != 0);
        } else {
            panic!("Expected result");
        }
    }
}
