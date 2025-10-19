///! Terminal setup and cleanup
///!
///! Handles raw mode, alternate screen, and mouse capture for the TUI.

use crate::error::{KodeError, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

/// Terminal type alias for convenience
pub type KodeTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Set up the terminal for TUI mode
///
/// This function:
/// - Enables raw mode (disables line buffering, echo)
/// - Enters alternate screen (saves current terminal state)
/// - Enables mouse capture
///
/// # Errors
/// Returns an error if terminal setup fails
pub fn setup_terminal() -> Result<KodeTerminal> {
    enable_raw_mode().map_err(|e| KodeError::Other(format!("Failed to enable raw mode: {}", e)))?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| KodeError::Other(format!("Failed to enter alternate screen: {}", e)))?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)
        .map_err(|e| KodeError::Other(format!("Failed to create terminal: {}", e)))?;

    Ok(terminal)
}

/// Restore the terminal to its original state
///
/// This function:
/// - Disables raw mode
/// - Leaves alternate screen (restores previous terminal state)
/// - Disables mouse capture
/// - Shows the cursor
///
/// # Errors
/// Returns an error if terminal restoration fails
pub fn restore_terminal(mut terminal: KodeTerminal) -> Result<()> {
    disable_raw_mode().map_err(|e| KodeError::Other(format!("Failed to disable raw mode: {}", e)))?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| KodeError::Other(format!("Failed to leave alternate screen: {}", e)))?;

    terminal
        .show_cursor()
        .map_err(|e| KodeError::Other(format!("Failed to show cursor: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_setup_and_restore() {
        // This test can only run in a real terminal
        // Skip if not in a TTY
        if !atty::is(atty::Stream::Stdout) {
            return;
        }

        let terminal = setup_terminal();
        assert!(terminal.is_ok());

        if let Ok(terminal) = terminal {
            let result = restore_terminal(terminal);
            assert!(result.is_ok());
        }
    }
}
