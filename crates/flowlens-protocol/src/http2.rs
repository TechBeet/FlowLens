//! HTTP/2 frame parser using the hpack crate for header decompression.

use std::collections::HashMap;

use hpack::Decoder;

const FRAME_DATA: u8 = 0x00;
const FRAME_HEADERS: u8 = 0x01;
const FRAME_RST_STREAM: u8 = 0x03;

#[derive(Debug, Clone, PartialEq)]
pub struct Http2FrameInfo {
    pub stream_id: u32,
    pub is_request: bool,
    pub method: Option<String>,
    pub path: Option<String>,
    pub status: Option<String>,
    pub headers: Vec<(String, String)>,
    pub data_length: usize,
}

pub struct Http2Parser {
    decoder: Decoder<'static>,
    stream_headers: HashMap<u32, Vec<(String, String)>>,
    stream_methods: HashMap<u32, String>,
    stream_paths: HashMap<u32, String>,
    stream_body_len: HashMap<u32, usize>,
}

impl Http2Parser {
    pub fn new() -> Self {
        Self {
            decoder: Decoder::new(),
            stream_headers: HashMap::new(),
            stream_methods: HashMap::new(),
            stream_paths: HashMap::new(),
            stream_body_len: HashMap::new(),
        }
    }

    pub fn parse_frames(&mut self, data: &[u8]) -> Vec<Http2FrameInfo> {
        let mut frames = Vec::new();
        let mut offset = 0;
        while offset + 9 <= data.len() {
            let length =
                u32::from_be_bytes([0, data[offset], data[offset + 1], data[offset + 2]]) as usize;
            let frame_type = data[offset + 3];
            let flags = data[offset + 4];
            let stream_id = u32::from_be_bytes([
                data[offset + 5] & 0x7f,
                data[offset + 6],
                data[offset + 7],
                data[offset + 8],
            ]);
            if offset + 9 + length > data.len() {
                break;
            }
            let payload = &data[offset + 9..offset + 9 + length];
            match frame_type {
                FRAME_DATA => {
                    if let Some(info) = self.handle_data_frame(stream_id, payload) {
                        frames.push(info);
                    }
                }
                FRAME_HEADERS => {
                    if let Some(info) = self.handle_headers_frame(stream_id, payload, flags) {
                        frames.push(info);
                    }
                }
                FRAME_RST_STREAM => {
                    self.stream_headers.remove(&stream_id);
                    self.stream_methods.remove(&stream_id);
                    self.stream_paths.remove(&stream_id);
                    self.stream_body_len.remove(&stream_id);
                }
                _ => {}
            }
            offset += 9 + length;
        }
        frames
    }

    fn handle_data_frame(&mut self, stream_id: u32, payload: &[u8]) -> Option<Http2FrameInfo> {
        *self.stream_body_len.entry(stream_id).or_insert(0) += payload.len();
        self.build_frame_info(stream_id)
    }
    fn handle_headers_frame(
        &mut self,
        stream_id: u32,
        payload: &[u8],
        flags: u8,
    ) -> Option<Http2FrameInfo> {
        let mut body = payload;
        if flags & 0x08 != 0 && !payload.is_empty() {
            let pad_len = payload[0] as usize;
            if pad_len < payload.len() {
                body = &payload[1..payload.len() - pad_len];
            } else {
                return None;
            }
        }
        if flags & 0x20 != 0 {
            if body.len() < 5 {
                return None;
            }
            body = &body[5..];
        }
        let headers = self.decoder.decode(body).ok()?;
        for (name_bytes, value_bytes) in headers {
            let name = String::from_utf8_lossy(&name_bytes).to_lowercase();
            let value = String::from_utf8_lossy(&value_bytes).to_string();
            match name.as_str() {
                ":method" => {
                    self.stream_methods.insert(stream_id, value);
                }
                ":path" => {
                    self.stream_paths.insert(stream_id, value);
                }
                ":status" => {
                    self.stream_headers
                        .entry(stream_id)
                        .or_default()
                        .push(("status".to_string(), value));
                }
                _ => {
                    self.stream_headers
                        .entry(stream_id)
                        .or_default()
                        .push((name, value));
                }
            }
        }
        self.build_frame_info(stream_id)
    }

    fn build_frame_info(&self, stream_id: u32) -> Option<Http2FrameInfo> {
        let body_len = *self.stream_body_len.get(&stream_id).unwrap_or(&0);
        if stream_id % 2 == 1 {
            let method = self.stream_methods.get(&stream_id)?.clone();
            let path = self.stream_paths.get(&stream_id)?.clone();
            Some(Http2FrameInfo {
                stream_id,
                is_request: true,
                method: Some(method),
                path: Some(path),
                status: None,
                headers: self
                    .stream_headers
                    .get(&stream_id)
                    .cloned()
                    .unwrap_or_default(),
                data_length: body_len,
            })
        } else {
            let headers = self.stream_headers.get(&stream_id)?;
            let status = headers.iter().find(|(k, _)| k == "status")?.1.clone();
            Some(Http2FrameInfo {
                stream_id,
                is_request: false,
                method: None,
                path: self.stream_paths.get(&stream_id).cloned(),
                status: Some(status),
                headers: headers.clone(),
                data_length: body_len,
            })
        }
    }

    pub fn flush(&mut self) -> Vec<Http2FrameInfo> {
        let mut ids: Vec<u32> = self.stream_headers.keys().copied().collect();
        ids.extend(self.stream_methods.keys().copied());
        ids.extend(self.stream_paths.keys().copied());
        ids.extend(self.stream_body_len.keys().copied());
        ids.sort_unstable();
        ids.dedup();

        let mut frames = Vec::new();
        for stream_id in ids {
            let method = self.stream_methods.remove(&stream_id);
            let path = self.stream_paths.remove(&stream_id);
            let headers = self.stream_headers.remove(&stream_id).unwrap_or_default();
            let data_length = self.stream_body_len.remove(&stream_id).unwrap_or(0);
            let status = headers
                .iter()
                .find(|(k, _)| k == "status")
                .map(|(_, v)| v.clone());
            frames.push(Http2FrameInfo {
                stream_id,
                is_request: stream_id % 2 == 1,
                method,
                path,
                status,
                headers,
                data_length,
            });
        }
        frames
    }
}

impl Default for Http2Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an HTTP/2 frame: 9-byte header + payload.
    fn frame(frame_type: u8, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
        let len = payload.len();
        let mut out = Vec::with_capacity(9 + len);
        out.push((len >> 16) as u8);
        out.push((len >> 8) as u8);
        out.push(len as u8);
        out.push(frame_type);
        out.push(flags);
        out.push((stream_id >> 24) as u8);
        out.push((stream_id >> 16) as u8);
        out.push((stream_id >> 8) as u8);
        out.push(stream_id as u8);
        out.extend_from_slice(payload);
        out
    }

    /// HPACK literal header field without indexing (new name).
    fn hpack_literal(name: &str, value: &str) -> Vec<u8> {
        let mut out = Vec::new();
        // 0x00 = literal header field without indexing, new name
        out.push(0x00);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
        out.push(value.len() as u8);
        out.extend_from_slice(value.as_bytes());
        out
    }

    #[test]
    fn test_empty_input_yields_no_frames() {
        let mut parser = Http2Parser::new();
        assert!(parser.parse_frames(&[]).is_empty());
    }

    #[test]
    fn test_incomplete_frame_is_ignored() {
        let mut parser = Http2Parser::new();
        // Header claims 100 bytes of payload but none are present.
        let data = vec![0, 0, 100, FRAME_HEADERS, 0, 0, 0, 0, 1];
        assert!(parser.parse_frames(&data).is_empty());
    }

    #[test]
    fn test_settings_frame_is_ignored() {
        let mut parser = Http2Parser::new();
        let data = frame(0x04, 0x00, 0, &[]);
        assert!(parser.parse_frames(&data).is_empty());
    }

    #[test]
    fn test_request_headers_are_decoded() {
        let mut parser = Http2Parser::new();
        let mut payload = hpack_literal(":method", "GET");
        payload.extend_from_slice(&hpack_literal(":path", "/api/users"));
        payload.extend_from_slice(&hpack_literal("host", "example.com"));

        let data = frame(FRAME_HEADERS, 0x04, 1, &payload);
        let frames = parser.parse_frames(&data);

        assert_eq!(frames.len(), 1);
        let f = &frames[0];
        assert_eq!(f.stream_id, 1);
        assert!(f.is_request);
        assert_eq!(f.method.as_deref(), Some("GET"));
        assert_eq!(f.path.as_deref(), Some("/api/users"));
        assert!(f.headers.iter().any(|(k, v)| k == "host" && v == "example.com"));
    }

    #[test]
    fn test_response_headers_are_decoded() {
        let mut parser = Http2Parser::new();
        let payload = hpack_literal(":status", "200");
        let data = frame(FRAME_HEADERS, 0x04, 2, &payload);
        let frames = parser.parse_frames(&data);

        assert_eq!(frames.len(), 1);
        assert!(!frames[0].is_request);
        assert_eq!(frames[0].status.as_deref(), Some("200"));
    }

    #[test]
    fn test_data_frame_accumulates_body_length() {
        let mut parser = Http2Parser::new();
        // Send a request HEADERS frame first so the stream is known.
        let mut payload = hpack_literal(":method", "POST");
        payload.extend_from_slice(&hpack_literal(":path", "/upload"));
        parser.parse_frames(&frame(FRAME_HEADERS, 0x04, 1, &payload));

        // Then a DATA frame with 5 bytes of body.
        let data = frame(FRAME_DATA, 0x00, 1, b"hello");
        let frames = parser.parse_frames(&data);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data_length, 5);
        assert_eq!(frames[0].method.as_deref(), Some("POST"));
    }

    #[test]
    fn test_rst_stream_clears_state() {
        let mut parser = Http2Parser::new();
        let mut payload = hpack_literal(":method", "GET");
        payload.extend_from_slice(&hpack_literal(":path", "/gone"));
        parser.parse_frames(&frame(FRAME_HEADERS, 0x04, 1, &payload));

        parser.parse_frames(&frame(FRAME_RST_STREAM, 0x00, 1, &[0, 0, 0, 0, 8]));

        // After RST_STREAM the stream state is gone, so flush yields nothing.
        assert!(parser.flush().is_empty());
    }

    #[test]
    fn test_flush_returns_pending_streams() {
        let mut parser = Http2Parser::new();
        let mut payload = hpack_literal(":method", "GET");
        payload.extend_from_slice(&hpack_literal(":path", "/pending"));
        parser.parse_frames(&frame(FRAME_HEADERS, 0x04, 1, &payload));

        let flushed = parser.flush();
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].path.as_deref(), Some("/pending"));
        // Flushing twice must not duplicate entries.
        assert!(parser.flush().is_empty());
    }
}
