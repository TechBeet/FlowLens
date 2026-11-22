//! HTTP/1.1 body parsing for FlowLens
//!
//! Supports:
//! - Content-Length based bodies
//! - Transfer-Encoding: chunked
//! - Content-Type detection

use flowlens_core::models::{HttpRequest, HttpResponse};

/// Parse HTTP request body from raw data after headers.
pub fn parse_request_body(data: &[u8], headers: &[(String, String)]) -> Option<Vec<u8>> {
    parse_body(data, headers)
}

/// Parse HTTP response body from raw data after headers.
pub fn parse_response_body(data: &[u8], headers: &[(String, String)]) -> Option<Vec<u8>> {
    parse_body(data, headers)
}

fn parse_body(data: &[u8], headers: &[(String, String)]) -> Option<Vec<u8>> {
    // Check Transfer-Encoding
    let transfer_encoding = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("transfer-encoding"))
        .map(|(_, v)| v.to_ascii_lowercase());

    if let Some(te) = transfer_encoding {
        if te.contains("chunked") {
            return parse_chunked_body(data);
        }
    }

    // Check Content-Length
    let content_length = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok());

    if let Some(cl) = content_length {
        if cl == 0 {
            return Some(Vec::new());
        }
        if data.len() >= cl {
            return Some(data[..cl].to_vec());
        }
    }

    // If no content-length and not chunked, return all remaining data
    if data.is_empty() {
        None
    } else {
        Some(data.to_vec())
    }
}

fn parse_chunked_body(data: &[u8]) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    let mut pos = 0;
    let text = std::str::from_utf8(data).ok()?;

    loop {
        // Find next chunk size line
        let line_end = text[pos..].find("\r\n")?;
        let size_str = text[pos..pos + line_end].trim();
        let size = usize::from_str_radix(size_str.split(';').next()?.trim(), 16).ok()?;

        if size == 0 {
            break;
        }

        pos += line_end + 2;
        if pos + size > data.len() {
            break;
        }
        result.extend_from_slice(&data[pos..pos + size]);
        pos += size + 2; // skip \r\n
    }

    Some(result)
}

/// Extract Content-Type from headers.
pub fn content_type(headers: &[(String, String)]) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.clone())
}

/// Check if content is likely text-based.
pub fn is_text_content(headers: &[(String, String)]) -> bool {
    content_type(headers)
        .map(|ct| {
            ct.starts_with("text/")
                || ct.contains("json")
                || ct.contains("xml")
                || ct.contains("javascript")
                || ct.contains("form-urlencoded")
        })
        .unwrap_or(false)
}

/// Check if this is a WebSocket upgrade request.
pub fn is_websocket_upgrade(request: &HttpRequest) -> bool {
    let upgrade = request
        .headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("upgrade") && v.eq_ignore_ascii_case("websocket"));
    let connection = request
        .headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("connection") && v.to_lowercase().contains("upgrade"));
    upgrade && connection
}

/// Check if response is SSE.
pub fn is_sse_response(response: &HttpResponse) -> bool {
    response
        .headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("content-type") && v.contains("text/event-stream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_length_body() {
        let headers = vec![("Content-Length".to_string(), "5".to_string())];
        let data = b"hello world";
        let body = parse_body(data, &headers).unwrap();
        assert_eq!(body, b"hello");
    }

    #[test]
    fn test_parse_chunked_body() {
        let headers = vec![("Transfer-Encoding".to_string(), "chunked".to_string())];
        let data = b"5\r\nhello\r\n0\r\n\r\n";
        let body = parse_body(data, &headers).unwrap();
        assert_eq!(body, b"hello");
    }

    #[test]
    fn test_is_text_content() {
        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        assert!(is_text_content(&headers));

        let headers2 = vec![("Content-Type".to_string(), "image/png".to_string())];
        assert!(!is_text_content(&headers2));
    }

    #[test]
    fn test_is_websocket_upgrade() {
        let req = HttpRequest {
            method: "GET".to_string(),
            url: "/ws".to_string(),
            path: "/ws".to_string(),
            request_line_version: flowlens_core::models::HttpVersion::Http11,
            headers: vec![
                ("Upgrade".to_string(), "websocket".to_string()),
                ("Connection".to_string(), "Upgrade".to_string()),
            ],
            body: None,
            body_size: 0,
            timestamp: 0,
        };
        assert!(is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_sse_response() {
        let resp = HttpResponse {
            status_code: 200,
            status_text: "OK".to_string(),
            version: flowlens_core::models::HttpVersion::Http11,
            headers: vec![("Content-Type".to_string(), "text/event-stream".to_string())],
            body: None,
            body_size: 0,
            timestamp: 0,
        };
        assert!(is_sse_response(&resp));
    }
}
