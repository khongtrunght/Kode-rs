///! Message rendering

use crate::{
    messages::{ContentBlock, Role},
    tui::app::App,
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Render messages area
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();

    // Add messages
    for msg in app.messages() {
        match msg.role {
            Role::User => {
                // User message header
                lines.push(Line::from(vec![
                    Span::styled(
                        "You: ",
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(msg.text_content()),
                ]));
                lines.push(Line::from("")); // Empty line for spacing
            }
            Role::Assistant => {
                // Assistant message header
                lines.push(Line::from(Span::styled(
                    "Assistant:",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));

                // Content blocks
                for block in &msg.content {
                    match block {
                        ContentBlock::Text { text } => {
                            // Split text into lines
                            for line in text.lines() {
                                lines.push(Line::from(line.to_string()));
                            }
                        }
                        ContentBlock::Thinking { thinking } => {
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "[Thinking] ",
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::ITALIC),
                                ),
                                Span::styled(
                                    thinking,
                                    Style::default().add_modifier(Modifier::ITALIC),
                                ),
                            ]));
                        }
                        ContentBlock::ToolUse { id: _, name, input } => {
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "[Tool: ",
                                    Style::default().fg(Color::Magenta),
                                ),
                                Span::styled(
                                    name,
                                    Style::default()
                                        .fg(Color::Magenta)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled("] ", Style::default().fg(Color::Magenta)),
                                Span::raw(
                                    serde_json::to_string(input)
                                        .unwrap_or_else(|_| "{}".to_string()),
                                ),
                            ]));
                        }
                        ContentBlock::ToolResult {
                            tool_use_id: _,
                            content,
                            is_error,
                        } => {
                            let color = if is_error.unwrap_or(false) {
                                Color::Red
                            } else {
                                Color::Cyan
                            };
                            lines.push(Line::from(vec![
                                Span::styled("[Result] ", Style::default().fg(color)),
                                Span::raw(content),
                            ]));
                        }
                    }
                }

                lines.push(Line::from("")); // Empty line for spacing
            }
            Role::System => {
                // System messages are usually not displayed in the UI
            }
        }
    }

    // Show loading indicator
    if app.is_loading() {
        lines.push(Line::from(Span::styled(
            "Loading...",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    // Create paragraph with scroll support
    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Chat History "),
        )
        .scroll((app.scroll_offset() as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
