//! Kode-rs binary entry point

use color_eyre::Result;
use kode_rs::{
    agents::AgentRegistry,
    cli::{Cli, Commands},
    config::{Config, ModelPointerType, ProviderType},
    services::{anthropic::AnthropicAdapter, openai::OpenAIAdapter},
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Install error handler
    color_eyre::install()?;

    // Parse CLI arguments
    let cli = Cli::parse_args();

    // Set up logging
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("kode_rs=debug")
            .init();
    }

    // Handle commands
    match cli.command {
        Some(Commands::Repl) | None => {
            // Start REPL (default command)
            start_repl(None).await?;
        }
        Some(Commands::Query { query }) => {
            // Start REPL with initial query
            start_repl(Some(query)).await?;
        }
        Some(Commands::Config {
            get,
            set,
            value,
            list,
            global,
        }) => {
            handle_config_command(get, set, value, list, global)?;
        }
        Some(Commands::Models { list, add, remove }) => {
            handle_models_command(list, add, remove)?;
        }
        Some(Commands::Agents { list }) => {
            handle_agents_command(list).await?;
        }
        Some(Commands::Version) => {
            println!("kode-rs version {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

/// Start the interactive REPL
async fn start_repl(initial_query: Option<String>) -> Result<()> {
    // Load configuration
    let config = Config::load()?;

    // Get the main model profile
    let model_profile = config
        .get_model_by_pointer(ModelPointerType::Main)
        .ok_or_else(|| {
            color_eyre::eyre::eyre!(
                "No main model configured. Please configure a model using `kode models --add`"
            )
        })?
        .clone();

    // Create adapter based on provider type
    let adapter: Arc<dyn kode_rs::services::ModelAdapter> = match model_profile.provider {
        ProviderType::Anthropic => Arc::new(AnthropicAdapter::new(model_profile.clone())?),
        ProviderType::OpenAI
        | ProviderType::Ollama
        | ProviderType::Groq
        | ProviderType::Xai
        | ProviderType::CustomOpenAI
        | ProviderType::Custom => Arc::new(OpenAIAdapter::new(model_profile.clone())?),
        _ => {
            return Err(color_eyre::eyre::eyre!(
                "Provider type {:?} is not yet supported",
                model_profile.provider
            ));
        }
    };

    // Run the TUI
    kode_rs::tui::run(initial_query, model_profile, adapter).await?;

    Ok(())
}

/// Handle config commands
fn handle_config_command(
    get: Option<String>,
    set: Option<String>,
    value: Option<String>,
    list: bool,
    global: bool,
) -> Result<()> {
    if list {
        let config = Config::load()?;
        println!("Configuration:");
        println!("  Global config: {:?}", Config::global_config_path());
        println!("  Project config: {:?}", Config::project_config_path());
        println!("\nModel profiles:");
        for profile in &config.global.model_profiles {
            println!(
                "  - {} ({:?})",
                profile.model_name, profile.provider
            );
        }
        println!("\nModel pointers:");
        println!("  main: {}", config.global.model_pointers.main);
        println!("  task: {}", config.global.model_pointers.task);
        println!("  reasoning: {}", config.global.model_pointers.reasoning);
        println!("  quick: {}", config.global.model_pointers.quick);
    } else if let Some(key) = get {
        println!("Getting config key: {key} (global: {global})");
        println!("(not yet implemented)");
    } else if let Some(key) = set {
        if let Some(val) = value {
            println!("Setting {key} = {val} (global: {global})");
            println!("(not yet implemented)");
        }
    }

    Ok(())
}

/// Handle models commands
fn handle_models_command(list: bool, add: bool, remove: Option<String>) -> Result<()> {
    if list {
        let config = Config::load()?;
        println!("Configured models:");
        for profile in &config.global.model_profiles {
            let default = config
                .global
                .default_model_name
                .as_ref()
                .map_or(false, |name| name == &profile.model_name);
            let marker = if default { " (default)" } else { "" };
            println!(
                "  - {}{} ({:?})",
                profile.model_name, marker, profile.provider
            );
        }
    } else if add {
        println!("Adding model... (not yet implemented)");
        println!("Please manually edit your config file at: {:?}", Config::global_config_path());
    } else if let Some(model) = remove {
        println!("Removing model: {model}");
        println!("(not yet implemented)");
    }

    Ok(())
}

/// Handle agents commands
async fn handle_agents_command(list: bool) -> Result<()> {
    if list {
        let registry = AgentRegistry::new(false).await?;
        println!("Available agents:");
        for agent in registry.get_active_agents().await {
            println!("  - {}", agent.agent_type);
            println!("    Description: {}", agent.when_to_use);
            let tools_str = match &agent.tools {
                kode_rs::agents::ToolPermissions::All => "all".to_string(),
                kode_rs::agents::ToolPermissions::Specific(t) => t.join(", "),
            };
            println!("    Tools: {}", tools_str);
            if let Some(model) = &agent.model_name {
                println!("    Model: {model}");
            }
        }
    }

    Ok(())
}
