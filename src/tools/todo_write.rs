//! TodoWriteTool - Task tracking and progress management

use crate::{
    error::Result,
    tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult},
};
use async_stream::stream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const DESCRIPTION: &str = "Create and manage todo items for task tracking and progress management";

const PROMPT: &str = r#"Use this tool to create and manage todo items for tracking tasks and progress.

## When to Use

1. Complex multi-step tasks (3+ steps)
2. User explicitly requests todo list
3. User provides multiple tasks
4. After receiving new instructions
5. When starting work on a task (mark as in_progress)
6. After completing a task (mark as completed)

## Task States

- `pending`: Task not yet started
- `in_progress`: Currently working on (limit to ONE task)
- `completed`: Task finished

## Required Fields

- `content`: Imperative form (e.g., "Run tests")
- `activeForm`: Present continuous form (e.g., "Running tests")
- `status`: pending/in_progress/completed"#;

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// A single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    #[serde(rename = "activeForm")]
    pub active_form: String,
    pub status: TodoStatus,
}

/// Input for TodoWriteTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoWriteInput {
    pub todos: Vec<TodoItem>,
}

/// Output from TodoWriteTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoWriteOutput {
    pub summary: String,
}

/// TodoWriteTool for task tracking
pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    type Input = TodoWriteInput;
    type Output = TodoWriteOutput;

    fn name(&self) -> &'static str {
        "TodoWrite"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The task description (imperative form)"
                            },
                            "activeForm": {
                                "type": "string",
                                "description": "The present continuous form shown during execution"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Current status of the task"
                            }
                        },
                        "required": ["content", "activeForm", "status"]
                    },
                    "description": "The updated todo list"
                }
            },
            "required": ["todos"]
        })
    }

    async fn description(&self) -> String {
        DESCRIPTION.to_string()
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        PROMPT.to_string()
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        _context: &ToolContext,
    ) -> ValidationResult {
        // Check for empty content
        for (i, todo) in input.todos.iter().enumerate() {
            if todo.content.trim().is_empty() {
                return ValidationResult::error(format!("Todo at index {i} has empty content"));
            }
            if todo.active_form.trim().is_empty() {
                return ValidationResult::error(format!("Todo at index {i} has empty activeForm"));
            }
        }

        // Check for multiple in_progress tasks
        let in_progress_count = input
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();

        if in_progress_count > 1 {
            return ValidationResult::error(format!(
                "Only one task can be in_progress at a time (found {in_progress_count})"
            ));
        }

        ValidationResult::ok()
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        Ok(output.summary.clone())
    }

    fn render_tool_use(&self, input: &Self::Input, _verbose: bool) -> String {
        format!("Updating {} todo(s)", input.todos.len())
    }

    async fn call(
        &self,
        input: Self::Input,
        _context: ToolContext,
    ) -> Result<ToolStream<Self::Output>> {
        let total = input.todos.len();
        let pending = input
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Pending)
            .count();
        let in_progress = input
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();
        let completed = input
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();

        let summary = if total == 0 {
            "Todo list cleared. No active tasks.".to_string()
        } else {
            format!(
                "Updated {total} todo(s) ({pending} pending, {in_progress} in progress, {completed} completed). Continue tracking your progress with the todo list."
            )
        };

        let output = TodoWriteOutput { summary };

        Ok(Box::pin(stream! {
            yield Ok(ToolStreamItem::Result {
                data: output,
                result_for_assistant: None,
            });
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_empty_content() {
        let tool = TodoWriteTool;
        let input = TodoWriteInput {
            todos: vec![TodoItem {
                content: "".to_string(),
                active_form: "Working".to_string(),
                status: TodoStatus::Pending,
            }],
        };
        let ctx = ToolContext::default();

        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
    }

    #[tokio::test]
    async fn test_validate_multiple_in_progress() {
        let tool = TodoWriteTool;
        let input = TodoWriteInput {
            todos: vec![
                TodoItem {
                    content: "Task 1".to_string(),
                    active_form: "Working on task 1".to_string(),
                    status: TodoStatus::InProgress,
                },
                TodoItem {
                    content: "Task 2".to_string(),
                    active_form: "Working on task 2".to_string(),
                    status: TodoStatus::InProgress,
                },
            ],
        };
        let ctx = ToolContext::default();

        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
    }

    #[tokio::test]
    async fn test_validate_valid_todos() {
        let tool = TodoWriteTool;
        let input = TodoWriteInput {
            todos: vec![
                TodoItem {
                    content: "Task 1".to_string(),
                    active_form: "Working on task 1".to_string(),
                    status: TodoStatus::Pending,
                },
                TodoItem {
                    content: "Task 2".to_string(),
                    active_form: "Working on task 2".to_string(),
                    status: TodoStatus::InProgress,
                },
                TodoItem {
                    content: "Task 3".to_string(),
                    active_form: "Working on task 3".to_string(),
                    status: TodoStatus::Completed,
                },
            ],
        };
        let ctx = ToolContext::default();

        let result = tool.validate_input(&input, &ctx).await;
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_todo_write_tool() {
        let tool = TodoWriteTool;
        let input = TodoWriteInput {
            todos: vec![
                TodoItem {
                    content: "Write tests".to_string(),
                    active_form: "Writing tests".to_string(),
                    status: TodoStatus::Pending,
                },
                TodoItem {
                    content: "Run build".to_string(),
                    active_form: "Running build".to_string(),
                    status: TodoStatus::InProgress,
                },
            ],
        };
        let ctx = ToolContext::default();

        // Validate
        let validation = tool.validate_input(&input, &ctx).await;
        assert!(validation.is_valid);

        // Call tool
        let mut stream = tool.call(input, ctx).await.unwrap();

        use futures::StreamExt;
        let mut results = Vec::new();
        while let Some(item) = stream.next().await {
            results.push(item.unwrap());
        }

        assert_eq!(results.len(), 1);

        // Check the result
        if let ToolStreamItem::Result { data, .. } = &results[0] {
            assert!(data.summary.contains("2 todo(s)"));
            assert!(data.summary.contains("1 pending"));
            assert!(data.summary.contains("1 in progress"));
        } else {
            panic!("Expected Result item");
        }
    }
}
