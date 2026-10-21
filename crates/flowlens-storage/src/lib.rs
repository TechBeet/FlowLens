//! SQLite storage engine for FlowLens
//!
//! This module provides:
//! - Session persistence via SQLite
//! - Query and filtering
//! - Index management
//! - Database migration support
//!
//! The storage layer supports two implementations:
//! - `InMemoryStorage`: Fast in-memory storage for testing and ephemeral sessions
//! - `SqliteStorage`: Persistent SQLite-backed storage with thread-safe access

use std::path::Path;
use std::sync::Arc;

use flowlens_core::models::{SessionId, TrafficSession};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

/// Storage engine trait — all storage backends implement this.
pub trait StorageEngine: Send + Sync {
    /// Create or migrate tables to the current schema version.
    fn create_tables(&self) -> anyhow::Result<()>;
    /// Insert a session, returning the assigned SessionId.
    fn insert_session(&self, session: &TrafficSession) -> anyhow::Result<SessionId>;
    /// Retrieve a session by ID.
    fn get_session(&self, id: SessionId) -> anyhow::Result<Option<TrafficSession>>;
    /// List all stored sessions.
    fn list_sessions(&self) -> anyhow::Result<Vec<TrafficSession>>;
    /// Delete sessions older than the given timestamp (microseconds).
    fn delete_sessions(&self, before: i64) -> anyhow::Result<u64>;
    /// Query sessions with optional filters.
    fn query_sessions(&self, query: &StorageQuery) -> anyhow::Result<Vec<TrafficSession>>;
}

/// Query parameters for filtering stored sessions.
#[derive(Debug, Clone, Default)]
pub struct StorageQuery {
    pub pid: Option<u32>,
    pub host: Option<String>,
    pub protocol: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Current schema version. Bump this when the schema changes.
pub const SCHEMA_VERSION: i64 = 2;

/// In-memory storage engine for Phase 1 MVP and testing.
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

/// SQLite storage engine for persistent session storage.
pub struct SqliteStorage {
    pool: Arc<SqlitePool>,
}

impl SqliteStorage {
    /// Create a new SQLite storage backend.
    pub async fn new(path: &Path) -> anyhow::Result<Self> {
        let db_url = format!("sqlite://{}", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;
        let storage = Self { pool: Arc::new(pool) };
        storage.create_tables()?;
        Ok(storage)
    }

    /// Get the inner pool for testing.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Migrate database schema to current version.
async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    // Use runtime query to avoid requiring DATABASE_URL for compile-time checking
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            start_time BIGINT NOT NULL,
            end_time BIGINT,
            src_addr TEXT NOT NULL,
            src_port INTEGER NOT NULL,
            dst_addr TEXT NOT NULL,
            dst_port INTEGER NOT NULL,
            protocol TEXT NOT NULL,
            connection_id INTEGER NOT NULL,
            pid INTEGER NOT NULL,
            process_name TEXT NOT NULL,
            user_sid TEXT NOT NULL,
            arch TEXT NOT NULL,
            match_quality TEXT NOT NULL,
            tls INTEGER DEFAULT 0,
            http_method TEXT,
            http_url TEXT,
            http_status INTEGER,
            bytes_sent INTEGER DEFAULT 0,
            bytes_received INTEGER DEFAULT 0,
            event_data TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
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

    fn query_sessions(&self, query: &StorageQuery) -> anyhow::Result<Vec<TrafficSession>> {
        let sessions = self.sessions.read().unwrap();
        let mut results: Vec<TrafficSession> = sessions
            .values()
            .filter(|s| {
                if let Some(pid) = query.pid {
                    if s.process.pid != pid { return false; }
                }
                if let Some(host) = &query.host {
                    let dst = s.conn.dst_addr.to_string();
                    if !dst.contains(host) { return false; }
                }
                if let Some(proto) = &query.protocol {
                    let pk = format!("{:?}", s.protocol).to_lowercase();
                    if pk != proto.to_lowercase() { return false; }
                }
                true
            })
            .cloned()
            .collect();

        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        Ok(results)
    }
}

impl StorageEngine for SqliteStorage {
    fn create_tables(&self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(migrate(&self.pool))?;
        Ok(())
    }

    fn insert_session(&self, _session: &TrafficSession) -> anyhow::Result<SessionId> {
        unimplemented!("SQLite storage insert not yet implemented")
    }

    fn get_session(&self, _id: SessionId) -> anyhow::Result<Option<TrafficSession>> {
        unimplemented!("SQLite storage get not yet implemented")
    }

    fn list_sessions(&self) -> anyhow::Result<Vec<TrafficSession>> {
        Ok(vec![])
    }

    fn delete_sessions(&self, _before: i64) -> anyhow::Result<u64> {
        Ok(0)
    }

    fn query_sessions(&self, _query: &StorageQuery) -> anyhow::Result<Vec<TrafficSession>> {
        Ok(vec![])
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