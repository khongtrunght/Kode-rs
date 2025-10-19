//! TUI module for kode-rs
//!
//! Provides a terminal user interface using ratatui + crossterm.

mod app;
mod event;
mod terminal;
mod ui;

pub use app::{App, AppEvent, InputMode};
pub use terminal::{restore_terminal, setup_terminal};

use crate::{config::models::ModelProfile, error::Result, services::ModelAdapter};
use std::sync::Arc;

/// Run the TUI application
pub async fn run(
    initial_prompt: Option<String>,
    model_profile: ModelProfile,
    adapter: Arc<dyn ModelAdapter>,
) -> Result<()> {
    // Set up terminal
    let mut terminal = setup_terminal()?;

    // Create app state
    let mut app = App::new(initial_prompt, model_profile, adapter)?;

    // Run the main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    restore_terminal(terminal)?;

    result
}

async fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut event_stream = event::EventStream::new();

    loop {
        // Render UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Wait for next event
        tokio::select! {
            // Terminal events (keyboard, resize)
            Some(event) = event_stream.next() => {
                app.handle_terminal_event(event).await?;
            }

            // Application events (streaming, etc.)
            Some(event) = app.next_event() => {
                app.handle_app_event(event).await?;
            }
        }

        // Check for quit
        if app.should_quit() {
            break;
        }
    }

    Ok(())
}
