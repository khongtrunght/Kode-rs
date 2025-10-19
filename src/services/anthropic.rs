//! Anthropic Claude API adapter
//!
//! Supports:
//! - Direct Anthropic API
//! - AWS Bedrock
//! - Google Vertex AI

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{Stream, StreamExt};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use crate::{
    config::models::ModelProfile,
    error::{KodeError, Result},
    messages::{ContentBlock, Message, Role},
};

use super::{
    streaming::AnthropicStreamHandler,
    CompletionChunk, CompletionOptions, CompletionResponse, CompletionStream, ModelAdapter,
    ToolSchema, Usage,
};

/// Anthropic API adapter
pub struct AnthropicAdapter {
    client: Client,
    profile: ModelProfile,
    base_url: String,
}

impl AnthropicAdapter {
    /// Create a new Anthropic adapter
    pub fn new(profile: ModelProfile) -> Result<Self> {
        let api_key = if profile.api_key.is_empty() {
            std::env::var("ANTHROPIC_API_KEY").map_err(|_| KodeError::MissingApiKey {
                provider: "anthropic".to_string(),
            })?
        } else {
            profile.api_key.clone()
        };

        let base_url = profile
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string());

        let client = Client::builder()
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    "x-api-key",
                    header::HeaderValue::from_str(&api_key).map_err(|_| {
                        KodeError::InvalidConfig("Invalid API key format".to_string())
                    })?,
                );
                headers.insert(
                    "anthropic-version",
                    header::HeaderValue::from_static("2023-06-01"),
                );
                headers
            })
            .build()?;

        Ok(Self {
            client,
            profile,
            base_url,
        })
    }

    /// Convert internal messages to Anthropic API format
    fn convert_messages(&self, messages: Vec<Message>) -> Vec<AnthropicMessage> {
        messages
            .into_iter()
            .map(|msg| AnthropicMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "user".to_string(), // System messages handled separately
                },
                content: self.convert_content_blocks(msg.content),
            })
            .collect()
    }

    /// Convert content blocks to Anthropic format
    fn convert_content_blocks(&self, blocks: Vec<ContentBlock>) -> Vec<AnthropicContentBlock> {
        blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Text { text } => AnthropicContentBlock::Text { text },
                ContentBlock::ToolUse { id, name, input } => AnthropicContentBlock::ToolUse {
                    id,
                    name,
                    input,
                },
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => AnthropicContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error: is_error.unwrap_or(false),
                },
                ContentBlock::Thinking { thinking } => AnthropicContentBlock::Text {
                    text: format!("<thinking>{}</thinking>", thinking),
                },
            })
            .collect()
    }

    /// Convert tool schemas to Anthropic format
    fn convert_tools(&self, tools: Vec<ToolSchema>) -> Vec<AnthropicTool> {
        tools
            .into_iter()
            .map(|tool| AnthropicTool {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
            })
            .collect()
    }

    /// Process SSE byte stream into CompletionChunks
    fn process_stream(
        byte_stream: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static,
    ) -> impl Stream<Item = Result<CompletionChunk>> + Send + 'static {
        async_stream::stream! {
            let mut handler = AnthropicStreamHandler::new();
            let mut byte_stream = Box::pin(byte_stream);

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        // Parse SSE events from bytes
                        let text = match std::str::from_utf8(&bytes) {
                            Ok(t) => t,
                            Err(e) => {
                                yield Err(KodeError::Other(format!("Invalid UTF-8 in stream: {}", e)));
                                continue;
                            }
                        };

                        // Process the chunk
                        match handler.process_chunk(text) {
                            Ok(done) => {
                                if done {
                                    break;
                                }
                            }
                            Err(e) => {
                                yield Err(e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(KodeError::NetworkError(e.to_string()));
                        break;
                    }
                }
            }

            // Get final message and emit done event
            match handler.get_message() {
                Ok(assistant_message) => {
                    // Extract content blocks from the message
                    for block in &assistant_message.message.content {
                        match block {
                            ContentBlock::Text { text } => {
                                yield Ok(CompletionChunk::TextDelta { text: text.clone() });
                            }
                            ContentBlock::Thinking { thinking } => {
                                yield Ok(CompletionChunk::ThinkingDelta { thinking: thinking.clone() });
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                yield Ok(CompletionChunk::ToolUseComplete {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                });
                            }
                            _ => {}
                        }
                    }

                    // Emit done event with usage stats
                    yield Ok(CompletionChunk::Done {
                        stop_reason: handler.get_stop_reason().unwrap_or_else(|| "end_turn".to_string()),
                        usage: Some(handler.get_usage()),
                    });
                }
                Err(e) => {
                    yield Err(e);
                }
            }
        }
    }
}

#[async_trait]
impl ModelAdapter for AnthropicAdapter {
    fn provider(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.profile.model_name
    }

    async fn complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSchema>,
        system_prompt: Option<String>,
        options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        let request = AnthropicRequest {
            model: self.profile.model_name.clone(),
            messages: self.convert_messages(messages),
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(8192),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop_sequences,
            tools: if tools.is_empty() {
                None
            } else {
                Some(self.convert_tools(tools))
            },
            stream: Some(false),
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(KodeError::ApiError {
                provider: "anthropic".to_string(),
                message: format!("HTTP {}: {}", status, error_text),
            });
        }

        let api_response: AnthropicResponse = response.json().await?;

        // Convert API response to CompletionResponse
        let content = api_response
            .content
            .into_iter()
            .map(|block| match block {
                AnthropicContentBlock::Text { text } => ContentBlock::Text { text },
                AnthropicContentBlock::ToolUse { id, name, input } => ContentBlock::ToolUse {
                    id,
                    name,
                    input,
                },
                AnthropicContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error: Some(is_error),
                },
            })
            .collect();

        Ok(CompletionResponse {
            content,
            model: Some(api_response.model),
            stop_reason: api_response.stop_reason,
            usage: api_response.usage.map(|u| Usage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
                cache_creation_input_tokens: u.cache_creation_input_tokens,
                cache_read_input_tokens: u.cache_read_input_tokens,
            }),
        })
    }

    async fn stream_complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSchema>,
        system_prompt: Option<String>,
        options: CompletionOptions,
    ) -> Result<CompletionStream> {
        let request = AnthropicRequest {
            model: self.profile.model_name.clone(),
            messages: self.convert_messages(messages),
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(8192),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop_sequences,
            tools: if tools.is_empty() {
                None
            } else {
                Some(self.convert_tools(tools))
            },
            stream: Some(true),
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(KodeError::ApiError {
                provider: "anthropic".to_string(),
                message: format!("HTTP {}: {}", status, error_text),
            });
        }

        // Create SSE parser and stream handler
        let byte_stream = response.bytes_stream();
        let stream = Self::process_stream(byte_stream);

        Ok(Box::pin(stream))
    }

    fn max_context_tokens(&self) -> u32 {
        // Default context window for Claude models
        // TODO: Make this configurable per model
        200_000
    }

    fn max_output_tokens(&self) -> u32 {
        self.profile.max_tokens
    }
}

/// AWS Bedrock adapter (uses Anthropic models via Bedrock)
pub struct BedrockAdapter {
    profile: ModelProfile,
}

impl BedrockAdapter {
    pub fn new(profile: ModelProfile) -> Result<Self> {
        Ok(Self { profile })
    }
}

#[async_trait]
impl ModelAdapter for BedrockAdapter {
    fn provider(&self) -> &str {
        "bedrock"
    }

    fn model(&self) -> &str {
        &self.profile.model_name
    }

    async fn complete(
        &self,
        _messages: Vec<Message>,
        _tools: Vec<ToolSchema>,
        _system_prompt: Option<String>,
        _options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        Err(KodeError::NotImplemented(
            "Bedrock adapter not yet implemented".to_string(),
        ))
    }

    async fn stream_complete(
        &self,
        _messages: Vec<Message>,
        _tools: Vec<ToolSchema>,
        _system_prompt: Option<String>,
        _options: CompletionOptions,
    ) -> Result<CompletionStream> {
        Err(KodeError::NotImplemented(
            "Bedrock streaming not yet implemented".to_string(),
        ))
    }

    fn max_context_tokens(&self) -> u32 {
        200_000
    }

    fn max_output_tokens(&self) -> u32 {
        self.profile.max_tokens
    }
}

/// Google Vertex AI adapter (uses Anthropic models via Vertex)
pub struct VertexAdapter {
    profile: ModelProfile,
}

impl VertexAdapter {
    pub fn new(profile: ModelProfile) -> Result<Self> {
        Ok(Self { profile })
    }
}

#[async_trait]
impl ModelAdapter for VertexAdapter {
    fn provider(&self) -> &str {
        "vertex"
    }

    fn model(&self) -> &str {
        &self.profile.model_name
    }

    async fn complete(
        &self,
        _messages: Vec<Message>,
        _tools: Vec<ToolSchema>,
        _system_prompt: Option<String>,
        _options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        Err(KodeError::NotImplemented(
            "Vertex adapter not yet implemented".to_string(),
        ))
    }

    async fn stream_complete(
        &self,
        _messages: Vec<Message>,
        _tools: Vec<ToolSchema>,
        _system_prompt: Option<String>,
        _options: CompletionOptions,
    ) -> Result<CompletionStream> {
        Err(KodeError::NotImplemented(
            "Vertex streaming not yet implemented".to_string(),
        ))
    }

    fn max_context_tokens(&self) -> u32 {
        200_000
    }

    fn max_output_tokens(&self) -> u32 {
        self.profile.max_tokens
    }
}

// Anthropic API types

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "is_false")]
        is_error: bool,
    },
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContentBlock>,
    model: String,
    stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_creation_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_read_input_tokens: Option<u32>,
}
