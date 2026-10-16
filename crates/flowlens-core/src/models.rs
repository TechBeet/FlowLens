//! Core domain models for FlowLens

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Connection key - uniquely identifies a network connection
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConnectionKey {
    pub src_addr: SocketAddr,
    pub dst_addr: SocketAddr,
    pub protocol: TransportProtocol,
    pub connection_id: u64,
    pub timestamp: i64,
    pub interface_guid: Option<String>,
}

/// Transport protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

/// Process architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessArch {
    X86,
    X64,
    Arm64,
    Unknown,
}

/// Attribution quality level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchQuality {
    ExactTuple,
    Heuristic,
    Unspecified,
}

/// Capture source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureSource {
    /// Windows Filtering Platform (WFP) kernel-mode capture
    Wfp,
    /// Npcap user-mode capture
    Npcap,
    /// Loopback traffic capture
    Loopback,
    /// Mock/test capture source
    Mock,
}

/// TLS version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsVersion {
    Tls10,
    Tls11,
    Tls12,
    Tls13,
}

/// HTTP version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpVersion {
    Http10,
    Http11,
    Http2,
    Http3,
    Unknown,
}

/// WebSocket opcode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WsOpcode {
    Text,
    Binary,
    Close,
    Ping,
    Pong,
    Continuation,
}

/// Protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolKind {
    Http1,
    Http2,
    WebSocket,
    Sse,
    Grpc,
    Unknown,
}

/// Session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// Process attribution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<PathBuf>,
    pub user_sid: String,
    pub arch: ProcessArch,
    pub match_quality: MatchQuality,
}

/// TLS session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsSession {
    pub tls_version: TlsVersion,
    pub cipher_suite: String,
    pub server_name: Option<String>,
    pub alpn: Option<String>,
    pub client_hello: Option<Vec<u8>>,
    pub server_hello: Option<Vec<u8>>,
    pub master_secret: Option<String>,
}

/// HTTP request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub path: String,
    pub request_line_version: HttpVersion,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub body_size: usize,
    pub timestamp: i64,
}

/// HTTP response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub version: HttpVersion,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub body_size: usize,
    pub timestamp: i64,
}

/// WebSocket frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketFrame {
    pub opcode: WsOpcode,
    pub payload: Vec<u8>,
    pub payload_size: usize,
    pub is_masked: bool,
    pub timestamp: i64,
}

/// Server-Sent Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub id: Option<String>,
    pub data: String,
    pub retry_ms: Option<u64>,
    pub timestamp: i64,
}

/// gRPC call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcCall {
    pub service: String,
    pub method: String,
    pub request_frames: Vec<Vec<u8>>,
    pub response_frames: Vec<Vec<u8>>,
    pub status: Option<u32>,
    pub is_streaming: bool,
    pub timestamp: i64,
}

/// Traffic session - complete representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSession {
    pub session_id: SessionId,
    pub conn: ConnectionKey,
    pub process: ProcessInfo,
    pub tls: Option<TlsSession>,
    pub protocol: ProtocolKind,
    pub http_request: Option<HttpRequest>,
    pub http_response: Option<HttpResponse>,
    pub websocket_frames: Vec<WebSocketFrame>,
    pub sse_events: Vec<SseEvent>,
    pub grpc_call: Option<GrpcCall>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Network traffic event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficEvent {
    ConnectionEstablished { conn: ConnectionKey, process: ProcessInfo },
    PacketReceived { data: Vec<u8>, interface: String, direction: String, timestamp: i64 },
    TlsHandshake { conn: ConnectionKey, session: TlsSession },
    HttpRequest { session_id: SessionId, conn: ConnectionKey, request: HttpRequest },
    HttpResponse { session_id: SessionId, conn: ConnectionKey, response: HttpResponse },
    WebSocketFrame { session_id: SessionId, conn: ConnectionKey, frame: WebSocketFrame },
    SseEvent { session_id: SessionId, conn: ConnectionKey, event: SseEvent },
    GrpcCall { session_id: SessionId, conn: ConnectionKey, call: GrpcCall },
    ConnectionClosed { conn: ConnectionKey },
    CaptureDropped { reason: String, count: u64 },
}

/// Traffic filter for event subscription
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficFilter {
    pub pids: Option<Vec<u32>>,
    pub hosts: Option<Vec<String>>,
    pub protocols: Option<Vec<ProtocolKind>>,
    pub methods: Option<Vec<String>>,
    pub status_codes: Option<Vec<u16>>,
    pub time_start: Option<i64>,
    pub time_end: Option<i64>,
    pub match_quality: Option<Vec<MatchQuality>>,
}

impl TrafficFilter {
    /// Returns true if the event should be delivered to this subscriber.
    /// Every variant is explicitly matched; nothing is accidentally forwarded.
    pub fn matches(&self, event: &TrafficEvent) -> bool {
        match event {
            TrafficEvent::ConnectionEstablished { conn, process } => {
                self.match_connection(conn) && self.match_process(process)
            }
            TrafficEvent::PacketReceived { .. } => self.match_packet(event),
            TrafficEvent::TlsHandshake { conn, .. } => self.match_connection(conn),
            TrafficEvent::HttpRequest { conn, request, .. } => {
                self.match_connection(conn) && self.match_request(request)
            }
            TrafficEvent::HttpResponse { conn, response, .. } => {
                self.match_connection(conn) && self.match_response(response)
            }
            TrafficEvent::WebSocketFrame { conn, frame, .. } => {
                self.match_connection(conn) && self.match_websocket(frame)
            }
            TrafficEvent::SseEvent { conn, .. } => self.match_connection(conn),
            TrafficEvent::GrpcCall { conn, call, .. } => {
                self.match_connection(conn) && self.match_grpc(call)
            }
            TrafficEvent::ConnectionClosed { conn } => self.match_connection(conn),
            TrafficEvent::CaptureDropped { .. } => true,
        }
    }

    fn match_packet(&self, event: &TrafficEvent) -> bool {
        // Packets are routed by the engine before connection attribution;
        // apply only host filter when present so filters aren't silently dropped.
        match event {
            TrafficEvent::PacketReceived { data, .. } => {
                if let Some(hosts) = &self.hosts {
                    if let Some((_, ip)) = parse_ip_from_packet(data) {
                        return hosts.iter().any(|h| ip.to_string().contains(h.as_str()));
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn match_websocket(&self, frame: &WebSocketFrame) -> bool {
        if let Some(methods) = &self.methods {
            let op = format!("{:?}", frame.opcode).to_lowercase();
            if !methods.iter().any(|m| op.contains(&m.to_lowercase())) {
                return false;
            }
        }
        true
    }

    fn match_grpc(&self, call: &GrpcCall) -> bool {
        if let Some(methods) = &self.methods {
            let full = format!("/{}/{}", call.service, call.method);
            if !methods.iter().any(|m| full.contains(m.as_str())) {
                return false;
            }
        }
        true
    }

    fn match_connection(&self, conn: &ConnectionKey) -> bool {
        if let Some(ref hosts) = self.hosts {
            let conn_host = conn.dst_addr.to_string();
            if !hosts.iter().any(|h| {
                // Allow exact match or subdomain match
                *h == conn_host || 
                h.starts_with(&format!(".{}", conn_host)) ||
                conn_host.ends_with(&format!(".{}", h))
            }) {
                return false;
            }
        }
        true
    }

    fn match_process(&self, process: &ProcessInfo) -> bool {
        if let Some(ref pids) = self.pids {
            if !pids.contains(&process.pid) {
                return false;
            }
        }
        true
    }

    fn match_request(&self, request: &HttpRequest) -> bool {
        if let Some(ref methods) = self.methods {
            if !methods.contains(&request.method) {
                return false;
            }
        }
        true
    }

    fn match_response(&self, response: &HttpResponse) -> bool {
        if let Some(ref codes) = self.status_codes {
            if !codes.contains(&response.status_code) {
                return false;
            }
        }
        true
    }
}

/// Extract (is_ipv4, IpAddr) from raw packet bytes when the packet carries
/// an Ethernet II header (14 bytes) with an inner IPv4 header.
/// Returns None if the packet is too short, uses IPv6, or lacks a valid IP header.
pub fn parse_ip_from_packet(data: &[u8]) -> Option<(bool, std::net::IpAddr)> {
    let off = if data.len() >= 14 && data[12] != 0x08 && data[13] != 0x00 {
        // Non-IPv4 Ethertype — skip Ethernet header check; still try direct offset
        0
    } else {
        14
    };
    if data.len() < off + 20 {
        return None;
    }
    let ihl = ((data[off] & 0x0F) as usize) * 4;
    if ihl < 20 || off + ihl > data.len() {
        return None;
    }
    let ver = data[off] >> 4;
    match ver {
        4 => {
            if off + ihl + 4 <= data.len() {
                let ip = std::net::IpAddr::from([
                    data[off + ihl],
                    data[off + ihl + 1],
                    data[off + ihl + 2],
                    data[off + ihl + 3],
                ]);
                Some((true, ip))
            } else {
                None
            }
        }
        6 => {
            // IPv6: first 16 bytes after header are the src address
            if off + ihl + 16 <= data.len() {
                let mut addr = [0u8; 16];
                addr.copy_from_slice(&data[off + ihl..off + ihl + 16]);
                Some((false, std::net::IpAddr::from(addr)))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Get current timestamp in microseconds
pub fn now_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

/// Display row for traffic events (used by TUI and GUI)
#[derive(Debug, Clone)]
pub struct TrafficRow {
    pub timestamp: i64,
    pub event_type: String,
    pub src_addr: String,
    pub dst_addr: String,
    pub method: String,
    pub path: String,
    pub status: String,
    pub duration_ms: u64,
    pub body_size: usize,
}

impl TrafficRow {
    pub fn from_event(event: &TrafficEvent) -> Option<Self> {
        let ts = match event {
            TrafficEvent::ConnectionEstablished { conn, .. }
            | TrafficEvent::TlsHandshake { conn, .. }
            | TrafficEvent::HttpRequest { conn, .. }
            | TrafficEvent::HttpResponse { conn, .. }
            | TrafficEvent::WebSocketFrame { conn, .. }
            | TrafficEvent::SseEvent { conn, .. }
            | TrafficEvent::GrpcCall { conn, .. } => conn.timestamp,
            TrafficEvent::ConnectionClosed { conn } => conn.timestamp,
            TrafficEvent::CaptureDropped { .. } => 0,
            TrafficEvent::PacketReceived { timestamp, .. } => *timestamp,
        };
        let (event_type, src, dst, method, path, status, body_size) = match event {
            TrafficEvent::ConnectionEstablished { conn, process } => (
                "connection".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                format!("{} (PID {})", process.name, process.pid),
                String::new(),
                String::new(),
                0,
            ),
            TrafficEvent::TlsHandshake { conn, session } => (
                "tls".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                format!("{:?}", session.tls_version),
                String::new(),
                String::new(),
                0,
            ),
            TrafficEvent::HttpRequest { conn, request, .. } => (
                "http_request".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                request.method.clone(),
                request.url.clone(),
                String::new(),
                request.body_size,
            ),
            TrafficEvent::HttpResponse { conn, response, .. } => (
                "http_response".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                String::new(),
                conn.dst_addr.to_string(),
                format!("{} {}", response.status_code, response.status_text),
                response.body_size,
            ),
            TrafficEvent::WebSocketFrame { conn, frame, .. } => (
                "websocket".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                format!("{:?}", frame.opcode),
                String::new(),
                format!("{} bytes", frame.payload_size),
                frame.payload_size,
            ),
            TrafficEvent::SseEvent { conn, event, .. } => (
                "sse".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                String::new(),
                format!("{} bytes", event.data.len()),
                String::new(),
                event.data.len(),
            ),
            TrafficEvent::GrpcCall { conn, call, .. } => (
                "grpc".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                format!("{}/{}", call.service, call.method),
                String::new(),
                String::new(),
                0,
            ),
            TrafficEvent::ConnectionClosed { conn } => (
                "closed".to_string(),
                conn.src_addr.to_string(),
                conn.dst_addr.to_string(),
                String::new(),
                String::new(),
                String::new(),
                0,
            ),
            TrafficEvent::CaptureDropped { reason, count } => (
                "dropped".to_string(),
                "-".to_string(),
                "-".to_string(),
                String::new(),
                format!("{} ({})", reason, count),
                String::new(),
                0,
            ),
            TrafficEvent::PacketReceived { data, interface, timestamp, .. } => {
                let _ = timestamp;
                (
                    "packet".to_string(),
                    interface.clone(),
                    format!("{} bytes", data.len()),
                    String::new(),
                    String::new(),
                    String::new(),
                    data.len(),
                )
            }
        };
        Some(Self {
            timestamp: ts,
            event_type,
            src_addr: src,
            dst_addr: dst,
            method,
            path,
            status,
            duration_ms: 0,
            body_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_matches_pid() {
        let filter = TrafficFilter {
            pids: Some(vec![1234]),
            ..Default::default()
        };
        let process = ProcessInfo {
            pid: 1234,
            name: "test.exe".to_string(),
            exe_path: None,
            user_sid: "S-1-5-21-123".to_string(),
            arch: ProcessArch::X64,
            match_quality: MatchQuality::ExactTuple,
        };
        assert!(filter.match_process(&process));
    }

    #[test]
    fn test_filter_matches_host() {
        let filter = TrafficFilter {
            hosts: Some(vec!["142.250.80.46:443".to_string()]),
            ..Default::default()
        };
        let conn = ConnectionKey {
            src_addr: "127.0.0.1:12345".parse().unwrap(),
            dst_addr: "142.250.80.46:443".parse().unwrap(),
            protocol: TransportProtocol::Tcp,
            connection_id: 1,
            timestamp: 0,
            interface_guid: None,
        };
        assert!(filter.match_connection(&conn));
    }

    #[test]
    fn test_filter_rejects_non_matching_host() {
        let filter = TrafficFilter {
            hosts: Some(vec!["142.250.80.46:443".to_string()]),
            ..Default::default()
        };
        let conn = ConnectionKey {
            src_addr: "127.0.0.1:12345".parse().unwrap(),
            dst_addr: "8.8.8.8:53".parse().unwrap(),
            protocol: TransportProtocol::Tcp,
            connection_id: 1,
            timestamp: 0,
            interface_guid: None,
        };
        assert!(!filter.match_connection(&conn));
    }
}

