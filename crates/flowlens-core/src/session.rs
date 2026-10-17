use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::errors::CoreError;
use crate::models::{ConnectionKey, SessionId, TrafficEvent, TrafficSession};

/// Session manager trait
pub trait SessionManager: Send + Sync + 'static {
    /// Create a new session
    fn create_session(&self, conn: ConnectionKey) -> SessionId;
    
    /// Update an existing session with an event
    fn update_session(&self, id: SessionId, event: TrafficEvent) -> Result<(), CoreError>;
    
    /// Get a session by ID
    fn get_session(&self, id: SessionId) -> Option<TrafficSession>;
    
    /// Persist a session (e.g., to storage)
    fn persist(&self, id: SessionId) -> Result<(), CoreError>;
    
    /// List all sessions
    fn list_sessions(&self) -> Vec<TrafficSession>;
}

/// In-memory session manager for Phase 1 MVP
pub struct InMemorySessionManager {
    sessions: Arc<Mutex<HashMap<SessionId, TrafficSession>>>,
    next_id: Arc<Mutex<u64>>,
}

impl InMemorySessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
        }
    }
}

impl Default for InMemorySessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager for InMemorySessionManager {
    fn create_session(&self, conn: ConnectionKey) -> SessionId {
        let mut id = self.next_id.lock().unwrap();
        *id += 1;
        let session_id = SessionId(*id);
        
        let session = TrafficSession {
            session_id,
            conn,
            process: crate::models::ProcessInfo {
                pid: 0,
                name: String::new(),
                exe_path: None,
                user_sid: String::new(),
                arch: crate::models::ProcessArch::Unknown,
                match_quality: crate::models::MatchQuality::Unspecified,
            },
            tls: None,
            protocol: crate::models::ProtocolKind::Unknown,
            http_request: None,
            http_response: None,
            websocket_frames: Vec::new(),
            sse_events: Vec::new(),
            grpc_call: None,
            started_at: 0,
            ended_at: None,
            bytes_sent: 0,
            bytes_received: 0,
        };
        
        self.sessions.lock().unwrap().insert(session_id, session);
        session_id
    }
    
    fn update_session(&self, id: SessionId, event: TrafficEvent) -> Result<(), CoreError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&id) {
            match &event {
                TrafficEvent::ConnectionEstablished { conn, process } => {
                    session.conn = conn.clone();
                    session.process = process.clone();
                }
                TrafficEvent::TlsHandshake { session: tls_session, .. } => {
                    session.tls = Some(tls_session.clone());
                    session.protocol = match tls_session.alpn.as_deref() {
                        Some("h2") => crate::models::ProtocolKind::Http2,
                        Some("http/1.1") => crate::models::ProtocolKind::Http1,
                        _ => crate::models::ProtocolKind::Unknown,
                    };
                }
                TrafficEvent::HttpRequest { request, .. } => {
                    session.http_request = Some(request.clone());
                }
                TrafficEvent::HttpResponse { response, .. } => {
                    session.http_response = Some(response.clone());
                }
                TrafficEvent::WebSocketFrame { frame, .. } => {
                    session.websocket_frames.push(frame.clone());
                }
                TrafficEvent::SseEvent { event, .. } => {
                    session.sse_events.push(event.clone());
                }
                TrafficEvent::GrpcCall { call, .. } => {
                    session.grpc_call = Some(call.clone());
                }
                TrafficEvent::ConnectionClosed { .. } => {
                    use crate::models::now_micros;
                    session.ended_at = Some(now_micros());
                }
                _ => {}
            }
            Ok(())
        } else {
            Err(CoreError::SessionNotFound(id))
        }
    }
    
    fn get_session(&self, id: SessionId) -> Option<TrafficSession> {
        self.sessions.lock().unwrap().get(&id).cloned()
    }
    
    fn persist(&self, _id: SessionId) -> Result<(), CoreError> {
        // Phase 1: in-memory only
        Ok(())
    }
    
    fn list_sessions(&self) -> Vec<TrafficSession> {
        self.sessions.lock().unwrap().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_session() {
        let mgr = InMemorySessionManager::new();
        let conn = crate::models::ConnectionKey {
            src_addr: "127.0.0.1:12345".parse().unwrap(),
            dst_addr: "8.8.8.8:53".parse().unwrap(),
            protocol: crate::models::TransportProtocol::Udp,
            connection_id: 1,
            timestamp: 0,
            interface_guid: None,
        };
        
        let id = mgr.create_session(conn.clone());
        let session = mgr.get_session(id).unwrap();
        assert_eq!(session.conn, conn);
    }
}