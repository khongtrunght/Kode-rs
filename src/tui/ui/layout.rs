///! Main layout for the TUI

use super::{input, message, status};
use crate::tui::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

/// Draw the main layout
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Messages area (expandable)
            Constraint::Length(3), // Input field (3 lines with border)
            Constraint::Length(1), // Status bar (1 line)
        ])
        .split(f.area());

    // Render components
    message::render(f, chunks[0], app);
    input::render(f, chunks[1], app);
    status::render(f, chunks[2], app);
}
