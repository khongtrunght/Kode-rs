///! Simplified application state for the TUI
///!
///! This is a simplified version that avoids ModelManager complexity for MVP.

use crate::{
    config::models::ModelProfile,
    error::{KodeError, Result},
    messages::{AssistantMessage, ContentBlock, InternalMessage, Message, UserMessage},
    services::{CompletionChunk, ModelAdapter, Usage},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal prompt mode
    Prompt,
}

/// Application events
#[derive(Debug)]
pub enum AppEvent {
    /// Streaming chunk received
    StreamChunk(CompletionChunk),
    /// Streaming completed
    StreamComplete,
    /// Streaming error
    StreamError(KodeError),
}

/// Main application state
pub struct App {
    /// Message history
    messages: Vec<Message>,

    /// Current input buffer
    input_buffer: String,

    /// Current input mode
    input_mode: InputMode,

    /// Scroll offset for message view
    scroll_offset: usize,

    /// Whether the app is loading (streaming)
    is_loading: bool,

    /// Should quit flag
    should_quit: bool,

    /// Model profile
    model_profile: ModelProfile,

    /// Model adapter
    adapter: Arc<dyn ModelAdapter>,

    /// Event channel for app events
    event_tx: mpsc::UnboundedSender<AppEvent>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,

    /// Current stream handle
    current_stream: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    /// Create a new app
    pub fn new(
        initial_prompt: Option<String>,
        model_profile: ModelProfile,
        adapter: Arc<dyn ModelAdapter>,
    ) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Ok(Self {
            messages: Vec::new(),
            input_buffer: initial_prompt.unwrap_or_default(),
            input_mode: InputMode::Prompt,
            scroll_offset: 0,
            is_loading: false,
            should_quit: false,
            model_profile,
            adapter,
            event_tx,
            event_rx,
            current_stream: None,
        })
    }

    /// Get the next application event
    pub async fn next_event(&mut self) -> Option<AppEvent> {
        self.event_rx.recv().await
    }

    /// Check if should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Get messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get input buffer
    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }

    /// Get input mode
    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    /// Get scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Check if loading
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Handle terminal event
    pub async fn handle_terminal_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event).await?,
            Event::Resize(_, _) => {
                // Handle resize - ratatui handles this automatically
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle key event
    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        // Handle Ctrl+C to quit
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        match key.code {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                self.submit_prompt().await?;
            }
            KeyCode::Up => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            KeyCode::Down => {
                self.scroll_offset += 1;
            }
            KeyCode::Esc => {
                if self.is_loading {
                    self.cancel_stream().await;
                } else {
                    self.should_quit = true;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Submit the current prompt
    async fn submit_prompt(&mut self) -> Result<()> {
        if self.input_buffer.trim().is_empty() {
            return Ok(());
        }

        // Add user message
        let user_content = self.input_buffer.clone();
        self.input_buffer.clear();

        let user_message = Message::User(UserMessage {
            content: user_content.clone(),
        });
        self.messages.push(user_message);

        // Create empty assistant message
        let assistant_message = Message::Assistant(AssistantMessage {
            id: uuid::Uuid::new_v4().to_string(),
            model: self.model_profile.name.clone(),
            role: "assistant".to_string(),
            stop_reason: None,
            stop_sequence: None,
            usage: Usage::default(),
            message: InternalMessage {
                content: Vec::new(),
            },
        });
        self.messages.push(assistant_message);

        // Start streaming
        self.start_streaming(user_content).await?;

        Ok(())
    }

    /// Start streaming response
    async fn start_streaming(&mut self, _prompt: String) -> Result<()> {
        self.is_loading = true;

        // Convert messages to API format
        let api_messages = self
            .messages
            .iter()
            .filter_map(|msg| match msg {
                Message::User(user_msg) => Some(Message::User(user_msg.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();

        // Get tool schemas (empty for now)
        let tools = Vec::new();

        // Start streaming
        let event_tx = self.event_tx.clone();
        let adapter = self.adapter.clone();
        let model_profile = self.model_profile.clone();

        let stream = adapter
            .stream_complete(api_messages, tools, &model_profile)
            .await?;

        let handle = tokio::spawn(async move {
            tokio::pin!(stream);

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if event_tx.send(AppEvent::StreamChunk(chunk)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::StreamError(e));
                        break;
                    }
                }
            }

            let _ = event_tx.send(AppEvent::StreamComplete);
        });

        self.current_stream = Some(handle);

        Ok(())
    }

    /// Cancel current stream
    async fn cancel_stream(&mut self) {
        if let Some(handle) = self.current_stream.take() {
            handle.abort();
        }
        self.is_loading = false;
    }

    /// Handle application event
    pub async fn handle_app_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::StreamChunk(chunk) => {
                self.handle_stream_chunk(chunk)?;
            }
            AppEvent::StreamComplete => {
                self.is_loading = false;
                self.current_stream = None;
            }
            AppEvent::StreamError(err) => {
                self.is_loading = false;
                self.current_stream = None;
                // Add error message
                let error_msg = Message::User(UserMessage {
                    content: format!("Error: {}", err),
                });
                self.messages.push(error_msg);
            }
        }

        Ok(())
    }

    /// Handle streaming chunk
    fn handle_stream_chunk(&mut self, chunk: CompletionChunk) -> Result<()> {
        // Get the last message (should be assistant message)
        if let Some(Message::Assistant(ref mut asst_msg)) = self.messages.last_mut() {
            match chunk {
                CompletionChunk::TextDelta { delta, .. } => {
                    // Append to last text block or create new one
                    if let Some(ContentBlock::Text(ref mut text)) =
                        asst_msg.message.content.last_mut()
                    {
                        text.push_str(&delta);
                    } else {
                        asst_msg
                            .message
                            .content
                            .push(ContentBlock::Text(delta));
                    }
                }
                CompletionChunk::ThinkingDelta { delta, .. } => {
                    // Append to last thinking block or create new one
                    if let Some(ContentBlock::Thinking(ref mut thinking)) =
                        asst_msg.message.content.last_mut()
                    {
                        thinking.push_str(&delta);
                    } else {
                        asst_msg
                            .message
                            .content
                            .push(ContentBlock::Thinking(delta));
                    }
                }
                CompletionChunk::ToolUseComplete { tool_use } => {
                    asst_msg
                        .message
                        .content
                        .push(ContentBlock::ToolUse(tool_use));
                }
                CompletionChunk::Done {
                    stop_reason, usage, ..
                } => {
                    asst_msg.stop_reason = stop_reason;
                    asst_msg.usage = usage;
                }
            }
        }

        Ok(())
    }
}
