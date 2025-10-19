///! Input field rendering

use crate::tui::app::{App, InputMode};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render input field
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mode_str = match app.input_mode() {
        InputMode::Prompt => "Prompt",
    };

    let mode_color = match app.input_mode() {
        InputMode::Prompt => Color::Green,
    };

    let input = Paragraph::new(app.input_buffer())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", mode_str))
                .border_style(Style::default().fg(mode_color)),
        );

    f.render_widget(input, area);

    // Set cursor position (inside the border)
    if !app.is_loading() {
        let cursor_x = area.x + app.input_buffer().len() as u16 + 1;
        let cursor_y = area.y + 1;

        // Make sure cursor is within bounds
        if cursor_x < area.x + area.width - 1 && cursor_y < area.y + area.height {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }
}
