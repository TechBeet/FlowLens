//! Session store trait and implementations (ADR 0010 §9)
//!
//! Provides session persistence with pagination support:
//! - `SessionStore` trait (ADR 0010 §9.1)
//! - `SessionDetail` - full session representation (ADR 0010 §9.3)
//! - `Query` / `QueryResult` - pagination and filtering

use crate::models::{ConnectionKey, GrpcCall, HttpRequest, HttpResponse, ProcessInfo, SseEvent, TlsSession, TrafficEvent, WebSocketFrame};
use serde::{Deserialize, Serialize};

/// Store-level errors (ADR 0010 §8.2).
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum StoreError {
    #[error("Invalid limit: {0} (must be 1..=500)")]
    InvalidLimit(u32),
    #[error("Session not found: {0}")]
    SessionNotFound(u64),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Query parameters for filtering sessions (ADR 0010 §9.1).
#[derive(Debug, Clone, Default)]
pub struct Query {
    pub pid: Option<u32>,
    pub host: Option<String>,
    pub protocol: Option<String>,
    /// Clamped to [1, 500] (ADR 0010 §9.4).
    pub limit: u32,
    /// Row offset (ADR 0010 §9.4).
    pub offset: u64,
}

impl Query {
    /// Validate and clamp the limit into [1, 500].
    pub fn clamped_limit(&self) -> u32 {
        self.limit.clamp(1, 500)
    }
}

/// Lightweight summary row returned by [`SessionStore::query`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: u64,
    pub protocol: String,
    pub src_addr: String,
    pub dst_addr: String,
    pub host: String,
    pub pid: u32,
    pub process_name: String,
    pub method: String,
    pub url: String,
    pub status_code: Option<u16>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub match_quality: String,
}

/// Paginated query result (ADR 0010 §9.1).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryResult {
    pub sessions: Vec<SessionSummary>,
    pub total: u64,
}

/// Full session detail (ADR 0010 §9.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub id: u64,
    pub conn: ConnectionKey,
    pub process: ProcessInfo,
    pub tls: Option<TlsSession>,
    pub http_request: Option<HttpRequest>,
    pub http_response: Option<HttpResponse>,
    pub websocket_frames: Vec<WebSocketFrame>,
    pub sse_events: Vec<SseEvent>,
    pub grpc_calls: Vec<GrpcCall>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Core store abstraction (ADR 0010 §9.1).
pub trait SessionStore: Send + Sync + 'static {
    fn append(&self, event: TrafficEvent) -> Result<(), StoreError>;
    fn get(&self, id: u64) -> Result<Option<SessionDetail>, StoreError>;
    fn query(&self, q: Query) -> Result<QueryResult, StoreError>;
    fn delete(&self, id: u64) -> Result<(), StoreError>;
    fn clear(&self) -> Result<(), StoreError>;
}
/// In-memory ring-buffer store (ADR 0010 §9.2 RingStore).
pub struct RingStore {
    entries: std::sync::Arc<std::sync::RwLock<std::collections::VecDeque<SessionDetail>>>,
    capacity: usize,
}

impl RingStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::sync::Arc::new(std::sync::RwLock::new(std::collections::VecDeque::with_capacity(capacity.min(1024)))),
            capacity: capacity.max(1),
        }
    }
    pub fn len(&self) -> usize { self.entries.read().map(|g| g.len()).unwrap_or(0) }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    fn with_entries<R>(&self, f: impl FnOnce(&std::collections::VecDeque<SessionDetail>) -> R) -> Result<R, StoreError> {
        let guard = self.entries.read().map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(f(&guard))
    }
}

impl Default for RingStore {
    fn default() -> Self { Self::new(50_000) }
}

impl SessionStore for RingStore {
    fn append(&self, event: TrafficEvent) -> Result<(), StoreError> {
        let mut guard = self.entries.write().map_err(|e| StoreError::Serialization(e.to_string()))?;
        match event {
            TrafficEvent::ConnectionEstablished { conn, process } => {
                let id = (guard.len() as u64) + 1;
                let detail = SessionDetail {
                    id, conn, process, tls: None, http_request: None, http_response: None,
                    websocket_frames: Vec::new(), sse_events: Vec::new(), grpc_calls: Vec::new(),
                    started_at: crate::models::now_micros(), ended_at: None, bytes_sent: 0, bytes_received: 0,
                };
                guard.push_back(detail);
                while guard.len() > self.capacity { guard.pop_front(); }
            }
            TrafficEvent::TlsHandshake { conn, session } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.conn == conn) { entry.tls = Some(session); }
            }
            TrafficEvent::HttpRequest { session_id, request, .. } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.id == session_id.0) { entry.http_request = Some(request); }
            }
            TrafficEvent::HttpResponse { session_id, response, .. } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.id == session_id.0) { entry.http_response = Some(response); }
            }
            TrafficEvent::WebSocketFrame { session_id, frame, .. } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.id == session_id.0) { entry.websocket_frames.push(frame); }
            }
            TrafficEvent::SseEvent { session_id, event, .. } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.id == session_id.0) { entry.sse_events.push(event); }
            }
            TrafficEvent::GrpcCall { session_id, call, .. } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.id == session_id.0) { entry.grpc_calls.push(call); }
            }
            TrafficEvent::ConnectionClosed { conn } => {
                if let Some(entry) = guard.iter_mut().find(|e| e.conn == conn) { entry.ended_at = Some(crate::models::now_micros()); }
            }
            _ => {}
        }
        Ok(())
    }

    fn get(&self, id: u64) -> Result<Option<SessionDetail>, StoreError> {
        self.with_entries(|entries| entries.iter().find(|e| e.id == id).cloned())
    }

    fn query(&self, q: Query) -> Result<QueryResult, StoreError> {
        if q.limit == 0 || q.limit > 500 {
            return Err(StoreError::InvalidLimit(q.limit));
        }
        let limit = q.limit.clamp(1, 500) as usize;
        let offset = q.offset as usize;
        self.with_entries(|entries| {
            let mut matched: Vec<&SessionDetail> = entries.iter().filter(|d| {
                if let Some(pid) = q.pid { if d.process.pid != pid { return false; } }
                if let Some(ref host) = q.host {
                    let hl = host.to_ascii_lowercase();
                    if !d.conn.src_addr.to_string().to_ascii_lowercase().contains(&hl)
                        && !d.conn.dst_addr.to_string().to_ascii_lowercase().contains(&hl) {
                        return false;
                    }
                }
                if let Some(ref proto) = q.protocol {
                    let p = proto.to_ascii_lowercase();
                    let ok = match d.conn.protocol {
                        crate::models::TransportProtocol::Tcp => p == "tcp" || p == "all",
                        crate::models::TransportProtocol::Udp => p == "udp" || p == "all",
                    };
                    if !ok { return false; }
                }
                true
            }).collect();
            matched.sort_by(|a, b| b.started_at.cmp(&a.started_at));
            let total = matched.len() as u64;
            let page: Vec<SessionSummary> = matched.into_iter().skip(offset).take(limit).map(|d| {
                let host = d.tls.as_ref().and_then(|t| t.server_name.clone())
                    .or_else(|| d.http_request.as_ref().and_then(|r| url_host(&r.url)))
                    .unwrap_or_else(|| d.conn.dst_addr.ip().to_string());
                SessionSummary {
                    id: d.id, protocol: format!("{:?}", d.conn.protocol),
                    src_addr: d.conn.src_addr.to_string(), dst_addr: d.conn.dst_addr.to_string(),
                    host, pid: d.process.pid, process_name: d.process.name.clone(),
                    method: d.http_request.as_ref().map(|r| r.method.clone()).unwrap_or_default(),
                    url: d.http_request.as_ref().map(|r| r.url.clone()).unwrap_or_default(),
                    status_code: d.http_response.as_ref().map(|r| r.status_code),
                    started_at: d.started_at, ended_at: d.ended_at,
                    bytes_sent: d.bytes_sent, bytes_received: d.bytes_received,
                    match_quality: format!("{:?}", d.process.match_quality),
                }
            }).collect();
            QueryResult { sessions: page, total }
        })
    }

    fn delete(&self, id: u64) -> Result<(), StoreError> {
        let mut guard = self.entries.write().map_err(|e| StoreError::Serialization(e.to_string()))?;
        let before = guard.len();
        guard.retain(|e| e.id != id);
        if guard.len() == before { return Err(StoreError::SessionNotFound(id)); }
        Ok(())
    }

    fn clear(&self) -> Result<(), StoreError> {
        let mut guard = self.entries.write().map_err(|e| StoreError::Serialization(e.to_string()))?;
        guard.clear();
        Ok(())
    }
}

fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.split('@').last().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() { None } else { Some(host.to_string()) }
}