//! Configuration management for Kode-rs
//!
//! Implements a hierarchical configuration system:
//! 1. Global config (`~/.kode.json`)
//! 2. Project config (`./.kode.json`)
//! 3. Environment variables
//! 4. CLI parameters (highest priority)

pub mod models;
pub mod settings;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use self::{
    models::{ModelConfig, ModelPointer, ModelPointerType, ModelProfile, ProviderType},
    settings::{GlobalConfig, ProjectConfig},
};
use crate::error::Result;

/// Main configuration structure combining global and project settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global configuration
    #[serde(flatten)]
    pub global: GlobalConfig,

    /// Project-specific configuration
    #[serde(skip)]
    pub project: ProjectConfig,
}

impl Config {
    /// Load configuration from files and environment
    ///
    /// # Errors
    ///
    /// Returns an error if configuration files cannot be read or parsed
    pub fn load() -> Result<Self> {
        let global = GlobalConfig::load()?;
        let project = ProjectConfig::load()?;

        Ok(Self { global, project })
    }

    /// Get the configuration directory path
    #[must_use]
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kode")
    }

    /// Get the global config file path
    #[must_use]
    pub fn global_config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    /// Get the project config file path in the current directory
    #[must_use]
    pub fn project_config_path() -> PathBuf {
        PathBuf::from(".kode.json")
    }

    /// Save configuration to disk
    ///
    /// # Errors
    ///
    /// Returns an error if configuration files cannot be written
    pub fn save(&self) -> Result<()> {
        self.global.save()?;
        self.project.save()?;
        Ok(())
    }

    /// Get a model profile by name
    #[must_use]
    pub fn get_model(&self, name: &str) -> Option<&ModelProfile> {
        self.global
            .model_profiles
            .iter()
            .find(|profile| profile.model_name == name)
    }

    /// Get the default model profile
    #[must_use]
    pub fn default_model(&self) -> Option<&ModelProfile> {
        self.global
            .default_model_name
            .as_ref()
            .and_then(|name| self.get_model(name))
    }

    /// Get model by pointer type (main, task, reasoning, quick)
    #[must_use]
    pub fn get_model_by_pointer(&self, pointer: ModelPointerType) -> Option<&ModelProfile> {
        let model_name = match pointer {
            ModelPointerType::Main => &self.global.model_pointers.main,
            ModelPointerType::Task => &self.global.model_pointers.task,
            ModelPointerType::Reasoning => &self.global.model_pointers.reasoning,
            ModelPointerType::Quick => &self.global.model_pointers.quick,
        };

        if model_name.is_empty() {
            self.default_model()
        } else {
            self.get_model(model_name)
        }
    }
}

/// Configuration validation errors
#[derive(Debug)]
pub enum ValidationError {
    MissingApiKey(String),
    InvalidModel(String),
    InvalidPointer(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingApiKey(provider) => {
                write!(f, "Missing API key for provider: {provider}")
            }
            Self::InvalidModel(name) => write!(f, "Invalid model configuration: {name}"),
            Self::InvalidPointer(pointer) => write!(f, "Invalid model pointer: {pointer}"),
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths() {
        let global_path = Config::global_config_path();
        assert!(global_path.ends_with("kode/config.json"));

        let project_path = Config::project_config_path();
        assert_eq!(project_path, PathBuf::from(".kode.json"));
    }
}
