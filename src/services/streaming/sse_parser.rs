//! Server-Sent Events (SSE) parser
//!
//! Parses SSE streams following the W3C spec and provider-specific formats.

use std::collections::HashMap;

/// SSE event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Event type (e.g., "message_start", "content_block_delta")
    pub event_type: Option<String>,

    /// Event data (JSON payload)
    pub data: String,

    /// Event ID (for reconnection)
    pub id: Option<String>,

    /// Retry delay in milliseconds
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Create a new SSE event
    pub fn new() -> Self {
        Self {
            event_type: None,
            data: String::new(),
            id: None,
            retry: None,
        }
    }

    /// Check if event is complete (has data)
    pub fn is_complete(&self) -> bool {
        !self.data.is_empty()
    }

    /// Check if this is the done marker for OpenAI streams
    pub fn is_done_marker(&self) -> bool {
        self.data == "[DONE]"
    }
}

impl Default for SseEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// SSE parser for streaming responses
///
/// Follows the W3C Server-Sent Events specification:
/// https://html.spec.whatwg.org/multipage/server-sent-events.html
pub struct SseParser {
    /// Current event being assembled
    current_event: SseEvent,

    /// Buffer for incomplete lines
    line_buffer: String,
}

impl SseParser {
    /// Create a new SSE parser
    pub fn new() -> Self {
        Self {
            current_event: SseEvent::new(),
            line_buffer: String::new(),
        }
    }

    /// Parse a chunk of SSE data
    ///
    /// Returns completed events. Incomplete events are buffered until next call.
    pub fn parse_chunk(&mut self, chunk: &str) -> Vec<SseEvent> {
        let mut events = Vec::new();

        // Add chunk to buffer
        self.line_buffer.push_str(chunk);

        // Process complete lines
        while let Some(line_end) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..line_end].trim_end_matches('\r').to_string();
            self.line_buffer.drain(..=line_end);

            // Process the line
            if let Some(event) = self.process_line(&line) {
                events.push(event);
            }
        }

        events
    }

    /// Process a single line from the SSE stream
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Empty line signals end of event
        if line.is_empty() {
            if self.current_event.is_complete() {
                let event = self.current_event.clone();
                self.current_event = SseEvent::new();
                return Some(event);
            }
            return None;
        }

        // Ignore comments
        if line.starts_with(':') {
            return None;
        }

        // Parse field
        if let Some((field, value)) = Self::parse_field(line) {
            match field {
                "event" => {
                    self.current_event.event_type = Some(value.to_string());
                }
                "data" => {
                    if !self.current_event.data.is_empty() {
                        self.current_event.data.push('\n');
                    }
                    self.current_event.data.push_str(value);
                }
                "id" => {
                    self.current_event.id = Some(value.to_string());
                }
                "retry" => {
                    if let Ok(retry_ms) = value.parse::<u64>() {
                        self.current_event.retry = Some(retry_ms);
                    }
                }
                _ => {
                    // Unknown field, ignore
                }
            }
        }

        None
    }

    /// Parse a field line into (field_name, value)
    fn parse_field(line: &str) -> Option<(&str, &str)> {
        if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = &line[colon_pos + 1..];

            // Remove optional space after colon
            let value = value.strip_prefix(' ').unwrap_or(value);

            Some((field, value))
        } else {
            // Field with no value
            Some((line, ""))
        }
    }

    /// Flush any remaining buffered event
    pub fn flush(&mut self) -> Option<SseEvent> {
        // Process any remaining line in buffer
        if !self.line_buffer.is_empty() {
            let line = self.line_buffer.clone();
            self.line_buffer.clear();
            self.process_line(&line);
        }

        if self.current_event.is_complete() {
            let event = self.current_event.clone();
            self.current_event = SseEvent::new();
            Some(event)
        } else {
            None
        }
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_event() {
        let mut parser = SseParser::new();
        let chunk = "event: message\ndata: {\"text\":\"hello\"}\n\n";

        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, Some("message".to_string()));
        assert_eq!(events[0].data, r#"{"text":"hello"}"#);
    }

    #[test]
    fn test_parse_multi_line_data() {
        let mut parser = SseParser::new();
        let chunk = "event: test\ndata: line1\ndata: line2\n\n";

        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn test_parse_multiple_events() {
        let mut parser = SseParser::new();
        let chunk = "event: msg1\ndata: data1\n\nevent: msg2\ndata: data2\n\n";

        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, Some("msg1".to_string()));
        assert_eq!(events[1].event_type, Some("msg2".to_string()));
    }

    #[test]
    fn test_parse_with_id() {
        let mut parser = SseParser::new();
        let chunk = "event: message\nid: 123\ndata: test\n\n";

        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("123".to_string()));
    }

    #[test]
    fn test_parse_done_marker() {
        let mut parser = SseParser::new();
        let chunk = "data: [DONE]\n\n";

        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert!(events[0].is_done_marker());
    }

    #[test]
    fn test_parse_incomplete_event() {
        let mut parser = SseParser::new();

        // First chunk: incomplete event
        let chunk1 = "event: message\ndata: partial";
        let events = parser.parse_chunk(chunk1);
        assert_eq!(events.len(), 0); // No complete event yet

        // Second chunk: completion
        let chunk2 = "\n\n";
        let events = parser.parse_chunk(chunk2);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "partial");
    }

    #[test]
    fn test_ignore_comments() {
        let mut parser = SseParser::new();
        let chunk = ": this is a comment\nevent: message\ndata: test\n\n";

        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, Some("message".to_string()));
    }

    #[test]
    fn test_flush() {
        let mut parser = SseParser::new();
        let events = parser.parse_chunk("event: message\ndata: test");

        // No complete events yet (missing final newline)
        assert_eq!(events.len(), 0);

        // Flush should return the incomplete event
        let event = parser.flush();
        assert!(event.is_some());
        assert_eq!(event.unwrap().data, "test");
    }
}
