# TUI Architecture Plan

## Overview
Port the TypeScript/Ink-based REPL to Rust using ratatui + crossterm. The goal is to maintain feature parity while simplifying where possible.

## TypeScript REPL Analysis

### Key Features (from REPL.tsx - 809 lines)
1. **Message Display**: Chat history with assistant/user messages
2. **Streaming**: Real-time updates as assistant responds
3. **Input Handling**: Multi-line input with history
4. **Tool Use Permissions**: Interactive approval for tool calls
5. **Cost Tracking**: Display cost and usage statistics
6. **Binary Feedback**: Compare two responses (A/B testing)
7. **Message Selector**: Choose between multiple messages
8. **Fork Management**: Branch conversations
9. **Mode Indicators**: Show current input mode (prompt/bash/koding)
10. **Syntax Highlighting**: Code blocks with tree-sitter
11. **Keyboard Shortcuts**: Ctrl+C to cancel, etc.

### Component Structure (Ink/React)
```
REPL (main component)
├── Logo (header with version info)
├── Static (scrollback message history)
│   └── Message (individual messages)
│       └── MessageResponse (tool uses, thinking, etc.)
├── PermissionRequest (tool approval dialog)
├── BinaryFeedback (A/B comparison)
├── MessageSelector (choose between messages)
├── ModeIndicator (current mode)
├── PromptInput (text input with multiline)
└── Spinner (loading indicator)
```

## Rust TUI Architecture

### Technology Stack
- **ratatui** 0.29: TUI framework (replaces Ink)
- **crossterm** 0.28: Terminal control (replaces React's terminal handling)
- **tokio**: Async runtime for streaming
- **tree-sitter**: Syntax highlighting (same as TypeScript)

### Module Structure
```
src/tui/
├── mod.rs              # Public API, re-exports
├── app.rs              # Main application state
├── terminal.rs         # Terminal setup/cleanup
├── event.rs            # Event handling (keyboard, resize)
├── ui/
│   ├── mod.rs          # UI components
│   ├── layout.rs       # Layout management
│   ├── message.rs      # Message rendering
│   ├── input.rs        # Input field
│   ├── permission.rs   # Permission dialog
│   └── status.rs       # Status bar
└── highlight.rs        # Syntax highlighting
```

### State Management

#### AppState (main application state)
```rust
pub struct AppState {
    // Messages
    messages: Vec<Message>,

    // UI state
    scroll_offset: usize,
    input_buffer: String,
    input_mode: InputMode,

    // Streaming
    is_loading: bool,
    current_stream: Option<JoinHandle<()>>,

    // Permission system
    pending_permission: Option<PermissionRequest>,

    // Configuration
    verbose: bool,
    safe_mode: bool,

    // Model & tools
    model_manager: ModelManager,
    tools: Vec<Arc<dyn Tool>>,

    // Event channels
    event_tx: mpsc::UnboundedSender<AppEvent>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
}
```

#### Events
```rust
pub enum AppEvent {
    // User input
    Input(char),
    Backspace,
    Enter,
    Submit,

    // Streaming events
    StreamChunk(CompletionChunk),
    StreamComplete,
    StreamError(KodeError),

    // Permission events
    PermissionRequest(ToolUseRequest),
    PermissionApproved,
    PermissionDenied,

    // UI events
    Resize(u16, u16),
    Scroll(isize),

    // Control
    Cancel,
    Quit,
}
```

### Event Loop Pattern

```rust
async fn run(mut app: AppState) -> Result<()> {
    // Set up terminal
    let mut terminal = setup_terminal()?;

    // Create event stream
    let mut event_stream = EventStream::new();

    loop {
        // Render UI
        terminal.draw(|f| ui::draw(f, &app))?;

        // Wait for events
        tokio::select! {
            // Terminal events (keyboard, resize)
            Some(event) = event_stream.next() => {
                handle_terminal_event(&mut app, event).await?;
            }

            // Application events (streaming, permissions)
            Some(event) = app.event_rx.recv() => {
                handle_app_event(&mut app, event).await?;
            }
        }

        // Check for quit
        if app.should_quit {
            break;
        }
    }

    // Clean up terminal
    restore_terminal(terminal)?;
    Ok(())
}
```

### Simplified vs. Full Feature Set

#### MVP (Phase 1 - This Session)
- ✅ Basic message display (text only)
- ✅ Simple input handling (single line)
- ✅ Streaming updates (show chunks as they arrive)
- ✅ Basic error display
- ✅ Quit on Ctrl+C
- ✅ Scroll support (arrow keys)

#### Phase 2
- Multi-line input
- Permission request UI
- Tool use visualization
- Cost tracking display

#### Phase 3
- Syntax highlighting
- Message selector
- Binary feedback
- Fork management

## Implementation Plan

### 1. Terminal Setup (terminal.rs)
```rust
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore_terminal(mut terminal: Terminal<...>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
```

### 2. Event Handling (event.rs)
```rust
pub struct EventStream {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventStream {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn event reader thread
        tokio::spawn(async move {
            loop {
                if let Ok(event) = crossterm::event::read() {
                    if tx.send(event).is_err() {
                        break;
                    }
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
```

### 3. Message Rendering (ui/message.rs)
```rust
pub fn render_messages(
    f: &mut Frame,
    area: Rect,
    messages: &[Message],
    scroll: usize,
) {
    let mut lines = Vec::new();

    for msg in messages {
        match msg {
            Message::User(user_msg) => {
                lines.push(Line::from(vec![
                    Span::styled("User: ", Style::default().fg(Color::Blue)),
                    Span::raw(&user_msg.content),
                ]));
            }
            Message::Assistant(asst_msg) => {
                lines.push(Line::from(vec![
                    Span::styled("Assistant: ", Style::default().fg(Color::Green)),
                ]));

                for block in &asst_msg.message.content {
                    match block {
                        ContentBlock::Text(text) => {
                            lines.push(Line::from(text.text.clone()));
                        }
                        ContentBlock::ToolUse(tool_use) => {
                            lines.push(Line::from(format!("[Tool: {}]", tool_use.name)));
                        }
                        // ... other block types
                    }
                }
            }
        }
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
```

### 4. Input Field (ui/input.rs)
```rust
pub fn render_input(f: &mut Frame, area: Rect, buffer: &str, mode: InputMode) {
    let mode_text = match mode {
        InputMode::Prompt => "Prompt",
        InputMode::Bash => "Bash",
        InputMode::Koding => "Koding",
    };

    let input = Paragraph::new(buffer)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", mode_text)));

    f.render_widget(input, area);
    f.set_cursor(area.x + buffer.len() as u16 + 1, area.y + 1);
}
```

### 5. Main UI Layout (ui/layout.rs)
```rust
pub fn draw(f: &mut Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),       // Messages
            Constraint::Length(3),    // Input
            Constraint::Length(1),    // Status bar
        ])
        .split(f.area());

    // Render components
    message::render_messages(f, chunks[0], &app.messages, app.scroll_offset);
    input::render_input(f, chunks[1], &app.input_buffer, app.input_mode);
    status::render_status(f, chunks[2], app);
}
```

## Streaming Integration

### Handling Completion Streams
```rust
async fn handle_submit(app: &mut AppState) -> Result<()> {
    let user_message = Message::User(UserMessage {
        content: app.input_buffer.clone(),
    });
    app.messages.push(user_message);
    app.input_buffer.clear();

    // Create assistant message placeholder
    let assistant_message = Message::Assistant(AssistantMessage {
        message: InternalMessage {
            content: vec![],
        },
        // ... other fields
    });
    app.messages.push(assistant_message);

    // Start streaming
    let event_tx = app.event_tx.clone();
    let handle = tokio::spawn(async move {
        let stream = adapter.stream_complete(...).await?;

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    event_tx.send(AppEvent::StreamChunk(chunk))?;
                }
                Err(e) => {
                    event_tx.send(AppEvent::StreamError(e))?;
                    break;
                }
            }
        }

        event_tx.send(AppEvent::StreamComplete)?;
        Ok::<_, KodeError>(())
    });

    app.current_stream = Some(handle);
    app.is_loading = true;

    Ok(())
}

async fn handle_stream_chunk(app: &mut AppState, chunk: CompletionChunk) {
    let last_msg = app.messages.last_mut().unwrap();

    if let Message::Assistant(asst_msg) = last_msg {
        match chunk {
            CompletionChunk::TextDelta { delta, .. } => {
                // Append to last text block or create new one
                if let Some(ContentBlock::Text(text)) = asst_msg.message.content.last_mut() {
                    text.text.push_str(&delta);
                } else {
                    asst_msg.message.content.push(ContentBlock::Text(TextBlock {
                        text: delta,
                    }));
                }
            }
            CompletionChunk::ToolUseComplete { tool_use } => {
                asst_msg.message.content.push(ContentBlock::ToolUse(tool_use));
            }
            CompletionChunk::Done { .. } => {
                app.is_loading = false;
            }
            // ... other chunk types
        }
    }
}
```

## Testing Strategy

### Unit Tests
- Message rendering logic
- Input handling
- Event parsing

### Integration Tests
- Full REPL flow with mocked streams
- Permission system
- Error handling

### Manual Testing
- Test with real APIs
- Verify streaming works
- Check terminal cleanup on Ctrl+C

## File Creation Order

1. ✅ `src/tui/mod.rs` - Module root
2. ✅ `src/tui/terminal.rs` - Terminal setup
3. ✅ `src/tui/event.rs` - Event stream
4. ✅ `src/tui/app.rs` - Application state
5. ✅ `src/tui/ui/mod.rs` - UI module
6. ✅ `src/tui/ui/layout.rs` - Main layout
7. ✅ `src/tui/ui/message.rs` - Message rendering
8. ✅ `src/tui/ui/input.rs` - Input field
9. ✅ `src/tui/ui/status.rs` - Status bar
10. ✅ Wire up in `main.rs`

## Success Criteria

### MVP Complete When:
- [x] Can start REPL from CLI
- [ ] Messages display correctly
- [ ] Can type and submit prompts
- [ ] Streaming updates show in real-time
- [ ] Ctrl+C exits cleanly
- [ ] Terminal state restored on exit
- [ ] Basic error handling works

## Time Estimate

- **Setup + Terminal**: 30 mins
- **Event Handling**: 30 mins
- **Basic UI**: 1 hour
- **Streaming Integration**: 1 hour
- **Testing + Debugging**: 1 hour
- **Total**: ~4 hours for MVP

## Notes

- Keep it simple for MVP - avoid over-engineering
- Focus on working end-to-end flow first
- Add features incrementally after MVP works
- Prioritize reliability over features
