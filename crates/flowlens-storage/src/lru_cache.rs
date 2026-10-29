//! LRU cache implementation for FlowLens session storage.
//!
//! This module provides an LRU cache that wraps an `InMemoryStorage`
//! and enforces a maximum memory budget. When the budget is exceeded,
//! least-recently-used sessions are evicted.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use crate::InMemoryStorage;
use crate::StorageEngine;
use flowlens_core::models::{SessionId, TrafficSession};

/// A thread-safe LRU cache for sessions with a memory budget.
pub struct LruSessionCache {
    storage: Arc<InMemoryStorage>,
    capacity_bytes: usize,
    current_bytes: std::sync::RwLock<usize>,
    sessions: Arc<std::sync::RwLock<HashMap<SessionId, (TrafficSession, SystemTime)>>>,
    lru_order: Arc<std::sync::RwLock<Vec<SessionId>>>,
}
impl LruSessionCache {
    /// Create a new LRU session cache with the given memory budget.
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            storage: Arc::new(InMemoryStorage::new()),
            capacity_bytes,
            current_bytes: std::sync::RwLock::new(0),
            sessions: Arc::new(std::sync::RwLock::new(HashMap::new())),
            lru_order: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    /// Insert a session into the cache.
    pub fn insert(&self, session: &TrafficSession) -> anyhow::Result<SessionId> {
        let id = self.storage.insert_session(session)?;
        self.insert_with_id(session, id)?;
        Ok(id)
    }

    fn insert_with_id(&self, session: &TrafficSession, id: SessionId) -> anyhow::Result<()> {
        let serialized_size = self.estimate_session_size(session);
        
        while *self.current_bytes.read().unwrap() + serialized_size > self.capacity_bytes
            && !self.lru_order.read().unwrap().is_empty()
        {
            self.evict_least_recently_used()?;
        }
        
        {
            let mut sessions = self.sessions.write().unwrap();
            let mut lru_order = self.lru_order.write().unwrap();
            
            sessions.insert(id, (session.clone(), SystemTime::now()));
            lru_order.push(id);
            *self.current_bytes.write().unwrap() += serialized_size;
        }
        
        Ok(())
    }

    /// Get a session from the cache.
    pub fn get(&self, id: SessionId) -> anyhow::Result<Option<TrafficSession>> {
        let mut sessions = self.sessions.write().unwrap();
        let mut lru_order = self.lru_order.write().unwrap();
        
        if let Some((session, _)) = sessions.get(&id) {
            let cloned = session.clone();
            if let Some(pos) = lru_order.iter().position(|&x| x == id) {
                lru_order.remove(pos);
                lru_order.push(id);
            }
            sessions.insert(id, (cloned.clone(), SystemTime::now()));
            Ok(Some(cloned))
        } else {
            Ok(None)
        }
    }

    /// Get the number of sessions in the cache.
    pub fn len(&self) -> usize {
        self.sessions.read().unwrap().len()
    }

    /// Get the total bytes used by the cache.
    pub fn total_bytes(&self) -> usize {
        *self.current_bytes.read().unwrap()
    }

    /// Evict sessions until the cache fits within the memory budget.
    pub fn evict_to_fit(&self) -> anyhow::Result<()> {
        while *self.current_bytes.read().unwrap() > self.capacity_bytes
            && !self.lru_order.read().unwrap().is_empty()
        {
            self.evict_least_recently_used()?;
        }
        Ok(())
    }

    fn evict_least_recently_used(&self) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().unwrap();
        let mut lru_order = self.lru_order.write().unwrap();
        
        if !lru_order.is_empty() {
            let id = lru_order.remove(0);
            if let Some((session, _)) = sessions.remove(&id) {
                let serialized_size = self.estimate_session_size(&session);
                *self.current_bytes.write().unwrap() -= serialized_size;
            }
        }
        Ok(())
    }

    fn estimate_session_size(&self, session: &TrafficSession) -> usize {
        serde_json::to_string(session).unwrap_or_else(|_| "{}".to_string()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_core::models::{ConnectionKey, ProcessInfo, TransportProtocol, ProtocolKind, ProcessArch, MatchQuality};

    fn create_test_session(id: u64, pad: usize) -> TrafficSession {
        TrafficSession {
            session_id: SessionId(id),
            conn: ConnectionKey {
                src_addr: "127.0.0.1:12345".parse().unwrap(),
                dst_addr: "8.8.8.8:53".parse().unwrap(),
                protocol: TransportProtocol::Udp,
                connection_id: 1,
                timestamp: 0,
                interface_guid: None,
            },
            process: ProcessInfo {
                pid: 1234,
                name: format!("test{}{}", id, "x".repeat(pad)),
                exe_path: None,
                user_sid: String::new(),
                arch: ProcessArch::X64,
                match_quality: MatchQuality::Heuristic,
            },
            tls: None,
            protocol: ProtocolKind::Http1,
            http_request: None,
            http_response: None,
            websocket_frames: Vec::new(),
            sse_events: Vec::new(),
            grpc_call: None,
            started_at: 0,
            ended_at: None,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    #[test]
    fn test_lru_eviction_on_overflow() {
        let cache = LruSessionCache::new(1000);
        
        for i in 0..5 {
            let session = create_test_session(i, 200);
            cache.insert(&session).unwrap();
        }
        
        assert!(cache.len() <= 5);
        assert!(cache.total_bytes() <= 1000);
    }

    #[test]
    fn test_lru_ordering() {
        let session1 = create_test_session(1, 10);
        let size = serde_json::to_string(&session1).unwrap().len();
        let cache = LruSessionCache::new(size * 3);
        
        let session2 = create_test_session(2, 10);
        let session3 = create_test_session(3, 10);
        
        cache.insert(&session1).unwrap();
        cache.insert(&session2).unwrap();
        cache.insert(&session3).unwrap();
        
        cache.get(SessionId(2)).unwrap();
        
        let session4 = create_test_session(4, 10);
        cache.insert(&session4).unwrap();
        
        assert!(cache.get(SessionId(1)).unwrap().is_none());
        assert!(cache.get(SessionId(2)).unwrap().is_some());
        assert!(cache.get(SessionId(3)).unwrap().is_some());
        assert!(cache.get(SessionId(4)).unwrap().is_some());
    }

    #[test]
    fn test_total_bytes_tracking() {
        let cache = LruSessionCache::new(10000);
        
        let session1 = create_test_session(1, 10);
        let session2 = create_test_session(2, 20);
        
        cache.insert(&session1).unwrap();
        let b1 = cache.total_bytes();
        cache.insert(&session2).unwrap();
        let b2 = cache.total_bytes();
        
        assert!(b2 > b1);
        assert!(b2 >= b1);
    }

    #[test]
    fn test_evict_to_fit() {
        let cache = LruSessionCache::new(500);
        
        let session1 = create_test_session(1, 200);
        let session2 = create_test_session(2, 200);
        let session3 = create_test_session(3, 200);
        
        cache.insert(&session1).unwrap();
        cache.insert(&session2).unwrap();
        cache.insert(&session3).unwrap();
        
        cache.evict_to_fit().unwrap();
        
        assert!(cache.total_bytes() <= 500);
    }
}
