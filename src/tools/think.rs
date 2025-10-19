//! ThinkTool - Log reasoning and thoughts
//!
//! A no-op tool that allows the AI to explicitly log its thought process.
//! Inspired by the tau-bench think tool. Useful for complex reasoning, planning,
//! and brainstorming without taking any actions.

use async_stream::stream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;
use crate::tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult};

/// Input for ThinkTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkInput {
    /// The thought or reasoning to log
    pub thought: String,
}

/// Output for ThinkTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkOutput {
    /// The logged thought
    pub thought: String,
}

/// Tool for logging thoughts and reasoning
pub struct ThinkTool;

#[async_trait]
impl Tool for ThinkTool {
    type Input = ThinkInput;
    type Output = ThinkOutput;

    fn name(&self) -> &'static str {
        "Think"
    }

    async fn description(&self) -> String {
        "This is a no-op tool that logs a thought. It is inspired by the tau-bench think tool.".to_string()
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "thought": {
                    "type": "string",
                    "description": "Your thoughts."
                }
            },
            "required": ["thought"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        r#"Use the tool to think about something. It will not obtain new information or make any changes to the repository, but just log the thought. Use it when complex reasoning or brainstorming is needed.

Common use cases:
1. When exploring a repository and discovering the source of a bug, call this tool to brainstorm several unique ways of fixing the bug, and assess which change(s) are likely to be simplest and most effective
2. After receiving test results, use this tool to brainstorm ways to fix failing tests
3. When planning a complex refactoring, use this tool to outline different approaches and their tradeoffs
4. When designing a new feature, use this tool to think through architecture decisions and implementation details
5. When debugging a complex issue, use this tool to organize your thoughts and hypotheses

The tool simply logs your thought process for better transparency and does not execute any code or make changes."#.to_string()
    }

    fn user_facing_name(&self) -> String {
        "Think".to_string()
    }

    async fn is_enabled(&self) -> bool {
        // Only enabled if THINK_TOOL environment variable is set
        std::env::var("THINK_TOOL").is_ok()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        false
    }

    async fn validate_input(
        &self,
        _input: &Self::Input,
        _context: &ToolContext,
    ) -> ValidationResult {
        ValidationResult::ok()
    }

    fn render_result(&self, _output: &Self::Output) -> Result<String> {
        Ok("Your thought has been logged.".to_string())
    }

    fn render_tool_use(&self, input: &Self::Input, _verbose: bool) -> String {
        // Return the thought itself as the display message
        input.thought.clone()
    }

    async fn call(
        &self,
        input: Self::Input,
        _context: ToolContext,
    ) -> Result<ToolStream<Self::Output>> {
        Ok(Box::pin(stream! {
            yield Ok(ToolStreamItem::Result {
                data: ThinkOutput {
                    thought: input.thought,
                },
                result_for_assistant: Some("Your thought has been logged.".to_string()),
            });
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_think_tool_basic() {
        let tool = ThinkTool;
        assert_eq!(tool.name(), "Think");
        assert!(tool.is_read_only());
        assert!(tool.is_concurrency_safe());

        let input = ThinkInput {
            thought: "I should refactor this code to use a better pattern.".to_string(),
        };

        assert!(!tool.needs_permissions(&input));
        assert_eq!(
            tool.render_tool_use(&input, false),
            "I should refactor this code to use a better pattern."
        );
    }

    #[tokio::test]
    async fn test_think_tool_validation() {
        let tool = ThinkTool;
        let input = ThinkInput {
            thought: "Test thought".to_string(),
        };
        let context = ToolContext::default();

        let result = tool.validate_input(&input, &context).await;
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_think_tool_render_result() {
        let tool = ThinkTool;
        let output = ThinkOutput {
            thought: "This is a complex problem".to_string(),
        };

        let rendered = tool.render_result(&output).unwrap();
        assert_eq!(rendered, "Your thought has been logged.");
    }
}
