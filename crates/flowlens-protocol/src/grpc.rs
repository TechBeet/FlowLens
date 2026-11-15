//! gRPC frame parser for FlowLens
//!
//! gRPC transmits messages over HTTP/2 using a 5-byte prefix followed by
//! the message payload:
//! ```text
//!  +-------+-----------------------+
//!  | Flag  | Length (24-bit, BE)   |
//!  +-------+-----------------------+
//!  | 0x00  | 0x00 0x01 0x02        |  <- 5-byte header
//!  +-------+-----------------------+
//!  | Message Body ...            |
//!  +-----------------------------+
//! ```

/// A single gRPC message frame.
#[derive(Debug, Clone, PartialEq)]
pub struct GrpcFrame {
    /// Whether the message is compressed (flag byte bit 0).
    pub compressed: bool,
    /// Length of the message payload in bytes.
    pub length: u32,
    /// The raw message payload.
    pub payload: Vec<u8>,
}

/// gRPC frame parser with incremental buffering.
pub struct GrpcParser {
    buffer: Vec<u8>,
    service_path: Option<String>,
}

impl GrpcParser {
    const HEADER_SIZE: usize = 5;

    /// Create a new gRPC parser.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            service_path: None,
        }
    }

    /// Set the gRPC path from the HTTP/2 `:path` pseudo-header.
    /// Expected format: `/fully.qualified.Service/MethodName`
    pub fn set_path(&mut self, path: &str) {
        self.service_path = Some(path.to_string());
    }

    /// Parse a buffer of gRPC message data.
    ///
    /// Returns all complete frames that could be assembled. Any trailing
    /// bytes that do not form a complete frame are retained internally.
    pub fn parse(&mut self, data: &[u8]) -> Vec<GrpcFrame> {
        self.buffer.extend_from_slice(data);
        let mut frames = Vec::new();

        loop {
            if self.buffer.len() < Self::HEADER_SIZE {
                break; // Need more data.
            }

            let compressed = self.buffer[0] & 0x01 != 0;
            let length = u32::from_be_bytes([
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
                self.buffer[4],
            ]) as usize;

            let total = Self::HEADER_SIZE + length;
            if self.buffer.len() < total {
                break; // Incomplete message.
            }

            let payload = self.buffer[Self::HEADER_SIZE..total].to_vec();
            self.buffer.drain(0..total);

            frames.push(GrpcFrame {
                compressed,
                length: length as u32,
                payload,
            });
        }

        frames
    }

    /// Return the qualified RPC name parsed from the path.
    ///
    /// For `/pkg.Service/Method`, returns `pkg.Service/Method`.
    pub fn rpc_name(&self) -> Option<String> {
        self.service_path
            .as_ref()
            .map(|p| p.strip_prefix('/').unwrap_or(p).to_string())
    }

    /// Return the service portion of the path (`pkg.Service`).
    pub fn service_name(&self) -> Option<String> {
        self.rpc_name()
            .and_then(|n| n.split_once('/').map(|(svc, _)| svc.to_string()))
    }

    /// Return the method portion of the path (`Method`).
    pub fn method_name(&self) -> Option<String> {
        self.rpc_name()
            .and_then(|n| n.split_once('/').map(|(_, m)| m.to_string()))
    }

    /// Reset the internal buffer.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}

impl Default for GrpcParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a gRPC message: 1 flag byte + 4-byte BE length + payload.
    fn grpc_message(compressed: bool, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(5 + payload.len());
        out.push(if compressed { 0x01 } else { 0x00 });
        let len = payload.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn test_parse_single_frame() {
        let mut parser = GrpcParser::new();
        let data = grpc_message(false, b"hello");
        let frames = parser.parse(&data);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].length, 5);
        assert_eq!(frames[0].payload, b"hello");
        assert!(!frames[0].compressed);
    }

    #[test]
    fn test_parse_multiple_frames() {
        let mut parser = GrpcParser::new();
        let mut data = grpc_message(false, b"one");
        data.extend_from_slice(&grpc_message(false, b"two"));
        data.extend_from_slice(&grpc_message(false, b"three"));

        let frames = parser.parse(&data);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].payload, b"one");
        assert_eq!(frames[1].payload, b"two");
        assert_eq!(frames[2].payload, b"three");
    }

    #[test]
    fn test_incomplete_frame_is_buffered() {
        let mut parser = GrpcParser::new();
        let full = grpc_message(false, b"hello world");

        // Feed only part of the message.
        let frames = parser.parse(&full[..6]);
        assert!(frames.is_empty());

        // Feed the remainder; the frame must now complete.
        let frames = parser.parse(&full[6..]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, b"hello world");
    }

    #[test]
    fn test_parse_compressed_flag() {
        let mut parser = GrpcParser::new();
        let data = grpc_message(true, b"zipped");
        let frames = parser.parse(&data);
        assert_eq!(frames.len(), 1);
        assert!(frames[0].compressed);
        assert_eq!(frames[0].payload, b"zipped");
    }

    #[test]
    fn test_empty_input() {
        let mut parser = GrpcParser::new();
        assert!(parser.parse(&[]).is_empty());
    }

    #[test]
    fn test_parse_path_service_method() {
        let mut parser = GrpcParser::new();
        parser.set_path("/flowlens.Greeter/SayHello");
        assert_eq!(parser.rpc_name().as_deref(), Some("flowlens.Greeter/SayHello"));
        assert_eq!(parser.service_name().as_deref(), Some("flowlens.Greeter"));
        assert_eq!(parser.method_name().as_deref(), Some("SayHello"));
    }

    #[test]
    fn test_reset_clears_buffer() {
        let mut parser = GrpcParser::new();
        let full = grpc_message(false, b"hello");
        parser.parse(&full[..6]);
        parser.reset();
        // After reset the parser needs the full message again.
        let frames = parser.parse(&full[6..]);
        assert!(frames.is_empty());
    }
}
