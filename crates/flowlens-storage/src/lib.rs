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
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

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

/// A flattened row of session data, ready for insertion into the `sessions` table.
struct SessionRow {
    start_time: i64,
    end_time: Option<i64>,
    src_addr: String,
    src_port: i64,
    dst_addr: String,
    dst_port: i64,
    protocol: String,
    connection_id: i64,
    pid: i64,
    name: String,
    user_sid: String,
    arch: String,
    match_quality: String,
    tls: i64,
    http_method: Option<String>,
    http_url: Option<String>,
    http_status: Option<i64>,
    bytes_sent: i64,
    bytes_received: i64,
    /// The complete session serialized as JSON so nested structures
    /// (headers, bodies, TLS info, WS frames, SSE events) round-trip exactly.
    event_data: String,
}

/// Flatten a session into its database columns.
fn session_to_row(session: &TrafficSession) -> SessionRow {
    SessionRow {
        start_time: session.started_at,
        end_time: session.ended_at,
        src_addr: session.conn.src_addr.ip().to_string(),
        src_port: session.conn.src_addr.port() as i64,
        dst_addr: session.conn.dst_addr.ip().to_string(),
        dst_port: session.conn.dst_addr.port() as i64,
        protocol: format!("{:?}", session.conn.protocol),
        connection_id: session.conn.connection_id as i64,
        pid: session.process.pid as i64,
        name: session.process.name.clone(),
        user_sid: session.process.user_sid.clone(),
        arch: format!("{:?}", session.process.arch),
        match_quality: format!("{:?}", session.process.match_quality),
        tls: if session.tls.is_some() { 1 } else { 0 },
        http_method: session.http_request.as_ref().map(|r| r.method.clone()),
        http_url: session.http_request.as_ref().map(|r| r.url.clone()),
        http_status: session.http_response.as_ref().map(|r| r.status_code as i64),
        bytes_sent: session.bytes_sent as i64,
        bytes_received: session.bytes_received as i64,
        event_data: serde_json::to_string(session).unwrap_or_else(|_| "{}".to_string()),
    }
}

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
    fn default() -> Self {
        Self::new()
    }
}

/// SQLite storage engine for persistent session storage.
pub struct SqliteStorage {
    pool: Arc<SqlitePool>,
    /// Dedicated runtime used to drive the async SQLx calls from the
    /// synchronous `StorageEngine` trait methods. Kept separate so that
    /// calling those methods from a non-async context never nests runtimes.
    rt: Arc<tokio::runtime::Runtime>,
}

impl SqliteStorage {
    /// Create a new SQLite storage backend.
    pub async fn new(path: &Path) -> anyhow::Result<Self> {
        // Ensure the parent directory exists so SQLite can create the file.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Use `SqliteConnectOptions` rather than a URL string: SQLite URLs
        // mishandle Windows drive letters (the `C:` gets parsed as a host).
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        
        // Create a dedicated current-thread runtime for sync wrapper methods.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let storage = Self {
            pool: Arc::new(pool),
            rt: Arc::new(rt),
        };
        // Migrate directly on the caller's runtime: calling the synchronous
        // `create_tables()` here would `block_on` inside an active runtime.
        migrate(&storage.pool).await?;
        Ok(storage)
    }

    /// Get the inner pool for testing.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get the dedicated runtime for testing.
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.rt
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
                    if s.process.pid != pid {
                        return false;
                    }
                }
                if let Some(host) = &query.host {
                    let dst = s.conn.dst_addr.to_string();
                    if !dst.contains(host) {
                        return false;
                    }
                }
                if let Some(proto) = &query.protocol {
                    let pk = format!("{:?}", s.protocol).to_lowercase();
                    if pk != proto.to_lowercase() {
                        return false;
                    }
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
        self.rt.block_on(migrate(&self.pool))?;
        Ok(())
    }

    fn insert_session(&self, session: &TrafficSession) -> anyhow::Result<SessionId> {
        let row = session_to_row(session);

        let id = self.rt.block_on(async {
            let mut conn = self.pool.acquire().await?;
            sqlx::query(
                "INSERT INTO sessions \
                 (start_time, end_time, src_addr, src_port, dst_addr, dst_port, \
                  protocol, connection_id, pid, process_name, user_sid, arch, \
                  match_quality, tls, http_method, http_url, http_status, \
                  bytes_sent, bytes_received, event_data) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.start_time)
            .bind(row.end_time)
            .bind(row.src_addr)
            .bind(row.src_port)
            .bind(row.dst_addr)
            .bind(row.dst_port)
            .bind(row.protocol)
            .bind(row.connection_id)
            .bind(row.pid)
            .bind(row.name)
            .bind(row.user_sid)
            .bind(row.arch)
            .bind(row.match_quality)
            .bind(row.tls)
            .bind(row.http_method)
            .bind(row.http_url)
            .bind(row.http_status)
            .bind(row.bytes_sent)
            .bind(row.bytes_received)
            .bind(row.event_data)
            .execute(&mut *conn)
            .await?;

            sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
                .fetch_one(&mut *conn)
                .await
        })?;

        Ok(SessionId(id as u64))
    }

    fn get_session(&self, id: SessionId) -> anyhow::Result<Option<TrafficSession>> {
        let data = self.rt.block_on(async {
            sqlx::query_scalar::<_, String>("SELECT event_data FROM sessions WHERE id = ?")
                .bind(id.0 as i64)
                 .fetch_optional(self.pool.as_ref())
                .await
        })?;

        let mut result = data.and_then(|d| serde_json::from_str::<TrafficSession>(&d).ok());
        if let Some(s) = &mut result {
            s.session_id = id;
        }
        Ok(result)
    }

    fn list_sessions(&self) -> anyhow::Result<Vec<TrafficSession>> {
        let rows: Vec<TrafficSession> = self.rt.block_on(async {
            sqlx::query("SELECT id, event_data FROM sessions ORDER BY id")
                .fetch_all(self.pool.as_ref())
                .await
                .map(|rs| {
                    rs.into_iter()
                        .filter_map(|row| -> Option<TrafficSession> {
                            let id: i64 = row.try_get("id").ok()?;
                            let data: String = row.try_get("event_data").ok()?;
                            let mut s: TrafficSession = serde_json::from_str(&data).ok()?;
                            s.session_id = SessionId(id as u64);
                            Some(s)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });

        Ok(rows)
    }

    fn delete_sessions(&self, before: i64) -> anyhow::Result<u64> {
        let count = self.rt.block_on(async {
            sqlx::query("DELETE FROM sessions WHERE start_time < ?")
                .bind(before)
                .execute(self.pool.as_ref())
                .await
                .map(|r| r.rows_affected() as u64)
                .unwrap_or(0)
        });

        Ok(count)
    }

    fn query_sessions(&self, query: &StorageQuery) -> anyhow::Result<Vec<TrafficSession>> {
        let rows = self.list_sessions()?;

        let mut results: Vec<TrafficSession> = rows
            .iter()
            .filter(|s| {
                if let Some(pid) = query.pid {
                    if s.process.pid != pid {
                        return false;
                    }
                }
                if let Some(host) = &query.host {
                    let dst = s.conn.dst_addr.to_string();
                    if !dst.contains(host) {
                        return false;
                    }
                }
                if let Some(proto) = &query.protocol {
                    let pk = format!("{:?}", s.protocol).to_lowercase();
                    if pk != proto.to_lowercase() {
                        return false;
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_core::models::{
        ConnectionKey, MatchQuality, ProcessArch, ProcessInfo, TransportProtocol,
    };

    /// Build a sample session for tests.
    fn sample_session() -> TrafficSession {
        TrafficSession {
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
                pid: 1234,
                name: "test.exe".to_string(),
                exe_path: None,
                user_sid: String::new(),
                arch: ProcessArch::X64,
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
        }
    }

    /// Create a temporary SQLite database for testing.
    ///
    /// The storage engine owns a dedicated runtime, so these tests must run
    /// outside of any ambient Tokio runtime (i.e. plain `#[test]`).
    fn temp_sqlite() -> (SqliteStorage, tempdir::TempDir) {
        let dir = tempdir::TempDir::new("flowlens_storage_test").unwrap();
        let path = dir.path().join("test.db");
        let storage = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(SqliteStorage::new(&path))
            .unwrap();
        (storage, dir)
    }

    #[test]
    fn test_sqlite_insert_and_get() {
        let (storage, _dir) = temp_sqlite();
        let session = sample_session();
        let id = storage.insert_session(&session).unwrap();
        assert!(id.0 > 0);

        let got = storage.get_session(id).unwrap().expect("session must exist");
        assert_eq!(got.session_id, id);
        assert_eq!(got.process.pid, 1234);
        assert_eq!(got.process.name, "test.exe");
        assert_eq!(got.conn.connection_id, 1);
    }

    #[test]
    fn test_sqlite_get_missing_returns_none() {
        let (storage, _dir) = temp_sqlite();
        assert!(storage.get_session(SessionId(9999)).unwrap().is_none());
    }

    #[test]
    fn test_sqlite_list_sessions() {
        let (storage, _dir) = temp_sqlite();
        storage.insert_session(&sample_session()).unwrap();
        storage.insert_session(&sample_session()).unwrap();

        let all = storage.list_sessions().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_sqlite_delete_sessions() {
        let (storage, _dir) = temp_sqlite();
        let mut old = sample_session();
        old.started_at = 100;
        let mut new = sample_session();
        new.started_at = 10_000;

        storage.insert_session(&old).unwrap();
        storage.insert_session(&new).unwrap();

        // Delete everything older than 1000.
        let deleted = storage.delete_sessions(1_000).unwrap();
        assert_eq!(deleted, 1);

        let remaining = storage.list_sessions().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].started_at, 10_000);
    }

    #[test]
    fn test_sqlite_query_by_pid() {
        let (storage, _dir) = temp_sqlite();
        let mut a = sample_session();
        a.process.pid = 100;
        let mut b = sample_session();
        b.process.pid = 200;

        storage.insert_session(&a).unwrap();
        storage.insert_session(&b).unwrap();

        let q = StorageQuery {
            pid: Some(200),
            ..Default::default()
        };
        let found = storage.query_sessions(&q).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].process.pid, 200);
    }

    #[test]
    fn test_sqlite_query_limit_and_offset() {
        let (storage, _dir) = temp_sqlite();
        for _ in 0..5 {
            storage.insert_session(&sample_session()).unwrap();
        }

        let q = StorageQuery {
            limit: Some(2),
            offset: Some(1),
            ..Default::default()
        };
        let found = storage.query_sessions(&q).unwrap();
        assert_eq!(found.len(), 2);
    }

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
                pid: 1234,
                name: "test.exe".to_string(),
                exe_path: None,
                user_sid: String::new(),
                arch: ProcessArch::X64,
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
