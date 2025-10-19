//! Kode-rs binary entry point

use color_eyre::Result;
use kode_rs::cli::{Cli, Commands};

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
        Some(Commands::Repl) => {
            println!("Starting REPL... (not yet implemented)");
        }
        Some(Commands::Query { query }) => {
            println!("Executing query: {query}");
            println!("(not yet implemented)");
        }
        Some(Commands::Config {
            get,
            set,
            value,
            list,
            global,
        }) => {
            if list {
                println!("Listing config... (not yet implemented)");
            } else if let Some(key) = get {
                println!("Getting config key: {key} (global: {global})");
            } else if let Some(key) = set {
                if let Some(val) = value {
                    println!("Setting {key} = {val} (global: {global})");
                }
            }
        }
        Some(Commands::Models { list, add, remove }) => {
            if list {
                println!("Listing models... (not yet implemented)");
            } else if add {
                println!("Adding model... (not yet implemented)");
            } else if let Some(model) = remove {
                println!("Removing model: {model}");
            }
        }
        Some(Commands::Agents { list }) => {
            if list {
                println!("Listing agents... (not yet implemented)");
            }
        }
        Some(Commands::Version) => {
            println!("kode-rs version {}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            // Default: start REPL
            println!("Starting REPL... (not yet implemented)");
            println!("Use --help for more information");
        }
    }

    Ok(())
}
