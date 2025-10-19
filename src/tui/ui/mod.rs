///! UI rendering components

mod input;
mod layout;
mod message;
mod status;

use crate::tui::app::App;
use ratatui::Frame;

/// Main draw function - renders the entire UI
pub fn draw(f: &mut Frame, app: &App) {
    layout::draw(f, app);
}
