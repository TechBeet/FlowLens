//! MCP tool definitions for FlowLens
//!
//! This module provides tool implementations for querying and analyzing
//! captured network traffic data.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use flowlens_core::models::SessionId;
use flowlens_storage::StorageEngine;

/// Trait for MCP tools
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Tool name (used in method calls)
    fn name(&self) -> &str;
    
    /// Tool description
    fn description(&self) -> &str;
    
    /// Input schema for the tool (JSON Schema format)
    fn input_schema(&self) -> serde_json::Value;
    
    /// Invoke the tool with arguments
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

/// Query sessions tool - retrieves traffic sessions with optional pagination
pub struct QuerySessionsTool {
    storage: Arc<dyn StorageEngine>,
}

impl QuerySessionsTool {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl McpTool for QuerySessionsTool {
    fn name(&self) -> &str {
        "query_sessions"
    }

    fn description(&self) -> &str {
        "Query captured traffic sessions with optional limit. Returns a list of sessions with their metadata."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of sessions to return (default: 50)"
                }
            },
            "additionalProperties": false
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
        let sessions = self.storage.list_sessions()?;
        let limited: Vec<_> = sessions.into_iter().take(limit as usize).collect();
        Ok(serde_json::json!({
            "sessions": limited,
            "count": limited.len(),
        }))
    }
}

/// Get session tool - retrieves a specific session by ID
pub struct GetSessionTool {
    storage: Arc<dyn StorageEngine>,
}

impl GetSessionTool {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl McpTool for GetSessionTool {
    fn name(&self) -> &str {
        "get_session"
    }

    fn description(&self) -> &str {
        "Get a specific traffic session by its ID. Returns full session details including requests, responses, and process info."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["session_id"],
            "properties": {
                "session_id": {
                    "type": "integer",
                    "description": "The unique identifier of the session"
                }
            },
            "additionalProperties": false
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id: u64 = args.get("session_id").and_then(|v| v.as_u64()).ok_or_else(|| {
            anyhow::anyhow!("Missing required parameter: session_id")
        })?;
        
        let session = self.storage.get_session(SessionId(id))?;
        match session {
            Some(s) => Ok(serde_json::json!({ "session": s })),
            None => Err(anyhow::anyhow!("Session not found: {}", id)),
        }
    }
}

/// Aggregate by host tool - groups sessions by destination host
pub struct AggregateByHostTool {
    storage: Arc<dyn StorageEngine>,
}

impl AggregateByHostTool {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl McpTool for AggregateByHostTool {
    fn name(&self) -> &str {
        "aggregate_by_host"
    }

    fn description(&self) -> &str {
        "Aggregate traffic statistics by destination host. Returns session count and total bytes transferred per host."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let sessions = self.storage.list_sessions()?;
        let mut host_stats: HashMap<String, HostStats> = HashMap::new();
        
        for session in sessions {
            let host = session.conn.dst_addr.to_string();
            let entry = host_stats.entry(host).or_insert(HostStats {
                session_count: 0,
                bytes_sent: 0,
                bytes_received: 0,
            });
            entry.session_count += 1;
            entry.bytes_sent += session.bytes_sent;
            entry.bytes_received += session.bytes_received;
        }
        
        let result: Vec<serde_json::Value> = host_stats
            .into_iter()
            .map(|(host, stats)| {
                serde_json::json!({
                    "host": host,
                    "session_count": stats.session_count,
                    "bytes_sent": stats.bytes_sent,
                    "bytes_received": stats.bytes_received,
                    "total_bytes": stats.bytes_sent + stats.bytes_received,
                })
            })
            .collect();
        
        Ok(serde_json::json!({ "aggregations": result }))
    }
}

struct HostStats {
    session_count: u64,
    bytes_sent: u64,
    bytes_received: u64,
}

/// Aggregate by PID tool - groups sessions by process ID
pub struct AggregateByPidTool {
    storage: Arc<dyn StorageEngine>,
}

impl AggregateByPidTool {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl McpTool for AggregateByPidTool {
    fn name(&self) -> &str {
        "aggregate_by_pid"
    }

    fn description(&self) -> &str {
        "Aggregate traffic statistics by process ID. Returns session count and total bytes transferred per process."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let sessions = self.storage.list_sessions()?;
        let mut pid_stats: HashMap<u32, PidStats> = HashMap::new();
        
        for session in sessions {
            let pid = session.process.pid;
            let entry = pid_stats.entry(pid).or_insert(PidStats {
                process_name: session.process.name.clone(),
                session_count: 0,
                bytes_sent: 0,
                bytes_received: 0,
            });
            entry.session_count += 1;
            entry.bytes_sent += session.bytes_sent;
            entry.bytes_received += session.bytes_received;
        }
        
        let result: Vec<serde_json::Value> = pid_stats
            .into_iter()
            .map(|(pid, stats)| {
                serde_json::json!({
                    "pid": pid,
                    "process_name": stats.process_name,
                    "session_count": stats.session_count,
                    "bytes_sent": stats.bytes_sent,
                    "bytes_received": stats.bytes_received,
                    "total_bytes": stats.bytes_sent + stats.bytes_received,
                })
            })
            .collect();
        
        Ok(serde_json::json!({ "aggregations": result }))
    }
}

struct PidStats {
    process_name: String,
    session_count: u64,
    bytes_sent: u64,
    bytes_received: u64,
}
