//! Error types for Kode-rs

use std::path::PathBuf;

use thiserror::Error;

/// Result type alias using [`KodeError`]
pub type Result<T> = std::result::Result<T, KodeError>;

/// Main error type for Kode-rs
#[derive(Debug, Error)]
pub enum KodeError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration parse error
    #[error("Failed to parse config at {path}: {message}")]
    ConfigParse { path: PathBuf, message: String },

    /// Configuration validation error
    #[error("Invalid configuration: {0}")]
    ConfigValidation(String),

    /// Tool execution error
    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    /// Tool validation error
    #[error("Tool validation error: {0}")]
    ToolValidation(String),

    /// API error (Anthropic, OpenAI, etc.)
    #[error("API error: {0}")]
    Api(String),

    /// HTTP request error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Operation cancelled by user
    #[error("Operation cancelled by user")]
    Cancelled,

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// MCP (Model Context Protocol) error
    #[error("MCP error: {0}")]
    Mcp(String),

    /// Generic error with context
    #[error("{0}")]
    Other(String),
}

impl From<String> for KodeError {
    fn from(s: String) -> Self {
        KodeError::Other(s)
    }
}

impl From<&str> for KodeError {
    fn from(s: &str) -> Self {
        KodeError::Other(s.to_string())
    }
}
