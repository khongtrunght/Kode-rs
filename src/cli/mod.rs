//! CLI argument parsing and command routing

use clap::{Parser, Subcommand};

/// Kode: AI-powered terminal assistant
#[derive(Debug, Parser)]
#[command(name = "kode")]
#[command(about = "AI-powered terminal assistant", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available commands
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start interactive REPL
    Repl,

    /// Run a single query
    Query {
        /// The query to execute
        query: String,
    },

    /// Manage configuration
    Config {
        /// Get a config value
        #[arg(long)]
        get: Option<String>,

        /// Set a config value
        #[arg(long, requires = "value")]
        set: Option<String>,

        /// Value to set (used with --set)
        #[arg(long)]
        value: Option<String>,

        /// List all config values
        #[arg(long)]
        list: bool,

        /// Use global config instead of project config
        #[arg(long)]
        global: bool,
    },

    /// Manage model profiles
    Models {
        /// List all models
        #[arg(long)]
        list: bool,

        /// Add a new model
        #[arg(long)]
        add: bool,

        /// Remove a model
        #[arg(long)]
        remove: Option<String>,
    },

    /// Manage agents
    Agents {
        /// List all agents
        #[arg(long)]
        list: bool,
    },

    /// Show version information
    Version,
}

impl Cli {
    /// Parse CLI arguments from environment
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_version() {
        // Just ensure the CLI can be constructed
        let _ = Cli::parse_args();
    }
}
