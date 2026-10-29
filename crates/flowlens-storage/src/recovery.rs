//! Crash recovery manager for FlowLens.
//!
//! Provides `RecoveryManager` which checkpoints session data to a JSON-line
//! write-ahead log, enabling recovery after unexpected crashes.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use flowlens_core::models::TrafficSession;

/// A single checkpoint entry written to the WAL.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointEntry {
    /// Monotonically increasing checkpoint sequence number.
    seq: u64,
    /// The serialized session payload (JSON).
    payload: String,
}

/// Recovery manager for storage backends.
///
/// Wraps a storage engine and maintains a JSON-line WAL that enables replay
/// after a crash. The WAL is appended to on every checkpoint and truncated
/// after a successful recovery.
pub struct RecoveryManager<S> {
    storage: S,
    wal_path: PathBuf,
}

impl<S> RecoveryManager<S>
where
    S: crate::StorageEngine,
{
    /// Create a new recovery manager wrapping `storage`.
    ///
    /// The WAL lives next to the database file with a `.wal` extension.
    pub fn new(storage: S, db_path: &Path) -> Self {
        let wal_path = db_path.with_extension("wal");
        Self { storage, wal_path }
    }

    /// Append a session to the WAL.
    pub fn checkpoint_session(&self, seq: u64, session: &TrafficSession) -> anyhow::Result<()> {
        let payload = serde_json::to_string(session)?;
        let entry = CheckpointEntry { seq, payload };
        let line = serde_json::to_string(&entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)?;
        writeln!(file, "{}", line)?;
        file.flush()?;
        Ok(())
    }

    /// Replay the WAL and merge the entries into the storage.
    ///
    /// Returns the number of sessions recovered. The WAL is truncated after a
    /// successful replay so the next run starts from a clean slate.
    pub fn recover(&self) -> anyhow::Result<usize> {
        if !self.wal_path.exists() {
            return Ok(0);
        }
        let file = File::open(&self.wal_path)?;
        let reader = BufReader::new(file);
        let mut count = 0usize;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: CheckpointEntry = serde_json::from_str(&line)?;
            let session: TrafficSession = serde_json::from_str(&entry.payload)?;
            self.storage.insert_session(&session)?;
            count += 1;
        }
        fs::write(&self.wal_path, "")?;
        Ok(count)
    }

    /// Backup all stored sessions to a newline-delimited JSON snapshot.
    ///
    /// Returns the number of sessions written.
    pub fn backup_to<P: AsRef<Path>>(&self, target: P) -> anyhow::Result<u64> {
        let sessions = self.storage.list_sessions()?;
        let mut out = BufWriter::new(File::create(target.as_ref())?);
        let mut count = 0u64;
        for session in &sessions {
            serde_json::to_writer(&mut out, session)?;
            writeln!(out)?;
            count += 1;
        }
        out.flush()?;
        Ok(count)
    }

    /// Clear the WAL without replaying it.
    pub fn truncate_wal(&self) -> anyhow::Result<()> {
        fs::write(&self.wal_path, "")?;
        Ok(())
    }

    /// Current WAL size in bytes.
    pub fn wal_size(&self) -> anyhow::Result<u64> {
        if !self.wal_path.exists() {
            return Ok(0);
        }
        Ok(fs::metadata(&self.wal_path)?.len())
    }

    /// Reference to the wrapped storage.
    pub fn storage(&self) -> &S {
        &self.storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageEngine;
    use flowlens_core::models::{
        ConnectionKey, MatchQuality, ProcessArch, ProcessInfo, ProtocolKind, TransportProtocol,
    };

    fn mk_session(id: u64, ts: i64) -> TrafficSession {
        TrafficSession {
            session_id: flowlens_core::models::SessionId(id),
            conn: ConnectionKey {
                src_addr: "127.0.0.1:54321".parse().unwrap(),
                dst_addr: "8.8.8.8:443".parse().unwrap(),
                protocol: TransportProtocol::Tcp,
                connection_id: id,
                timestamp: ts,
                interface_guid: None,
            },
            process: ProcessInfo {
                pid: 1234,
                name: format!("test{}", id),
                exe_path: None,
                user_sid: String::new(),
                arch: ProcessArch::X64,
                match_quality: MatchQuality::Heuristic,
            },
            tls: None,
            protocol: ProtocolKind::Http1,
            http_request: None,
            http_response: None,
            websocket_frames: vec![],
            sse_events: vec![],
            grpc_call: None,
            started_at: ts,
            ended_at: None,
            bytes_sent: 100u64 + id as u64,
            bytes_received: 200u64 + id as u64,
        }
    }

    #[test]
    fn test_checkpoint_and_recover() {
        let dir = tempdir::TempDir::new("recovery_test").unwrap();
        let db = dir.path().join("test.db");
        let store = crate::InMemoryStorage::new();
        let mgr = RecoveryManager::new(store, &db);

        let s1 = mk_session(1, 1000);
        let s2 = mk_session(2, 2000);
        mgr.checkpoint_session(1, &s1).unwrap();
        mgr.checkpoint_session(2, &s2).unwrap();

        assert!(mgr.wal_size().unwrap() > 0);

        let count = mgr.recover().unwrap();
        assert_eq!(count, 2);
        assert_eq!(mgr.wal_size().unwrap(), 0);

        let sessions = mgr.storage().list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_recover_empty_wal() {
        let dir = tempdir::TempDir::new("recovery_empty").unwrap();
        let db = dir.path().join("test.db");
        let store = crate::InMemoryStorage::new();
        let mgr = RecoveryManager::new(store, &db);

        let count = mgr.recover().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_backup_roundtrip() {
        let dir = tempdir::TempDir::new("recovery_backup").unwrap();
        let db = dir.path().join("test.db");
        let snap = dir.path().join("snapshot.jsonl");
        let store = crate::InMemoryStorage::new();
        let mgr = RecoveryManager::new(store, &db);

        for id in 0..3 {
            let s = mk_session(id, (id as i64) * 1000);
            mgr.storage().insert_session(&s).unwrap();
        }

        let count = mgr.backup_to(&snap).unwrap();
        assert_eq!(count, 3);
        assert!(snap.exists());
    }

    #[test]
    fn test_truncate_wal() {
        let dir = tempdir::TempDir::new("recovery_trunc").unwrap();
        let db = dir.path().join("test.db");
        let store = crate::InMemoryStorage::new();
        let mgr = RecoveryManager::new(store, &db);

        let session = mk_session(1, 1000);
        mgr.checkpoint_session(1, &session).unwrap();
        assert!(mgr.wal_size().unwrap() > 0);

        mgr.truncate_wal().unwrap();
        assert_eq!(mgr.wal_size().unwrap(), 0);
    }
}
