//! OpenAI API streaming handler
//!
//! Processes Server-Sent Events from OpenAI's streaming API
//! and assembles them into complete messages.

use std::collections::HashMap;

use serde_json;

use crate::{
    error::{KodeError, Result},
    messages::{AssistantMessage, ContentBlock, Message, Role},
    services::Usage,
};

use super::{OpenAIStreamChunk, SseEvent, SseParser};

/// Tool call being assembled from deltas
#[derive(Debug, Clone)]
struct ToolCallBuilder {
    id: String,
    name: String,
    arguments: String,
}

/// Handler for OpenAI streaming responses
pub struct OpenAIStreamHandler {
    /// SSE parser
    parser: SseParser,

    /// Message ID
    id: Option<String>,

    /// Model name
    model: Option<String>,

    /// Created timestamp
    created: Option<u64>,

    /// Text content being assembled
    text_content: String,

    /// Tool calls being assembled (index -> builder)
    tool_calls: HashMap<usize, ToolCallBuilder>,

    /// Thinking/reasoning content (for o1/o3 models)
    thinking_content: Option<String>,

    /// Accumulated usage statistics
    usage: Option<Usage>,

    /// Stop reason
    finish_reason: Option<String>,
}

impl OpenAIStreamHandler {
    /// Create a new handler
    pub fn new() -> Self {
        Self {
            parser: SseParser::new(),
            id: None,
            model: None,
            created: None,
            text_content: String::new(),
            tool_calls: HashMap::new(),
            thinking_content: None,
            usage: None,
            finish_reason: None,
        }
    }

    /// Process a chunk of streaming data
    ///
    /// Returns true if the stream is complete ([DONE] marker received)
    pub fn process_chunk(&mut self, chunk: &str) -> Result<bool> {
        let events = self.parser.parse_chunk(chunk);

        for event in events {
            if event.is_done_marker() {
                return Ok(true); // Stream complete
            }

            self.process_event(event)?;
        }

        Ok(false)
    }

    /// Process a single SSE event
    fn process_event(&mut self, event: SseEvent) -> Result<()> {
        // Parse JSON data
        let chunk: OpenAIStreamChunk = serde_json::from_str(&event.data)
            .map_err(|e| KodeError::Other(format!("Failed to parse SSE event: {}", e)))?;

        // Extract metadata
        if self.id.is_none() {
            self.id = Some(chunk.id);
        }
        if self.model.is_none() {
            self.model = Some(chunk.model);
        }
        if self.created.is_none() {
            self.created = Some(chunk.created);
        }
        if chunk.usage.is_some() {
            self.usage = chunk.usage;
        }

        // Process choices
        if let Some(choice) = chunk.choices.first() {
            let delta = &choice.delta;

            // Text content
            if let Some(content) = &delta.content {
                self.text_content.push_str(content);
            }

            // Thinking/reasoning (o1/o3 models)
            if let Some(reasoning) = &delta.reasoning {
                self.thinking_content
                    .get_or_insert_with(String::new)
                    .push_str(reasoning);
            }

            // Tool calls
            if let Some(tool_call_deltas) = &delta.tool_calls {
                for tool_delta in tool_call_deltas {
                    self.process_tool_call_delta(tool_delta)?;
                }
            }

            // Finish reason
            if let Some(reason) = &choice.finish_reason {
                self.finish_reason = Some(reason.clone());
            }
        }

        Ok(())
    }

    /// Process a tool call delta
    fn process_tool_call_delta(
        &mut self,
        delta: &super::ToolCallDelta,
    ) -> Result<()> {
        let builder = self
            .tool_calls
            .entry(delta.index)
            .or_insert_with(|| ToolCallBuilder {
                id: String::new(),
                name: String::new(),
                arguments: String::new(),
            });

        // Update ID
        if let Some(id) = &delta.id {
            builder.id = id.clone();
        }

        // Update function name and arguments
        if let Some(function) = &delta.function {
            if let Some(name) = &function.name {
                builder.name = name.clone();
            }
            if let Some(args) = &function.arguments {
                builder.arguments.push_str(args);
            }
        }

        Ok(())
    }

    /// Get the assembled message
    ///
    /// Should be called after stream is complete
    pub fn get_message(&self) -> Result<AssistantMessage> {
        let mut content_blocks = Vec::new();

        // Add text content if present
        if !self.text_content.is_empty() {
            content_blocks.push(ContentBlock::Text {
                text: self.text_content.clone(),
            });
        }

        // Add thinking content if present
        if let Some(thinking) = &self.thinking_content {
            content_blocks.push(ContentBlock::Thinking {
                thinking: thinking.clone(),
            });
        }

        // Add tool calls
        let mut tool_indices: Vec<_> = self.tool_calls.keys().copied().collect();
        tool_indices.sort();

        for index in tool_indices {
            if let Some(builder) = self.tool_calls.get(&index) {
                // Parse arguments JSON
                let input: serde_json::Value = if builder.arguments.is_empty() {
                    serde_json::Value::Object(serde_json::Map::new())
                } else {
                    serde_json::from_str(&builder.arguments).map_err(|e| {
                        KodeError::Other(format!("Failed to parse tool arguments: {}", e))
                    })?
                };

                content_blocks.push(ContentBlock::ToolUse {
                    id: builder.id.clone(),
                    name: builder.name.clone(),
                    input,
                });
            }
        }

        let _id = self
            .id
            .clone()
            .ok_or_else(|| KodeError::Other("No message ID received".to_string()))?;

        let _model = self
            .model
            .clone()
            .ok_or_else(|| KodeError::Other("No model received".to_string()))?;

        Ok(AssistantMessage {
            message: Message {
                role: Role::Assistant,
                content: content_blocks,
                uuid: Some(uuid::Uuid::new_v4()),
            },
            uuid: uuid::Uuid::new_v4(),
            cost_usd: 0.0,
            duration_ms: 0,
            is_api_error_message: None,
            response_id: None,
        })
    }

    /// Get current text content (for incremental updates)
    pub fn get_current_text(&self) -> &str {
        &self.text_content
    }
}

impl Default for OpenAIStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text_stream() {
        let mut handler = OpenAIStreamHandler::new();

        // First chunk with metadata and initial content
        let chunk1 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}

"#;
        assert!(!handler.process_chunk(chunk1).unwrap());

        // Second chunk with more content
        let chunk2 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

"#;
        assert!(!handler.process_chunk(chunk2).unwrap());

        // Final chunk with finish reason
        let chunk3 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

"#;
        assert!(!handler.process_chunk(chunk3).unwrap());

        // Done marker
        let chunk4 = "data: [DONE]\n\n";
        assert!(handler.process_chunk(chunk4).unwrap());

        let message = handler.get_message().unwrap();
        assert_eq!(message.message.content.len(), 1);
        if let ContentBlock::Text { text } = &message.message.content[0] {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected text block");
        }
    }

    #[test]
    fn test_tool_call_stream() {
        let mut handler = OpenAIStreamHandler::new();

        // First chunk with tool call start
        let chunk1 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}

"#;
        handler.process_chunk(chunk1).unwrap();

        // Second chunk with arguments
        let chunk2 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\":"}}]},"finish_reason":null}]}

"#;
        handler.process_chunk(chunk2).unwrap();

        // Third chunk with more arguments
        let chunk3 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"Boston\"}"}}]},"finish_reason":null}]}

"#;
        handler.process_chunk(chunk3).unwrap();

        // Done marker
        let chunk4 = "data: [DONE]\n\n";
        handler.process_chunk(chunk4).unwrap();

        let message = handler.get_message().unwrap();
        assert_eq!(message.message.content.len(), 1);
        if let ContentBlock::ToolUse { name, input, .. } = &message.message.content[0] {
            assert_eq!(name, "get_weather");
            assert_eq!(input["location"], "Boston");
        } else {
            panic!("Expected tool_use block");
        }
    }

    #[test]
    fn test_reasoning_stream() {
        let mut handler = OpenAIStreamHandler::new();

        // Chunk with reasoning content (o1 model)
        let chunk1 = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"o1-preview","choices":[{"index":0,"delta":{"reasoning":"Let me think..."},"finish_reason":null}]}

"#;
        handler.process_chunk(chunk1).unwrap();

        // Done marker
        let chunk2 = "data: [DONE]\n\n";
        handler.process_chunk(chunk2).unwrap();

        let message = handler.get_message().unwrap();
        assert_eq!(message.message.content.len(), 1);
        if let ContentBlock::Thinking { thinking, .. } = &message.message.content[0] {
            assert_eq!(thinking, "Let me think...");
        } else {
            panic!("Expected thinking block");
        }
    }
}
