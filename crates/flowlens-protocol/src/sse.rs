//! SSE (Server-Sent Events) parser for FlowLens
//!
//! Supports:
//! - event, data, id, retry fields
//! - Multi-line data
//! - retry_ms parsing

use flowlens_core::models::SseEvent;

/// Parse SSE events from raw data.
pub fn parse_sse_events(data: &[u8]) -> Vec<SseEvent> {
    let text = match std::str::from_utf8(data) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut events = Vec::new();
    let mut cur_event_type: Option<String> = None;
    let mut cur_id: Option<String> = None;
    let mut cur_data: Vec<String> = Vec::new();
    let mut cur_retry: Option<u64> = None;

    for line in text.lines() {
        if line.is_empty() {
            // End of event
            if !cur_data.is_empty() {
                events.push(SseEvent {
                    event_type: cur_event_type.take(),
                    id: cur_id.take(),
                    data: cur_data.join("\n"),
                    retry_ms: cur_retry.take(),
                    timestamp: 0,
                });
                cur_data.clear();
            }
            // Reset per-event fields that should persist? No, reset all
            cur_event_type = None;
            cur_id = None;
            cur_retry = None;
        } else if let Some(stripped) = line.strip_prefix(':') {
            // Comment, ignore
            let _ = stripped;
        } else if let Some((field, value)) = line.split_once(':') {
            let field = field.trim();
            let value = value.trim_start_matches(' ').to_string();
            match field {
                "event" => cur_event_type = Some(value),
                "data" => cur_data.push(value),
                "id" => cur_id = Some(value),
                "retry" => {
                    if let Ok(ms) = value.parse::<u64>() {
                        cur_retry = Some(ms);
                    }
                }
                _ => {}
            }
        } else {
            // Field with no colon - treat entire line as field name with empty value
            match line {
                "event" => {}
                _ => {}
            }
        }
    }

    // Handle trailing event without empty line
    if !cur_data.is_empty() {
        events.push(SseEvent {
            event_type: cur_event_type,
            id: cur_id,
            data: cur_data.join("\n"),
            retry_ms: cur_retry,
            timestamp: 0,
        });
    }

    events
}

/// Check if a response is SSE based on Content-Type.
pub fn is_sse_content_type(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("content-type") && v.contains("text/event-stream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_event() {
        let data = b"event: message\ndata: hello world\n\n";
        let events = parse_sse_events(data);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type.as_deref(), Some("message"));
        assert_eq!(events[0].data, "hello world");
    }

    #[test]
    fn test_parse_multi_data() {
        let data = b"data: line1\ndata: line2\n\n";
        let events = parse_sse_events(data);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn test_parse_retry_ms() {
        let data = b"retry: 3000\ndata: hello\n\n";
        let events = parse_sse_events(data);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry_ms, Some(3000));
    }

    #[test]
    fn test_parse_id() {
        let data = b"id: 42\ndata: hello\n\n";
        let events = parse_sse_events(data);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id.as_deref(), Some("42"));
    }

    #[test]
    fn test_parse_multiple_events() {
        let data = b"data: first\n\ndata: second\n\n";
        let events = parse_sse_events(data);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn test_trailing_event() {
        let data = b"data: trailing";
        let events = parse_sse_events(data);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "trailing");
    }
}