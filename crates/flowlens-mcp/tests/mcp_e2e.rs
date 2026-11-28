//! End-to-end MCP server integration test.
//!
//! Drives the full JSON-RPC pipeline — `initialize`, `tools/list`, and a
//! `tools/call` for every registered tool — against an in-memory storage seeded
//! with sample sessions. It exercises the real request routing and tool logic
//! without needing a network transport (loopback TCP can be blocked in CI).

use std::sync::Arc;

use flowlens_core::models::{
    ConnectionKey, HttpRequest, HttpResponse, HttpVersion, MatchQuality, ProcessArch, ProcessInfo,
    ProtocolKind, SessionId, TrafficSession, TransportProtocol,
};
use flowlens_mcp::tools::{
    DiagnoseCaptureTool, ExportAsCurlTool, GetCaptureStatusTool, GetSessionStatsTool,
    GetTransactionTool, ListEndpointsTool, ListTransactionsTool, SearchTransactionsTool,
};
use flowlens_mcp::{JsonRpcRequest, McpServerImpl};
use flowlens_storage::{InMemoryStorage, StorageEngine};

fn sample_session(method: &str, url: &str, status: u16) -> TrafficSession {
    TrafficSession {
        session_id: SessionId(0),
        conn: ConnectionKey {
            src_addr: "127.0.0.1:12345".parse().unwrap(),
            dst_addr: "93.184.216.34:443".parse().unwrap(),
            protocol: TransportProtocol::Tcp,
            connection_id: 1,
            timestamp: 0,
            interface_guid: None,
        },
        process: ProcessInfo {
            pid: 4321,
            name: "browser.exe".to_string(),
            exe_path: None,
            user_sid: String::new(),
            arch: ProcessArch::X64,
            match_quality: MatchQuality::Heuristic,
        },
        tls: None,
        protocol: ProtocolKind::Http1,
        http_request: Some(HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            path: url.split('?').next().unwrap_or(url).to_string(),
            request_line_version: HttpVersion::Http11,
            headers: vec![
                ("Host".to_string(), "api.example.com".to_string()),
                ("Accept".to_string(), "application/json".to_string()),
            ],
            body: None,
            body_size: 0,
            timestamp: 0,
        }),
        http_response: Some(HttpResponse {
            status_code: status,
            status_text: if status < 300 { "OK" } else { "Error" }.to_string(),
            version: HttpVersion::Http11,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: Some(b"{\"ok\":true}".to_vec()),
            body_size: 11,
            timestamp: 0,
        }),
        websocket_frames: vec![],
        sse_events: vec![],
        grpc_call: None,
        started_at: 0,
        ended_at: Some(1000),
        bytes_sent: 120,
        bytes_received: 340,
    }
}

fn build_server() -> McpServerImpl {
    let storage = Arc::new(InMemoryStorage::new());
    storage
        .insert_session(&sample_session("GET", "https://api.example.com/users?page=1", 200))
        .unwrap();
    storage
        .insert_session(&sample_session("POST", "https://api.example.com/login", 401))
        .unwrap();

    let s: Arc<dyn StorageEngine> = storage.clone();
    let mut server = McpServerImpl::new();
    server.register_tool(Arc::new(GetCaptureStatusTool::new(s.clone())));
    server.register_tool(Arc::new(DiagnoseCaptureTool::new(s.clone())));
    server.register_tool(Arc::new(ListEndpointsTool::new(s.clone())));
    server.register_tool(Arc::new(ListTransactionsTool::new(s.clone())));
    server.register_tool(Arc::new(GetTransactionTool::new(s.clone())));
    server.register_tool(Arc::new(SearchTransactionsTool::new(s.clone())));
    server.register_tool(Arc::new(ExportAsCurlTool::new(s.clone())));
    server.register_tool(Arc::new(GetSessionStatsTool::new(s.clone())));
    server
}

fn req(id: u64, method: &str, params: Option<serde_json::Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(id)),
        method: method.to_string(),
        params,
    }
}

fn call_tool(
    server: &McpServerImpl,
    id: u64,
    tool: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let resp = server.handle_request(req(
        id,
        "tools/call",
        Some(serde_json::json!({ "name": tool, "arguments": args })),
    ));
    assert!(resp.error.is_none(), "tool {tool} error: {:?}", resp.error);
    resp.result.expect("tools/call must return a result")
}

#[test]
fn test_mcp_initialize_and_tools_list() {
    let server = build_server();

    let init = server.handle_request(req(1, "initialize", None));
    assert!(init.error.is_none());
    let init_res = init.result.unwrap();
    assert_eq!(init_res["protocolVersion"], "2024-11-05");
    assert_eq!(init_res["serverInfo"]["name"], "flowlens");

    let list = server.handle_request(req(2, "tools/list", None));
    let tools = list.result.unwrap()["tools"].as_array().unwrap().clone();
    assert_eq!(tools.len(), 8, "all 8 MCP tools must be listed");
    for t in &tools {
        assert!(t["name"].is_string());
        assert!(t["inputSchema"].is_object());
    }
}

#[test]
fn test_mcp_all_tools_call() {
    let server = build_server();

    let status = call_tool(&server, 10, "get_capture_status", serde_json::json!({}));
    assert_eq!(status["status"], "running");

    let list = call_tool(&server, 11, "list_transactions", serde_json::json!({}));
    assert_eq!(list["count"], 2);

    let tx = call_tool(
        &server,
        12,
        "get_transaction",
        serde_json::json!({ "id": 1, "side": "response" }),
    );
    assert_eq!(tx["id"], 1);
    assert!(tx["content"].as_str().unwrap().contains("ok"));

    let found = call_tool(
        &server,
        13,
        "search_transactions",
        serde_json::json!({ "query": "login" }),
    );
    assert!(found["count"].as_u64().unwrap() >= 1);

    let curl = call_tool(&server, 14, "export_as_curl", serde_json::json!({ "id": 1 }));
    assert!(curl["curl"].as_str().unwrap().contains("curl -X"));

    let stats = call_tool(&server, 15, "get_session_stats", serde_json::json!({}));
    assert_eq!(stats["transaction_count"], 2);

    let eps = call_tool(&server, 16, "list_endpoints", serde_json::json!({}));
    assert!(eps["count"].as_u64().unwrap() >= 1);

    let diag = call_tool(&server, 17, "diagnose_capture", serde_json::json!({}));
    assert_eq!(diag["status"], "healthy");
}

#[test]
fn test_mcp_unknown_tool_returns_error() {
    let server = build_server();
    let resp = server.handle_request(req(
        20,
        "tools/call",
        Some(serde_json::json!({ "name": "does_not_exist", "arguments": {} })),
    ));
    assert!(resp.error.is_some());
}
