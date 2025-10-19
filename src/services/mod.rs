//! Service layer for external AI providers and integrations
//!
//! This module provides adapters for various AI model providers including:
//! - Anthropic (Claude)
//! - OpenAI (ChatGPT)
//! - AWS Bedrock
//! - Google Vertex AI
//! - Custom OpenAI-compatible endpoints

pub mod adapters;
pub mod anthropic;
pub mod openai;
pub mod streaming;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::{
    config::models::ModelProfile,
    error::Result,
    messages::{ContentBlock, Message},
};

/// Completion options for model requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature for sampling (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p for nucleus sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Whether to stream the response
    #[serde(default = "default_stream")]
    pub stream: bool,

    /// Reasoning effort level (for reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,

    /// Verbosity level (for some models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
}

fn default_stream() -> bool {
    true
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            max_tokens: Some(8192),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            stream: true,
            reasoning_effort: None,
            verbosity: None,
        }
    }
}

/// A chunk of streaming completion data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompletionChunk {
    /// Text delta
    TextDelta { text: String },

    /// Thinking/reasoning content (for reasoning models)
    ThinkingDelta { thinking: String },

    /// Tool use started
    ToolUseStart {
        id: String,
        name: String,
    },

    /// Tool input delta (JSON string fragment)
    ToolInputDelta {
        id: String,
        partial_json: String,
    },

    /// Tool use completed
    ToolUseComplete {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Stream completed
    Done {
        stop_reason: String,
        usage: Option<Usage>,
    },

    /// Error occurred
    Error {
        message: String,
    },
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

/// Completion stream type
pub type CompletionStream = Pin<Box<dyn Stream<Item = Result<CompletionChunk>> + Send>>;

/// Response from a completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Tool schema for API requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Core trait for model adapters
///
/// This trait abstracts over different AI provider APIs (Anthropic, OpenAI, etc.)
/// and provides a unified interface for making completion requests.
#[async_trait]
pub trait ModelAdapter: Send + Sync {
    /// Get the provider name (e.g., "anthropic", "openai")
    fn provider(&self) -> &str;

    /// Get the model name
    fn model(&self) -> &str;

    /// Create a non-streaming completion
    async fn complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSchema>,
        system_prompt: Option<String>,
        options: CompletionOptions,
    ) -> Result<CompletionResponse>;

    /// Create a streaming completion
    async fn stream_complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSchema>,
        system_prompt: Option<String>,
        options: CompletionOptions,
    ) -> Result<CompletionStream>;

    /// Count tokens in a message (estimate if not supported by provider)
    fn count_tokens(&self, text: &str) -> u32 {
        // Simple approximation: ~4 chars per token
        (text.len() / 4) as u32
    }

    /// Get maximum context window size for this model
    fn max_context_tokens(&self) -> u32;

    /// Get maximum output tokens for this model
    fn max_output_tokens(&self) -> u32;
}

/// Factory for creating model adapters
pub struct ModelAdapterFactory;

impl ModelAdapterFactory {
    /// Create an adapter from a model profile
    pub fn create(profile: &ModelProfile) -> Result<Box<dyn ModelAdapter>> {
        use crate::config::models::ProviderType;

        match profile.provider {
            ProviderType::Anthropic => Ok(Box::new(anthropic::AnthropicAdapter::new(profile.clone())?)),
            ProviderType::OpenAI | ProviderType::CustomOpenAI => Ok(Box::new(openai::OpenAIAdapter::new(profile.clone())?)),
            ProviderType::Azure => Ok(Box::new(openai::OpenAIAdapter::new(profile.clone())?)), // Azure uses OpenAI API
            ProviderType::Custom => Ok(Box::new(openai::OpenAIAdapter::new(profile.clone())?)), // Assume OpenAI-compatible
            ProviderType::Ollama => Ok(Box::new(openai::OpenAIAdapter::new(profile.clone())?)), // Ollama uses OpenAI API
            ProviderType::Groq => Ok(Box::new(openai::OpenAIAdapter::new(profile.clone())?)), // Groq uses OpenAI API
            _ => Err(crate::error::KodeError::UnsupportedProvider {
                provider: format!("{:?}", profile.provider),
            }),
        }
    }
}
