//! Model configuration and profiles

use serde::{Deserialize, Serialize};

/// AI provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Anthropic,
    OpenAI,
    Mistral,
    Deepseek,
    Kimi,
    Qwen,
    Glm,
    Minimax,
    #[serde(rename = "baidu-qianfan")]
    BaiduQianfan,
    Siliconflow,
    Bigdream,
    Opendev,
    Xai,
    Groq,
    Gemini,
    Ollama,
    Azure,
    Custom,
    #[serde(rename = "custom-openai")]
    CustomOpenAI,
}

impl ProviderType {
    /// Get the default base URL for this provider
    #[must_use]
    pub const fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some("https://api.anthropic.com"),
            Self::OpenAI | Self::CustomOpenAI => Some("https://api.openai.com/v1"),
            Self::Azure => None, // Azure requires custom endpoint
            Self::Custom => None, // Custom requires user-specified endpoint
            Self::Groq => Some("https://api.groq.com/openai/v1"),
            Self::Ollama => Some("http://localhost:11434"),
            _ => None,
        }
    }

    /// Check if this provider requires an API key
    #[must_use]
    pub const fn requires_api_key(&self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

/// Reasoning effort level (for models that support it, like o1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
}

/// Model profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    /// User-friendly name for the profile
    pub name: String,

    /// Provider type
    pub provider: ProviderType,

    /// Actual model identifier (primary key)
    pub model_name: String,

    /// Custom API endpoint (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// API key for authentication
    pub api_key: String,

    /// Maximum output tokens
    pub max_tokens: u32,

    /// Context window size
    pub context_length: u32,

    /// Reasoning effort level (for reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// Whether this profile is active
    #[serde(default = "default_true")]
    pub is_active: bool,

    /// Creation timestamp
    pub created_at: u64,

    /// Last usage timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<u64>,

    /// Whether this is a GPT-5 model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_gpt5: Option<bool>,

    /// Configuration validation status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_status: Option<ValidationStatus>,

    /// Last validation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_validation: Option<u64>,
}

fn default_true() -> bool {
    true
}

/// Configuration validation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Valid,
    NeedsRepair,
    AutoRepaired,
}

impl ModelProfile {
    /// Create a new model profile
    #[must_use]
    pub fn new(
        name: String,
        provider: ProviderType,
        model_name: String,
        api_key: String,
        max_tokens: u32,
        context_length: u32,
    ) -> Self {
        Self {
            name,
            provider,
            model_name,
            base_url: provider.default_base_url().map(String::from),
            api_key,
            max_tokens,
            context_length,
            reasoning_effort: None,
            is_active: true,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_used: None,
            is_gpt5: None,
            validation_status: None,
            last_validation: None,
        }
    }

    /// Get the effective base URL (custom or default)
    #[must_use]
    pub fn effective_base_url(&self) -> Option<String> {
        self.base_url
            .clone()
            .or_else(|| self.provider.default_base_url().map(String::from))
    }

    /// Check if this is a GPT-5 model
    #[must_use]
    pub fn is_gpt5_model(&self) -> bool {
        self.is_gpt5.unwrap_or_else(|| {
            let model_lower = self.model_name.to_lowercase();
            model_lower.starts_with("gpt-5") || model_lower.contains("gpt-5")
        })
    }

    /// Update last used timestamp
    pub fn mark_used(&mut self) {
        self.last_used = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }
}

/// Model pointer types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelPointerType {
    Main,
    Task,
    Reasoning,
    Quick,
}

impl std::fmt::Display for ModelPointerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Task => write!(f, "task"),
            Self::Reasoning => write!(f, "reasoning"),
            Self::Quick => write!(f, "quick"),
        }
    }
}

impl std::str::FromStr for ModelPointerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "main" => Ok(Self::Main),
            "task" => Ok(Self::Task),
            "reasoning" => Ok(Self::Reasoning),
            "quick" => Ok(Self::Quick),
            _ => Err(format!("Invalid model pointer type: {s}")),
        }
    }
}

/// Model pointer configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPointer {
    /// Main dialog model
    #[serde(default)]
    pub main: String,

    /// Task tool model
    #[serde(default)]
    pub task: String,

    /// Reasoning model
    #[serde(default)]
    pub reasoning: String,

    /// Quick model
    #[serde(default)]
    pub quick: String,
}

/// Model configuration helper
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub profiles: Vec<ModelProfile>,
    pub pointers: ModelPointer,
    pub default_model_name: Option<String>,
}

impl ModelConfig {
    /// Get a model by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ModelProfile> {
        self.profiles.iter().find(|p| p.model_name == name)
    }

    /// Get active models only
    #[must_use]
    pub fn active_models(&self) -> Vec<&ModelProfile> {
        self.profiles.iter().filter(|p| p.is_active).collect()
    }

    /// Get model by pointer
    #[must_use]
    pub fn get_by_pointer(&self, pointer: ModelPointerType) -> Option<&ModelProfile> {
        let model_name = match pointer {
            ModelPointerType::Main => &self.pointers.main,
            ModelPointerType::Task => &self.pointers.task,
            ModelPointerType::Reasoning => &self.pointers.reasoning,
            ModelPointerType::Quick => &self.pointers.quick,
        };

        if model_name.is_empty() {
            self.default_model_name
                .as_ref()
                .and_then(|name| self.get(name))
        } else {
            self.get(model_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_default_urls() {
        assert_eq!(
            ProviderType::Anthropic.default_base_url(),
            Some("https://api.anthropic.com")
        );
        assert_eq!(
            ProviderType::OpenAI.default_base_url(),
            Some("https://api.openai.com/v1")
        );
        assert!(ProviderType::Custom.default_base_url().is_none());
    }

    #[test]
    fn test_gpt5_detection() {
        let profile = ModelProfile::new(
            "GPT-5".into(),
            ProviderType::OpenAI,
            "gpt-5-preview".into(),
            "test-key".into(),
            8192,
            128000,
        );
        assert!(profile.is_gpt5_model());
    }

    #[test]
    fn test_model_pointer_from_str() {
        assert_eq!("main".parse::<ModelPointerType>().unwrap(), ModelPointerType::Main);
        assert_eq!("task".parse::<ModelPointerType>().unwrap(), ModelPointerType::Task);
        assert!("invalid".parse::<ModelPointerType>().is_err());
    }
}
