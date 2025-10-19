///! Status bar rendering

use crate::tui::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render status bar
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![];

    // Loading indicator
    if app.is_loading() {
        spans.push(Span::styled(
            " ● ",
            Style::default().fg(Color::Yellow),
        ));
    } else {
        spans.push(Span::raw(" "));
    }

    // Help text
    spans.push(Span::styled(
        "Press ",
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        "Enter",
        Style::default().fg(Color::White),
    ));
    spans.push(Span::styled(
        " to submit, ",
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        "Esc",
        Style::default().fg(Color::White),
    ));
    spans.push(Span::styled(
        " to cancel/quit, ",
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        "Ctrl+C",
        Style::default().fg(Color::White),
    ));
    spans.push(Span::styled(
        " to quit",
        Style::default().fg(Color::DarkGray),
    ));

    // Safe mode indicator
    if app.is_loading() {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            "Loading...",
            Style::default().fg(Color::Yellow),
        ));
    }

    let status = Paragraph::new(Line::from(spans));
    f.render_widget(status, area);
}
