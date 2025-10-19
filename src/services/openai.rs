//! OpenAI API adapter
//!
//! Supports:
//! - OpenAI official API (ChatGPT, GPT-4, etc.)
//! - OpenAI-compatible endpoints (Ollama, LM Studio, etc.)

use async_trait::async_trait;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use crate::{
    config::models::ModelProfile,
    error::{KodeError, Result},
    messages::{ContentBlock, Message, Role},
};

use super::{CompletionOptions, CompletionResponse, CompletionStream, ModelAdapter, ToolSchema, Usage};

/// OpenAI API adapter
pub struct OpenAIAdapter {
    client: Client,
    profile: ModelProfile,
    base_url: String,
}

impl OpenAIAdapter {
    /// Create a new OpenAI adapter
    pub fn new(profile: ModelProfile) -> Result<Self> {
        let api_key = if profile.api_key.is_empty() {
            std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "dummy-key".to_string())
        } else {
            profile.api_key.clone()
        };

        let base_url = profile
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        let client = Client::builder()
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    "Authorization",
                    header::HeaderValue::from_str(&format!("Bearer {}", api_key)).map_err(
                        |_| KodeError::InvalidConfig("Invalid API key format".to_string()),
                    )?,
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

    /// Convert internal messages to OpenAI format
    fn convert_messages(&self, messages: Vec<Message>) -> Vec<OpenAIMessage> {
        messages
            .into_iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                }
                .to_string();

                // Extract text content
                let text_content: Vec<String> = msg
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => Some(format!("Tool result for {}: {}", tool_use_id, content)),
                        _ => None,
                    })
                    .collect();

                // Extract tool calls
                let tool_calls: Vec<OpenAIToolCall> = msg
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolUse { id, name, input } => Some(OpenAIToolCall {
                            id: id.clone(),
                            call_type: "function".to_string(),
                            function: OpenAIFunction {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).ok()?,
                            },
                        }),
                        _ => None,
                    })
                    .collect();

                OpenAIMessage {
                    role,
                    content: if text_content.is_empty() {
                        None
                    } else {
                        Some(text_content.join("\n"))
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                    name: None,
                }
            })
            .collect()
    }

    /// Convert tool schemas to OpenAI format
    fn convert_tools(&self, tools: Vec<ToolSchema>) -> Vec<OpenAITool> {
        tools
            .into_iter()
            .map(|tool| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDef {
                    name: tool.name,
                    description: tool.description,
                    parameters: tool.input_schema,
                },
            })
            .collect()
    }
}

#[async_trait]
impl ModelAdapter for OpenAIAdapter {
    fn provider(&self) -> &str {
        "openai"
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
        let mut openai_messages = Vec::new();

        // Add system message if provided
        if let Some(system) = system_prompt {
            openai_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: Some(system),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        // Add converted messages
        openai_messages.extend(self.convert_messages(messages));

        let request = OpenAIRequest {
            model: self.profile.model_name.clone(),
            messages: openai_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop_sequences,
            tools: if tools.is_empty() {
                None
            } else {
                Some(self.convert_tools(tools))
            },
            tool_choice: None,
            stream: Some(false),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(KodeError::ApiError {
                provider: "openai".to_string(),
                message: format!("HTTP {}: {}", status, error_text),
            });
        }

        let api_response: OpenAIResponse = response.json().await?;

        // Convert OpenAI response to CompletionResponse
        let choice = api_response.choices.into_iter().next().ok_or_else(|| {
            KodeError::ApiError {
                provider: "openai".to_string(),
                message: "No choices in response".to_string(),
            }
        })?;

        let mut content = Vec::new();

        // Add text content if present
        if let Some(text) = choice.message.content {
            if !text.is_empty() {
                content.push(ContentBlock::Text { text });
            }
        }

        // Add tool calls if present
        if let Some(tool_calls) = choice.message.tool_calls {
            for tool_call in tool_calls {
                let input: serde_json::Value =
                    serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

                content.push(ContentBlock::ToolUse {
                    id: tool_call.id,
                    name: tool_call.function.name,
                    input,
                });
            }
        }

        Ok(CompletionResponse {
            content,
            model: Some(api_response.model),
            stop_reason: Some(choice.finish_reason),
            usage: api_response.usage.map(|u| Usage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
        })
    }

    async fn stream_complete(
        &self,
        _messages: Vec<Message>,
        _tools: Vec<ToolSchema>,
        _system_prompt: Option<String>,
        _options: CompletionOptions,
    ) -> Result<CompletionStream> {
        Err(KodeError::NotImplemented(
            "OpenAI streaming not yet implemented".to_string(),
        ))
    }

    fn max_context_tokens(&self) -> u32 {
        // Default context window for GPT-4 models
        // TODO: Make this configurable per model
        128_000
    }

    fn max_output_tokens(&self) -> u32 {
        self.profile.max_tokens
    }
}

// OpenAI API types

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAIChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChoice {
    index: u32,
    message: OpenAIMessage,
    finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
