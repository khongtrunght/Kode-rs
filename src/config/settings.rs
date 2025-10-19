//! Global and project-specific settings

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{ModelPointer, ModelProfile};
use crate::error::{KodeError, Result};

/// Global configuration (stored in `~/.kode.json`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Number of times the app has been started
    #[serde(default)]
    pub num_startups: u64,

    /// User ID for analytics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Verbose logging enabled
    #[serde(default)]
    pub verbose: bool,

    /// Primary provider
    #[serde(default = "default_provider")]
    pub primary_provider: String,

    /// Model profiles
    #[serde(default)]
    pub model_profiles: Vec<ModelProfile>,

    /// Model pointers
    #[serde(default)]
    pub model_pointers: ModelPointer,

    /// Default model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model_name: Option<String>,

    /// Enable streaming responses
    #[serde(default = "default_true")]
    pub stream: bool,

    /// HTTP proxy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,

    /// Projects configuration
    #[serde(default)]
    pub projects: HashMap<String, ProjectConfig>,
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            num_startups: 0,
            user_id: None,
            verbose: false,
            primary_provider: default_provider(),
            model_profiles: Vec::new(),
            model_pointers: ModelPointer::default(),
            default_model_name: None,
            stream: true,
            proxy: None,
            projects: HashMap::new(),
        }
    }
}

impl GlobalConfig {
    /// Load global configuration from disk
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    pub fn load() -> Result<Self> {
        let path = super::Config::global_config_path();
        Self::load_from_path(&path)
    }

    /// Load configuration from a specific path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path).map_err(|e| KodeError::ConfigParse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let mut config: Self = serde_json::from_str(&contents).map_err(|e| {
            KodeError::ConfigParse {
                path: path.to_path_buf(),
                message: e.to_string(),
            }
        })?;

        // Merge with defaults for missing fields
        if config.model_profiles.is_empty() && config.model_pointers.main.is_empty() {
            let default = Self::default();
            config.model_pointers = default.model_pointers;
        }

        Ok(config)
    }

    /// Save configuration to disk
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written
    pub fn save(&self) -> Result<()> {
        let path = super::Config::global_config_path();
        self.save_to_path(&path)
    }

    /// Save configuration to a specific path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Get API key for a provider from environment or config
    #[must_use]
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        // Try environment variables first
        match provider.to_lowercase().as_str() {
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            "openai" | "custom-openai" => std::env::var("OPENAI_API_KEY").ok(),
            _ => None,
        }
    }
}

/// Project-specific configuration (stored in `./.kode.json`)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Allowed tools for this project
    #[serde(default)]
    pub allowed_tools: Vec<String>,

    /// Project context (key-value pairs)
    #[serde(default)]
    pub context: HashMap<String, String>,

    /// Context files to always include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_files: Option<Vec<String>>,

    /// Command history
    #[serde(default)]
    pub history: Vec<String>,

    /// Don't crawl this directory for context
    #[serde(default)]
    pub dont_crawl_directory: bool,

    /// Enable architect tool
    #[serde(default)]
    pub enable_architect_tool: bool,

    /// MCP context URIs
    #[serde(default)]
    pub mcp_context_uris: Vec<String>,

    /// MCP servers configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,

    /// Trust dialog accepted
    #[serde(default)]
    pub has_trust_dialog_accepted: bool,

    /// Project onboarding completed
    #[serde(default)]
    pub has_completed_project_onboarding: bool,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
    },
    Sse {
        url: String,
    },
}

impl ProjectConfig {
    /// Load project configuration from current directory
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    pub fn load() -> Result<Self> {
        let path = super::Config::project_config_path();
        Self::load_from_path(&path)
    }

    /// Load configuration from a specific path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path).map_err(|e| KodeError::ConfigParse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        serde_json::from_str(&contents).map_err(|e| KodeError::ConfigParse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Save configuration to disk
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written
    pub fn save(&self) -> Result<()> {
        let path = super::Config::project_config_path();
        self.save_to_path(&path)
    }

    /// Save configuration to a specific path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_global_config_default() {
        let config = GlobalConfig::default();
        assert_eq!(config.num_startups, 0);
        assert!(!config.verbose);
        assert!(config.stream);
    }

    #[test]
    fn test_project_config_default() {
        let config = ProjectConfig::default();
        assert!(config.allowed_tools.is_empty());
        assert!(!config.dont_crawl_directory);
    }

    #[test]
    fn test_save_and_load_global_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let mut config = GlobalConfig::default();
        config.verbose = true;
        config.num_startups = 42;

        config.save_to_path(&config_path).unwrap();

        let loaded = GlobalConfig::load_from_path(&config_path).unwrap();
        assert_eq!(loaded.verbose, true);
        assert_eq!(loaded.num_startups, 42);
    }

    #[test]
    fn test_save_and_load_project_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".kode.json");

        let mut config = ProjectConfig::default();
        config.dont_crawl_directory = true;
        config.allowed_tools = vec!["FileRead".to_string(), "Bash".to_string()];

        config.save_to_path(&config_path).unwrap();

        let loaded = ProjectConfig::load_from_path(&config_path).unwrap();
        assert!(loaded.dont_crawl_directory);
        assert_eq!(loaded.allowed_tools.len(), 2);
    }
}
