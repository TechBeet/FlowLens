//! Protocol parsers for FlowLens
//!
//! This module provides:
//! - HTTP/1.1 parser (using httparse)
//! - HTTP/2 frame parser (using hpack for header decompression)
//! - WebSocket frame parser
//! - SSE event parser

pub mod grpc;
pub mod http1_body;
pub mod http2;
pub mod sse;
pub mod websocket;

use flowlens_core::models::{
    HttpRequest, HttpResponse, HttpVersion, SseEvent, WebSocketFrame,
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

            // Parse body using http1_body module
            let header_end = find_header_end(data)?;
            let body_data = &data[header_end..];
            let body = http1_body::parse_request_body(body_data, &hdrs);

            Some(HttpRequest {
                method,
                url: path.clone(),
                path,
                request_line_version: version,
                headers: hdrs,
                body,
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
            // Parse status text based on code
            let st = match code {
                200 => "OK",
                404 => "Not Found",
                500 => "Error",
                _ => "Unknown",
            };
            let header_end = find_header_end(data)?;
            let body_data = &data[header_end..];
            let body = http1_body::parse_response_body(body_data, &hdrs);
            Some(HttpResponse {
                status_code: code as u16,
                status_text: st.to_string(),
                version,
                headers: hdrs,
                body,
                body_size: 0,
                timestamp: 0,
            })
        } else {
            None
        }
    }

    pub fn parse_websocket_frame(&self, data: &[u8]) -> Option<WebSocketFrame> {
        websocket::parse_websocket_frame(data).map(|(f, _)| f)
    }

    pub fn parse_sse_events(&self, data: &[u8]) -> Vec<SseEvent> {
        sse::parse_sse_events(data)
    }

    pub fn detect_protocol(&self, data: &[u8]) -> ProtocolKind {
        if let Ok(s) = std::str::from_utf8(data) {
            let lower = s.to_lowercase();
            if lower.contains("upgrade: websocket") {
                return ProtocolKind::WebSocket;
            }
            if lower.contains("text/event-stream") {
                return ProtocolKind::Sse;
            }
            if lower.contains("content-type: application/grpc") {
                return ProtocolKind::Grpc;
            }
            if lower.starts_with("pri * http/2.0") || lower.contains("http/2") {
                return ProtocolKind::Http2;
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
        assert_eq!(f.opcode, flowlens_core::models::WsOpcode::Text);
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

fn find_header_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if data[i] == b'\r' && data[i + 1] == b'\n' && data[i + 2] == b'\r' && data[i + 3] == b'\n' {
            return Some(i + 4);
        }
    }
    for i in 0..data.len().saturating_sub(1) {
        if data[i] == b'\n' && data[i + 1] == b'\n' {
            return Some(i + 2);
        }
    }
    None
}
