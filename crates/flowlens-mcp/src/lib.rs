//! MCP (Model Context Protocol) server for FlowLens

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use flowlens_core::models::SessionId;
use flowlens_storage::StorageEngine;

/// MCP server trait
pub trait McpServer: Send + Sync {
    fn start(&mut self, bind_addr: SocketAddr) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}

/// MCP tool trait
#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

/// Query sessions tool
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
    fn name(&self) -> &'static str {
        "query_sessions"
    }
    fn description(&self) -> &'static str {
        "Query captured traffic sessions with filters"
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

/// Get session tool
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
    fn name(&self) -> &'static str {
        "get_session"
    }
    fn description(&self) -> &'static str {
        "Get a specific traffic session by ID"
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id: u64 = args.get("session_id").and_then(|v| v.as_u64()).unwrap_or(0);
        let session = self.storage.get_session(SessionId(id))?;
        Ok(serde_json::json!({
            "session": session,
        }))
    }
}

/// MCP server implementation
pub struct McpServerImpl {
    tools: Vec<Box<dyn McpTool>>,
    addr: Option<SocketAddr>,
}

impl McpServerImpl {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            addr: None,
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn McpTool>) {
        self.tools.push(tool);
    }
}

impl Default for McpServerImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer for McpServerImpl {
    fn start(&mut self, bind_addr: SocketAddr) -> anyhow::Result<()> {
        self.addr = Some(bind_addr);
        tracing::info!("MCP server starting on {}", bind_addr);
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("MCP server stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_storage::InMemoryStorage;

    #[tokio::test]
    async fn test_query_sessions_tool() {
        let storage = Arc::new(InMemoryStorage::new());
        let tool = QuerySessionsTool::new(storage);
        let result = tool.invoke(serde_json::json!({"limit": 10})).await.unwrap();
        assert!(result.get("sessions").is_some());
    }
}
