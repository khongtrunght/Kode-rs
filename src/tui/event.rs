///! Event handling for the TUI
///!
///! Provides an async stream of terminal events (keyboard, mouse, resize).

use crossterm::event::{self, Event};
use tokio::sync::mpsc;

/// Stream of terminal events
pub struct EventStream {
    rx: mpsc::UnboundedReceiver<Event>,
    _handle: tokio::task::JoinHandle<()>,
}

impl EventStream {
    /// Create a new event stream
    ///
    /// Spawns a background task that reads terminal events and sends them
    /// through a channel.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(async move {
            loop {
                // Read event from terminal (blocking)
                match event::read() {
                    Ok(event) => {
                        if tx.send(event).is_err() {
                            // Channel closed, exit
                            break;
                        }
                    }
                    Err(_) => {
                        // Error reading event, exit
                        break;
                    }
                }
            }
        });

        Self {
            rx,
            _handle: handle,
        }
    }

    /// Get the next event from the stream
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

impl Default for EventStream {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_stream_creation() {
        // Just verify we can create an event stream
        let _stream = EventStream::new();
        // Can't easily test event reading without a real terminal
    }
}
