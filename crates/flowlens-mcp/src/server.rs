//! MCP JSON-RPC 2.0 server implementation
//!
//! This module provides the core request routing and response handling
//! for the FlowLens MCP server.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, error};

use crate::json_rpc::{JsonRpcRequest, JsonRpcResponse, ErrorCode};
use crate::tools::McpTool;

/// MCP server implementation with TCP line-delimited JSON-RPC transport
pub struct McpServerImpl {
    tools: Vec<Arc<dyn McpTool>>,
    addr: Option<SocketAddr>,
}

impl McpServerImpl {
    pub fn new() -> Self {
        Self { tools: Vec::new(), addr: None }
    }

    pub fn register_tool(&mut self, tool: Arc<dyn McpTool>) {
        self.tools.push(tool);
    }

    pub fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        if req.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                req.id,
                ErrorCode::INVALID_REQUEST,
                "Invalid JSON-RPC version. Must be 2.0",
                None,
            );
        }

        match req.method.as_str() {
            "initialize" => self.handle_initialize(req),
            "tools/list" => self.handle_tools_list(req),
            "tools/call" => self.handle_tools_call(req),
            _ => JsonRpcResponse::error(
                req.id,
                ErrorCode::METHOD_NOT_FOUND,
                format!("Method not found: {}", req.method),
                None,
            ),
        }
    }

    fn handle_initialize(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "flowlens", "version": "0.1.0" }
        });
        JsonRpcResponse::success(req.id, result)
    }

    fn handle_tools_list(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let tools: Vec<_> = self.tools.iter().map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "inputSchema": t.input_schema()
            })
        }).collect();
        JsonRpcResponse::success(req.id, serde_json::json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let params = match &req.params {
            Some(p) => p.clone(),
            None => return JsonRpcResponse::error(req.id, ErrorCode::INVALID_PARAMS, "Missing required params field", None),
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name.to_string(),
            None => return JsonRpcResponse::error(req.id, ErrorCode::INVALID_PARAMS, "Missing required parameter: name", None),
        };

        let arguments = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);

        match self.tools.iter().find(|t| t.name() == tool_name) {
            Some(tool) => {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let result = runtime.block_on(tool.invoke(arguments));
                match result {
                    Ok(v) => JsonRpcResponse::success(req.id, v),
                    Err(e) => JsonRpcResponse::error(req.id, ErrorCode::INTERNAL_ERROR, format!("Tool invocation failed: {}", e), None),
                }
            }
            None => JsonRpcResponse::error(req.id, ErrorCode::METHOD_NOT_FOUND, format!("Tool not found: {}", tool_name), None),
        }
    }

    pub async fn start(&mut self, bind_addr: SocketAddr) -> anyhow::Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        info!("MCP server listening on {}", bind_addr);
        self.addr = Some(bind_addr);

        loop {
            let (socket, addr) = listener.accept().await?;
            info!("New connection from {}", addr);
            let stream = BufReader::new(socket);
            let tools = self.tools.clone();

            tokio::spawn(async move {
                Self::handle_connection(stream, tools).await;
            });
        }
    }

    async fn handle_connection(mut reader: BufReader<tokio::net::TcpStream>, tools: Vec<Arc<dyn McpTool>>) {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let request: Result<JsonRpcRequest, _> = serde_json::from_str(line.trim());
                    let temp_server = McpServerImpl { tools: tools.clone(), addr: None };
                    let response = match request {
                        Ok(req) => temp_server.handle_request(req),
                        Err(e) => JsonRpcResponse::error(None, ErrorCode::PARSE_ERROR, format!("Parse error: {}", e), None),
                    };
                    let response_bytes = format!("{}\n", serde_json::to_string(&response).unwrap());
                    if let Err(e) = reader.get_mut().write_all(response_bytes.as_bytes()).await {
                        error!("Failed to write response: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to read line: {}", e);
                    break;
                }
            }
        }
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.addr = None;
        Ok(())
    }
}

impl Default for McpServerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{AggregateByHostTool, AggregateByPidTool, GetSessionTool, QuerySessionsTool};
    use flowlens_storage::InMemoryStorage;

    fn create_test_server() -> McpServerImpl {
        let storage = Arc::new(InMemoryStorage::new());
        let mut server = McpServerImpl::new();

        server.register_tool(Arc::new(QuerySessionsTool::new(storage.clone())));
        server.register_tool(Arc::new(GetSessionTool::new(storage.clone())));
        server.register_tool(Arc::new(AggregateByHostTool::new(storage.clone())));
        server.register_tool(Arc::new(AggregateByPidTool::new(storage.clone())));

        server
    }

    #[test]
    fn test_initialize() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let resp = server.handle_request(req);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "flowlens");
    }

    #[test]
    fn test_tools_list() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn test_method_not_found() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "unknown_method".to_string(),
            params: None,
        };
        let resp = server.handle_request(req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, ErrorCode::METHOD_NOT_FOUND);
    }

    #[test]
    fn test_invalid_version() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            id: Some(serde_json::json!(4)),
            method: "initialize".to_string(),
            params: None,
        };
        let resp = server.handle_request(req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, ErrorCode::INVALID_REQUEST);
    }

    #[test]
    fn test_parse_error() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(5)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({"name": "query_sessions", "arguments": {"limit": 10}})),
        };
        let resp = server.handle_request(req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_aggregate_by_host() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(6)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({"name": "aggregate_by_host", "arguments": {}})),
        };
        let resp = server.handle_request(req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_get_session_invalid_params() {
        let server = create_test_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(7)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({"name": "get_session"})),
        };
        let resp = server.handle_request(req);
        assert!(resp.error.is_some());
        // Tool returns an error when session_id is missing
        assert!(resp.error.as_ref().unwrap().code == ErrorCode::INVALID_PARAMS 
            || resp.error.as_ref().unwrap().code == ErrorCode::INTERNAL_ERROR);
    }
}