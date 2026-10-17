//! SQLite storage engine for FlowLens
//!
//! This module provides:
//! - Session persistence
//! - Query and filtering
//! - Index management

use flowlens_core::models::{SessionId, TrafficSession};

/// Storage engine trait
pub trait StorageEngine: Send + Sync {
    fn create_tables(&self) -> anyhow::Result<()>;
    fn insert_session(&self, session: &TrafficSession) -> anyhow::Result<SessionId>;
    fn get_session(&self, id: SessionId) -> anyhow::Result<Option<TrafficSession>>;
    fn list_sessions(&self) -> anyhow::Result<Vec<TrafficSession>>;
    fn delete_sessions(&self, before: i64) -> anyhow::Result<u64>;
}

/// In-memory storage engine for Phase 1 MVP
pub struct InMemoryStorage {
    sessions: std::sync::RwLock<std::collections::HashMap<SessionId, TrafficSession>>,
    next_id: std::sync::RwLock<u64>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::RwLock::new(std::collections::HashMap::new()),
            next_id: std::sync::RwLock::new(0),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self { Self::new() }
}

impl StorageEngine for InMemoryStorage {
    fn create_tables(&self) -> anyhow::Result<()> {
        tracing::info!("In-memory storage initialized");
        Ok(())
    }

    fn insert_session(&self, session: &TrafficSession) -> anyhow::Result<SessionId> {
        let mut sessions = self.sessions.write().unwrap();
        let mut next = self.next_id.write().unwrap();
        *next += 1;
        let id = SessionId(*next);
        let mut session = session.clone();
        session.session_id = id;
        sessions.insert(id, session);
        Ok(id)
    }

    fn get_session(&self, id: SessionId) -> anyhow::Result<Option<TrafficSession>> {
        let sessions = self.sessions.read().unwrap();
        Ok(sessions.get(&id).cloned())
    }

    fn list_sessions(&self) -> anyhow::Result<Vec<TrafficSession>> {
        let sessions = self.sessions.read().unwrap();
        Ok(sessions.values().cloned().collect())
    }

    fn delete_sessions(&self, before: i64) -> anyhow::Result<u64> {
        let mut sessions = self.sessions.write().unwrap();
        let to_delete: Vec<SessionId> = sessions
            .iter()
            .filter(|(_, s)| s.started_at < before)
            .map(|(id, _)| *id)
            .collect();
        let count = to_delete.len() as u64;
        for id in to_delete {
            sessions.remove(&id);
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_core::models::{ConnectionKey, ProcessInfo, ProcessArch, MatchQuality, TransportProtocol};

    #[test]
    fn test_insert_and_get() {
        let storage = InMemoryStorage::new();
        storage.create_tables().unwrap();
        let session = TrafficSession {
            session_id: SessionId(0),
            conn: ConnectionKey {
                src_addr: "127.0.0.1:12345".parse().unwrap(),
                dst_addr: "8.8.8.8:53".parse().unwrap(),
                protocol: TransportProtocol::Udp,
                connection_id: 1,
                timestamp: 0,
                interface_guid: None,
            },
            process: ProcessInfo {
                pid: 1234, name: "test.exe".to_string(), exe_path: None,
                user_sid: String::new(), arch: ProcessArch::X64,
                match_quality: MatchQuality::Heuristic,
            },
            tls: None,
            protocol: flowlens_core::models::ProtocolKind::Http1,
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
        let id = storage.insert_session(&session).unwrap();
        let got = storage.get_session(id).unwrap().unwrap();
        assert_eq!(got.session_id, id);
    }
}