//! Anthropic API streaming handler
//!
//! Processes Server-Sent Events from Anthropic's streaming API
//! and assembles them into complete messages.

use std::collections::HashMap;

use serde_json;

use crate::{
    error::{KodeError, Result},
    messages::{AssistantMessage, ContentBlock, Message, Role},
    services::Usage,
};

use super::{
    AnthropicStreamEvent, ContentBlockStart, ContentDelta, MessageMetadata, SseEvent, SseParser,
};

/// Handler for Anthropic streaming responses
pub struct AnthropicStreamHandler {
    /// SSE parser
    parser: SseParser,

    /// Message metadata from message_start
    message_metadata: Option<MessageMetadata>,

    /// Content blocks being assembled
    content_blocks: Vec<ContentBlock>,

    /// JSON buffers for tool_use inputs (index -> partial JSON)
    input_json_buffers: HashMap<usize, String>,

    /// Accumulated usage statistics
    usage: Usage,

    /// Stop reason
    stop_reason: Option<String>,

    /// Stop sequence
    stop_sequence: Option<String>,
}

impl AnthropicStreamHandler {
    /// Create a new handler
    pub fn new() -> Self {
        Self {
            parser: SseParser::new(),
            message_metadata: None,
            content_blocks: Vec::new(),
            input_json_buffers: HashMap::new(),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            stop_reason: None,
            stop_sequence: None,
        }
    }

    /// Process a chunk of streaming data
    ///
    /// Returns true if the stream is complete (message_stop received)
    pub fn process_chunk(&mut self, chunk: &str) -> Result<bool> {
        let events = self.parser.parse_chunk(chunk);

        for event in events {
            if self.process_event(event)? {
                return Ok(true); // Stream complete
            }
        }

        Ok(false)
    }

    /// Process a single SSE event
    ///
    /// Returns true if stream is complete
    fn process_event(&mut self, event: SseEvent) -> Result<bool> {
        // Parse JSON data
        let stream_event: AnthropicStreamEvent = serde_json::from_str(&event.data)
            .map_err(|e| KodeError::Other(format!("Failed to parse SSE event: {}", e)))?;

        match stream_event {
            AnthropicStreamEvent::MessageStart { message } => {
                self.message_metadata = Some(message.clone());
                self.usage = message.usage;
                Ok(false)
            }

            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                self.handle_content_block_start(index, content_block);
                Ok(false)
            }

            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                self.handle_content_block_delta(index, delta)?;
                Ok(false)
            }

            AnthropicStreamEvent::ContentBlockStop { index } => {
                self.handle_content_block_stop(index)?;
                Ok(false)
            }

            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                if let Some(reason) = delta.stop_reason {
                    self.stop_reason = Some(reason);
                }
                if let Some(seq) = delta.stop_sequence {
                    self.stop_sequence = Some(seq);
                }
                if let Some(usage_delta) = usage {
                    if let Some(output_tokens) = usage_delta.output_tokens {
                        self.usage.output_tokens = output_tokens;
                    }
                }
                Ok(false)
            }

            AnthropicStreamEvent::MessageStop => {
                // Clear buffers
                self.input_json_buffers.clear();
                Ok(true) // Signal stream complete
            }

            AnthropicStreamEvent::Ping => Ok(false),

            AnthropicStreamEvent::Error { error } => Err(KodeError::ApiError {
                provider: "Anthropic".to_string(),
                message: format!("Stream error: {} - {}", error.error_type, error.message),
            }),
        }
    }

    /// Handle content_block_start event
    fn handle_content_block_start(&mut self, index: usize, content_block: ContentBlockStart) {
        // Ensure vector is large enough
        while self.content_blocks.len() <= index {
            self.content_blocks.push(ContentBlock::Text {
                text: String::new(),
            });
        }

        match content_block {
            ContentBlockStart::Text { text } => {
                self.content_blocks[index] = ContentBlock::Text { text };
            }
            ContentBlockStart::ToolUse { id, name } => {
                self.content_blocks[index] = ContentBlock::ToolUse {
                    id,
                    name,
                    input: serde_json::Value::Object(serde_json::Map::new()),
                };
                // Initialize JSON buffer
                self.input_json_buffers.insert(index, String::new());
            }
            ContentBlockStart::Thinking { thinking } => {
                self.content_blocks[index] = ContentBlock::Thinking {
                    thinking,
                };
            }
        }
    }

    /// Handle content_block_delta event
    fn handle_content_block_delta(&mut self, index: usize, delta: ContentDelta) -> Result<()> {
        // Ensure content block exists
        while self.content_blocks.len() <= index {
            self.content_blocks.push(ContentBlock::Text {
                text: String::new(),
            });
        }

        match delta {
            ContentDelta::TextDelta { text } => {
                if let ContentBlock::Text { text: ref mut existing } = self.content_blocks[index] {
                    existing.push_str(&text);
                } else {
                    // Initialize if not already text block
                    self.content_blocks[index] = ContentBlock::Text { text };
                }
            }
            ContentDelta::InputJsonDelta { partial_json } => {
                // Accumulate JSON in buffer
                self.input_json_buffers
                    .entry(index)
                    .or_insert_with(String::new)
                    .push_str(&partial_json);
            }
            ContentDelta::ThinkingDelta { thinking } => {
                if let ContentBlock::Thinking {
                    thinking: ref mut existing,
                } = self.content_blocks[index]
                {
                    existing.push_str(&thinking);
                } else {
                    // Initialize if not already thinking block
                    self.content_blocks[index] = ContentBlock::Thinking {
                        thinking,
                    };
                }
            }
        }

        Ok(())
    }

    /// Handle content_block_stop event
    fn handle_content_block_stop(&mut self, index: usize) -> Result<()> {
        // If this is a tool_use block, parse the accumulated JSON
        if let Some(json_str) = self.input_json_buffers.remove(&index) {
            if let ContentBlock::ToolUse {
                ref mut input,
                ..
            } = self.content_blocks[index]
            {
                *input = serde_json::from_str(&json_str).map_err(|e| {
                    KodeError::Other(format!("Failed to parse tool input JSON: {}", e))
                })?;
            }
        }

        Ok(())
    }

    /// Get the assembled message
    ///
    /// Should be called after stream is complete
    pub fn get_message(&self) -> Result<AssistantMessage> {
        let _metadata = self
            .message_metadata
            .as_ref()
            .ok_or_else(|| KodeError::Other("No message metadata received".to_string()))?;

        Ok(AssistantMessage {
            message: Message {
                role: Role::Assistant,
                content: self.content_blocks.clone(),
                uuid: Some(uuid::Uuid::new_v4()),
            },
            uuid: uuid::Uuid::new_v4(),
            cost_usd: 0.0,
            duration_ms: 0,
            is_api_error_message: None,
            response_id: None,
        })
    }

    /// Get current content blocks (for incremental updates)
    pub fn get_current_content(&self) -> &[ContentBlock] {
        &self.content_blocks
    }
}

impl Default for AnthropicStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text_stream() {
        let mut handler = AnthropicStreamHandler::new();

        // message_start
        let chunk1 = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_123","model":"claude-3","role":"assistant","type":"message","usage":{"input_tokens":10,"output_tokens":0}}}

"#;
        assert!(!handler.process_chunk(chunk1).unwrap());

        // content_block_start
        let chunk2 = r#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

"#;
        assert!(!handler.process_chunk(chunk2).unwrap());

        // content_block_delta
        let chunk3 = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

"#;
        assert!(!handler.process_chunk(chunk3).unwrap());

        // message_stop
        let chunk4 = r#"event: message_stop
data: {"type":"message_stop"}

"#;
        assert!(handler.process_chunk(chunk4).unwrap());

        let message = handler.get_message().unwrap();
        assert_eq!(message.message.content.len(), 1);
        if let ContentBlock::Text { text } = &message.message.content[0] {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected text block");
        }
    }

    #[test]
    fn test_tool_use_stream() {
        let mut handler = AnthropicStreamHandler::new();

        // message_start
        let chunk1 = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_123","model":"claude-3","role":"assistant","type":"message","usage":{"input_tokens":10,"output_tokens":0}}}

"#;
        handler.process_chunk(chunk1).unwrap();

        // content_block_start for tool_use
        let chunk2 = r#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool_1","name":"test_tool"}}

"#;
        handler.process_chunk(chunk2).unwrap();

        // input_json_delta
        let chunk3 = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"arg\":"}}

"#;
        handler.process_chunk(chunk3).unwrap();

        let chunk4 = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"value\"}"}}

"#;
        handler.process_chunk(chunk4).unwrap();

        // content_block_stop (triggers JSON parsing)
        let chunk5 = r#"event: content_block_stop
data: {"type":"content_block_stop","index":0}

"#;
        handler.process_chunk(chunk5).unwrap();

        // message_stop
        let chunk6 = r#"event: message_stop
data: {"type":"message_stop"}

"#;
        handler.process_chunk(chunk6).unwrap();

        let message = handler.get_message().unwrap();
        if let ContentBlock::ToolUse { name, input, .. } = &message.message.content[0] {
            assert_eq!(name, "test_tool");
            assert_eq!(input["arg"], "value");
        } else {
            panic!("Expected tool_use block");
        }
    }
}
