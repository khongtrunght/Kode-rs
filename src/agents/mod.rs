//! Agent configuration loader
//!
//! Loads agent configurations from markdown files with YAML frontmatter.
//! Maintains compatibility with Claude Code `.claude` agent directories while
//! prioritizing Kode-specific overrides.
//!
//! ## Priority System
//!
//! Agent configurations are loaded from multiple directories with the following priority
//! (later entries override earlier ones):
//!
//! 1. Built-in agents (code-embedded)
//! 2. `~/.claude/agents/` (Claude Code user directory)
//! 3. `~/.kode/agents/` (Kode user directory)
//! 4. `./.claude/agents/` (Claude Code project directory)
//! 5. `./.kode/agents/` (Kode project directory)

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{KodeError, Result};

/// Agent configuration defining behavior, permissions, and system prompt
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfig {
    /// Agent identifier (matches subagent_type)
    pub agent_type: String,

    /// Description of when to use this agent
    pub when_to_use: String,

    /// Tool permissions: specific tool names or "*" for all tools
    #[serde(default = "default_all_tools")]
    pub tools: ToolPermissions,

    /// System prompt content
    pub system_prompt: String,

    /// Agent source location
    #[serde(skip, default = "default_location")]
    pub location: AgentLocation,

    /// Optional UI color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Optional model override (uses "model_name" field from frontmatter)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

fn default_all_tools() -> ToolPermissions {
    ToolPermissions::All
}

fn default_location() -> AgentLocation {
    AgentLocation::BuiltIn
}

/// Tool permissions for an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ToolPermissions {
    /// All tools allowed
    All,
    /// Specific tools allowed
    Specific(Vec<String>),
}

impl ToolPermissions {
    /// Check if a tool is allowed
    pub fn allows(&self, tool_name: &str) -> bool {
        match self {
            ToolPermissions::All => true,
            ToolPermissions::Specific(tools) => tools.iter().any(|t| t == tool_name),
        }
    }
}

/// Agent source location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentLocation {
    BuiltIn,
    UserClaude,
    UserKode,
    ProjectClaude,
    ProjectKode,
}

impl AgentLocation {
    /// Get priority value (higher = more priority)
    const fn priority(self) -> u8 {
        match self {
            Self::BuiltIn => 0,
            Self::UserClaude => 1,
            Self::UserKode => 2,
            Self::ProjectClaude => 3,
            Self::ProjectKode => 4,
        }
    }
}

/// YAML frontmatter for agent configuration files
#[derive(Debug, Deserialize)]
struct AgentFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    tools: Option<serde_yaml::Value>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    model_name: Option<String>,

    /// Deprecated field (ignored with warning)
    #[serde(default)]
    model: Option<String>,
}

/// Built-in general-purpose agent
fn builtin_general_purpose() -> AgentConfig {
    AgentConfig {
        agent_type: "general-purpose".to_string(),
        when_to_use: "General-purpose agent for researching complex questions, searching for code, and executing multi-step tasks".to_string(),
        tools: ToolPermissions::All,
        system_prompt: r#"You are a general-purpose agent. Given the user's task, use the tools available to complete it efficiently and thoroughly.

When to use your capabilities:
- Searching for code, configurations, and patterns across large codebases
- Analyzing multiple files to understand system architecture
- Investigating complex questions that require exploring many files
- Performing multi-step research tasks

Guidelines:
- For file searches: Use Grep or Glob when you need to search broadly. Use FileRead when you know the specific file path.
- For analysis: Start broad and narrow down. Use multiple search strategies if the first doesn't yield results.
- Be thorough: Check multiple locations, consider different naming conventions, look for related files.
- Complete tasks directly using your capabilities."#.to_string(),
        location: AgentLocation::BuiltIn,
        color: None,
        model_name: None,
    }
}

/// Parse tools field from YAML frontmatter
fn parse_tools(value: Option<serde_yaml::Value>) -> ToolPermissions {
    match value {
        None => ToolPermissions::All,
        Some(serde_yaml::Value::String(s)) if s == "*" => ToolPermissions::All,
        Some(serde_yaml::Value::Sequence(seq)) => {
            let tools: Vec<String> = seq
                .into_iter()
                .filter_map(|v| {
                    if let serde_yaml::Value::String(s) = v {
                        Some(s)
                    } else {
                        None
                    }
                })
                .collect();

            if tools.is_empty() {
                ToolPermissions::All
            } else {
                ToolPermissions::Specific(tools)
            }
        }
        Some(serde_yaml::Value::String(s)) => ToolPermissions::Specific(vec![s]),
        _ => ToolPermissions::All,
    }
}

/// Parse a markdown file with YAML frontmatter
fn parse_agent_file(path: &Path, location: AgentLocation) -> Result<AgentConfig> {
    let content = fs::read_to_string(path)
        .map_err(|e| KodeError::AgentLoadError(format!("Failed to read {}: {}", path.display(), e)))?;

    // Simple YAML frontmatter parser (looking for --- delimiters)
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || lines[0] != "---" {
        return Err(KodeError::AgentLoadError(
            format!("Missing YAML frontmatter in {}", path.display())
        ));
    }

    // Find closing ---
    let end_idx = lines[1..]
        .iter()
        .position(|line| line.trim() == "---")
        .ok_or_else(|| {
            KodeError::AgentLoadError(format!("Unclosed YAML frontmatter in {}", path.display()))
        })?
        + 1;

    // Parse frontmatter
    let frontmatter_lines = &lines[1..end_idx];
    let frontmatter_str = frontmatter_lines.join("\n");
    let frontmatter: AgentFrontmatter = serde_yaml::from_str(&frontmatter_str)
        .map_err(|e| KodeError::AgentLoadError(format!("Invalid YAML frontmatter: {}", e)))?;

    // Check for deprecated 'model' field
    if frontmatter.model.is_some() && frontmatter.model_name.is_none() {
        if std::env::var("KODE_DEBUG_AGENTS").is_ok() {
            eprintln!(
                "⚠️  Agent {}: 'model' field is deprecated and ignored. Use 'model_name' instead.",
                frontmatter.name
            );
        }
    }

    // Extract body (everything after closing ---)
    let body = lines[end_idx + 1..].join("\n").trim().to_string();

    Ok(AgentConfig {
        agent_type: frontmatter.name,
        when_to_use: frontmatter.description.replace("\\n", "\n"),
        tools: parse_tools(frontmatter.tools),
        system_prompt: body,
        location,
        color: frontmatter.color,
        model_name: frontmatter.model_name,
    })
}

/// Scan a directory for agent configuration files
async fn scan_agent_directory(dir: &Path, location: AgentLocation) -> Vec<AgentConfig> {
    if !dir.exists() {
        return Vec::new();
    }

    let mut agents = Vec::new();

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Warning: Failed to scan directory {}: {}", dir.display(), e);
            return agents;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process .md files
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        match parse_agent_file(&path, location) {
            Ok(agent) => agents.push(agent),
            Err(e) => {
                eprintln!("Warning: Failed to parse agent file {}: {}", path.display(), e);
            }
        }
    }

    agents
}

/// Agent registry with caching
pub struct AgentRegistry {
    /// Cache of active agents (deduplicated by priority)
    agents: Arc<RwLock<HashMap<String, AgentConfig>>>,

    /// File watcher for hot reload
    #[allow(dead_code)]
    watcher: Option<RecommendedWatcher>,
}

impl AgentRegistry {
    /// Create a new agent registry
    pub async fn new(enable_watch: bool) -> Result<Self> {
        let agents = Arc::new(RwLock::new(HashMap::new()));

        let mut registry = Self {
            agents: Arc::clone(&agents),
            watcher: None,
        };

        // Load initial agents
        registry.reload().await?;

        // Set up file watcher if enabled
        if enable_watch {
            let agents_clone = Arc::clone(&agents);
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    // Only reload on modify/create/remove events for .md files
                    if event.paths.iter().any(|p| {
                        p.extension().and_then(|s| s.to_str()) == Some("md")
                    }) {
                        let agents = Arc::clone(&agents_clone);
                        tokio::spawn(async move {
                            if let Err(e) = Self::reload_agents(&agents).await {
                                eprintln!("Failed to reload agents: {}", e);
                            }
                        });
                    }
                }
            })
            .map_err(|e| KodeError::Other(format!("Failed to create file watcher: {}", e)))?;

            // Watch all agent directories
            for dir in agent_directories() {
                if dir.exists() {
                    let _ = watcher.watch(&dir, RecursiveMode::NonRecursive);
                }
            }

            registry.watcher = Some(watcher);
        }

        Ok(registry)
    }

    /// Reload all agents from disk
    pub async fn reload(&mut self) -> Result<()> {
        Self::reload_agents(&self.agents).await
    }

    /// Internal reload implementation
    async fn reload_agents(agents: &Arc<RwLock<HashMap<String, AgentConfig>>>) -> Result<()> {
        // Scan all directories in parallel
        let dirs = agent_directories();
        let mut tasks = Vec::new();

        for (dir, location) in dirs.into_iter().zip([
            AgentLocation::UserClaude,
            AgentLocation::UserKode,
            AgentLocation::ProjectClaude,
            AgentLocation::ProjectKode,
        ]) {
            tasks.push(async move { scan_agent_directory(&dir, location).await });
        }

        let results = futures::future::join_all(tasks).await;

        // Build agent map with priority
        let mut agent_map = HashMap::new();

        // Start with built-in
        let builtin = builtin_general_purpose();
        agent_map.insert(builtin.agent_type.clone(), builtin);

        // Add scanned agents in priority order
        for scanned_agents in results {
            for agent in scanned_agents {
                // Check priority: only replace if new agent has higher priority
                agent_map
                    .entry(agent.agent_type.clone())
                    .and_modify(|existing: &mut AgentConfig| {
                        if agent.location.priority() > existing.location.priority() {
                            *existing = agent.clone();
                        }
                    })
                    .or_insert(agent);
            }
        }

        // Update cache
        let mut cache = agents.write().await;
        *cache = agent_map;

        Ok(())
    }

    /// Get all active agents
    pub async fn get_active_agents(&self) -> Vec<AgentConfig> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Get a specific agent by type
    pub async fn get_agent(&self, agent_type: &str) -> Option<AgentConfig> {
        let agents = self.agents.read().await;
        agents.get(agent_type).cloned()
    }

    /// Get all available agent types
    pub async fn get_agent_types(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }
}

/// Get agent directory paths
fn agent_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User directories
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".claude").join("agents"));
        dirs.push(home.join(".kode").join("agents"));
    }

    // Project directories (using current working directory)
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    dirs.push(cwd.join(".claude").join("agents"));
    dirs.push(cwd.join(".kode").join("agents"));

    dirs
}

/// Global agent registry (lazy-initialized)
static GLOBAL_REGISTRY: Lazy<Arc<RwLock<Option<AgentRegistry>>>> =
    Lazy::new(|| Arc::new(RwLock::new(None)));

/// Initialize the global agent registry
pub async fn init_agent_registry(enable_watch: bool) -> Result<()> {
    let mut global = GLOBAL_REGISTRY.write().await;
    if global.is_none() {
        *global = Some(AgentRegistry::new(enable_watch).await?);
    }
    Ok(())
}

/// Get the global agent registry
async fn get_registry() -> Result<Arc<RwLock<Option<AgentRegistry>>>> {
    let global = GLOBAL_REGISTRY.read().await;
    if global.is_none() {
        drop(global);
        init_agent_registry(false).await?;
    }
    Ok(Arc::clone(&GLOBAL_REGISTRY))
}

/// Get all active agents
pub async fn get_active_agents() -> Result<Vec<AgentConfig>> {
    let registry_lock = get_registry().await?;
    let registry = registry_lock.read().await;

    if let Some(registry) = registry.as_ref() {
        Ok(registry.get_active_agents().await)
    } else {
        Ok(vec![builtin_general_purpose()])
    }
}

/// Get a specific agent by type
pub async fn get_agent_by_type(agent_type: &str) -> Result<Option<AgentConfig>> {
    let registry_lock = get_registry().await?;
    let registry = registry_lock.read().await;

    if let Some(registry) = registry.as_ref() {
        Ok(registry.get_agent(agent_type).await)
    } else {
        Ok(if agent_type == "general-purpose" {
            Some(builtin_general_purpose())
        } else {
            None
        })
    }
}

/// Get all available agent types
pub async fn get_available_agent_types() -> Result<Vec<String>> {
    let registry_lock = get_registry().await?;
    let registry = registry_lock.read().await;

    if let Some(registry) = registry.as_ref() {
        Ok(registry.get_agent_types().await)
    } else {
        Ok(vec!["general-purpose".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_tools_all() {
        let tools = parse_tools(Some(serde_yaml::Value::String("*".to_string())));
        assert_eq!(tools, ToolPermissions::All);
        assert!(tools.allows("any-tool"));
    }

    #[test]
    fn test_parse_tools_specific() {
        let tools = parse_tools(Some(serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("FileRead".to_string()),
            serde_yaml::Value::String("FileWrite".to_string()),
        ])));

        if let ToolPermissions::Specific(tool_list) = tools {
            assert_eq!(tool_list.len(), 2);
            assert!(tool_list.contains(&"FileRead".to_string()));
            assert!(tool_list.contains(&"FileWrite".to_string()));
        } else {
            panic!("Expected Specific tools");
        }
    }

    #[test]
    fn test_builtin_agent() {
        let agent = builtin_general_purpose();
        assert_eq!(agent.agent_type, "general-purpose");
        assert_eq!(agent.location, AgentLocation::BuiltIn);
        assert_eq!(agent.tools, ToolPermissions::All);
    }

    #[tokio::test]
    async fn test_parse_agent_file() {
        let temp_dir = TempDir::new().unwrap();
        let agent_file = temp_dir.path().join("test-agent.md");

        let content = r#"---
name: test-agent
description: "A test agent"
tools:
  - FileRead
  - Bash
---

This is the system prompt for the test agent.
It can be multiple lines."#;

        fs::write(&agent_file, content).unwrap();

        let agent = parse_agent_file(&agent_file, AgentLocation::UserKode).unwrap();

        assert_eq!(agent.agent_type, "test-agent");
        assert_eq!(agent.when_to_use, "A test agent");
        assert!(agent.system_prompt.contains("This is the system prompt"));

        if let ToolPermissions::Specific(tools) = agent.tools {
            assert_eq!(tools.len(), 2);
            assert!(tools.contains(&"FileRead".to_string()));
        } else {
            panic!("Expected Specific tools");
        }
    }

    #[tokio::test]
    async fn test_agent_registry() {
        let registry = AgentRegistry::new(false).await.unwrap();

        // Should have at least the built-in agent
        let agents = registry.get_active_agents().await;
        assert!(!agents.is_empty());

        // Should be able to find general-purpose agent
        let gp = registry.get_agent("general-purpose").await;
        assert!(gp.is_some());
        assert_eq!(gp.unwrap().agent_type, "general-purpose");
    }

    #[tokio::test]
    async fn test_agent_priority() {
        let temp_dir = TempDir::new().unwrap();

        // Create two agent files with same name but different priority
        let user_dir = temp_dir.path().join(".kode").join("agents");
        let project_dir = temp_dir.path().join(".claude").join("agents");

        fs::create_dir_all(&user_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();

        // Lower priority agent
        let user_agent = user_dir.join("test.md");
        fs::write(&user_agent, r#"---
name: priority-test
description: "User agent"
---
User prompt"#).unwrap();

        // Higher priority agent (but in claude, not kode)
        let project_agent = project_dir.join("test.md");
        fs::write(&project_agent, r#"---
name: priority-test
description: "Project agent"
---
Project prompt"#).unwrap();

        // Scan both directories
        let user_agents = scan_agent_directory(&user_dir, AgentLocation::UserKode).await;
        let project_agents = scan_agent_directory(&project_dir, AgentLocation::ProjectClaude).await;

        assert_eq!(user_agents.len(), 1);
        assert_eq!(project_agents.len(), 1);

        // Project agent should have higher priority
        assert!(project_agents[0].location.priority() > user_agents[0].location.priority());
    }
}
