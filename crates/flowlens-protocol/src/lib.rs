//! Protocol parsers for FlowLens
//!
//! This module provides:
//! - HTTP/1.1 parser (using httparse)
//! - HTTP/2 frame parser (using hpack for header decompression)
//! - WebSocket frame parser
//! - SSE event parser

pub mod http2;
pub mod grpc;

use std::collections::HashMap;

use flowlens_core::models::{
    HttpRequest, HttpResponse, HttpVersion, SseEvent, WebSocketFrame, WsOpcode,
};

/// Protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    Http1,
    Http2,
    WebSocket,
    Sse,
    Grpc,
    Unknown,
}

/// HTTP parse result
#[derive(Debug)]
pub struct HttpParseResult {
    pub request: Option<HttpRequest>,
    pub response: Option<HttpResponse>,
    pub is_upgrade: bool,
    pub bytes_parsed: usize,
}

/// Protocol parser trait
pub trait ProtocolParser: Send + Sync {
    fn parse_http1_request(&self, data: &[u8]) -> Option<HttpRequest>;
    fn parse_http1_response(&self, data: &[u8]) -> Option<HttpResponse>;
    fn parse_websocket_frame(&self, data: &[u8]) -> Option<WebSocketFrame>;
    fn parse_sse_events(&self, data: &[u8]) -> Vec<SseEvent>;
    fn detect_protocol(&self, data: &[u8]) -> ProtocolKind;
}

/// HTTP/1.1 parser using httparse
pub struct Http1Parser;

impl Http1Parser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_http1_request(&self, data: &[u8]) -> Option<HttpRequest> {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);
        if request.parse(data).is_ok() {
            let method = request.method?.to_string();
            let path = request.path?.to_string();
            let version = match request.version? {
                0 => HttpVersion::Http10,
                1 => HttpVersion::Http11,
                _ => HttpVersion::Unknown,
            };
            let hdrs: Vec<(String, String)> = request
                .headers
                .iter()
                .map(|h| {
                    (
                        h.name.to_string(),
                        String::from_utf8_lossy(h.value).to_string(),
                    )
                })
                .collect();
            Some(HttpRequest {
                method,
                url: path.clone(),
                path,
                request_line_version: version,
                headers: hdrs,
                body: None,
                body_size: 0,
                timestamp: 0,
            })
        } else {
            None
        }
    }

    pub fn parse_http1_response(&self, data: &[u8]) -> Option<HttpResponse> {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut response = httparse::Response::new(&mut headers);
        if let Ok(httparse::Status::Complete(code)) = response.parse(data) {
            let version = match response.version? {
                0 => HttpVersion::Http10,
                1 => HttpVersion::Http11,
                _ => HttpVersion::Unknown,
            };
            let hdrs: Vec<(String, String)> = response
                .headers
                .iter()
                .map(|h| {
                    (
                        h.name.to_string(),
                        String::from_utf8_lossy(h.value).to_string(),
                    )
                })
                .collect();
            let st = match code {
                200 => "OK",
                404 => "Not Found",
                500 => "Error",
                _ => "Unknown",
            };
            Some(HttpResponse {
                status_code: code as u16,
                status_text: st.to_string(),
                version,
                headers: hdrs,
                body: None,
                body_size: 0,
                timestamp: 0,
            })
        } else {
            None
        }
    }

    pub fn parse_websocket_frame(&self, data: &[u8]) -> Option<WebSocketFrame> {
        if data.len() < 2 {
            return None;
        }
        let opcode = match data[0] & 0x0F {
            0x1 => WsOpcode::Text,
            0x2 => WsOpcode::Binary,
            0x8 => WsOpcode::Close,
            0x9 => WsOpcode::Ping,
            0xA => WsOpcode::Pong,
            _ => return None,
        };
        let mask_bit = (data[1] & 0x80) != 0;
        let plen = (data[1] & 0x7F) as usize;
        let off = if data.len() >= 4 && plen == 126 { 4 } else { 2 };
        let mask_off = if mask_bit { off + 4 } else { off };
        let plen = plen.min(data.len().saturating_sub(mask_off));
        Some(WebSocketFrame {
            opcode,
            payload: data.get(mask_off..mask_off + plen).unwrap_or(&[]).to_vec(),
            payload_size: plen,
            is_masked: mask_bit,
            timestamp: 0,
        })
    }

    pub fn parse_sse_events(&self, data: &[u8]) -> Vec<SseEvent> {
        let text = match std::str::from_utf8(data) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let mut events = Vec::new();
        let mut cur: HashMap<String, String> = HashMap::new();
        for line in text.lines() {
            if line.is_empty() {
                if let Some(d) = cur.get("data") {
                    events.push(SseEvent {
                        event_type: cur.get("event").cloned(),
                        id: cur.get("id").cloned(),
                        data: d.clone(),
                        retry_ms: None,
                        timestamp: 0,
                    });
                }
                cur.clear();
            } else if let Some((k, v)) = line.split_once(':') {
                cur.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        if !cur.is_empty() {
            if let Some(d) = cur.get("data") {
                events.push(SseEvent {
                    event_type: cur.get("event").cloned(),
                    id: cur.get("id").cloned(),
                    data: d.clone(),
                    retry_ms: None,
                    timestamp: 0,
                });
            }
        }
        events
    }

    pub fn detect_protocol(&self, data: &[u8]) -> ProtocolKind {
        if let Ok(s) = std::str::from_utf8(data) {
            if s.contains("Upgrade: websocket") || s.contains("upgrade: websocket") {
                return ProtocolKind::WebSocket;
            }
            if s.contains("text/event-stream") {
                return ProtocolKind::Sse;
            }
        }
        ProtocolKind::Http1
    }
}

impl Default for Http1Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolParser for Http1Parser {
    fn parse_http1_request(&self, data: &[u8]) -> Option<HttpRequest> {
        Http1Parser::parse_http1_request(self, data)
    }
    fn parse_http1_response(&self, data: &[u8]) -> Option<HttpResponse> {
        Http1Parser::parse_http1_response(self, data)
    }
    fn parse_websocket_frame(&self, data: &[u8]) -> Option<WebSocketFrame> {
        Http1Parser::parse_websocket_frame(self, data)
    }
    fn parse_sse_events(&self, data: &[u8]) -> Vec<SseEvent> {
        Http1Parser::parse_sse_events(self, data)
    }
    fn detect_protocol(&self, data: &[u8]) -> ProtocolKind {
        Http1Parser::detect_protocol(self, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_request() {
        let parser = Http1Parser::new();
        let data = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let r = parser.parse_http1_request(data).unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.path, "/index.html");
    }

    #[test]
    fn test_parse_websocket_frame() {
        let parser = Http1Parser::new();
        let data = vec![0x81, 0x05, b'h', b'e', b'l', b'l', b'o'];
        let f = parser.parse_websocket_frame(&data).unwrap();
        assert_eq!(f.opcode, WsOpcode::Text);
        assert_eq!(f.payload_size, 5);
    }

    #[test]
    fn test_detect_protocol() {
        let parser = Http1Parser::new();
        assert_eq!(
            parser.detect_protocol(b"Upgrade: websocket"),
            ProtocolKind::WebSocket
        );
    }
}
