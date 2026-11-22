//! MCP tool definitions for FlowLens (HTTPDebuggerPro-compatible)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use flowlens_core::models::SessionId;
use flowlens_storage::StorageEngine;

#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

pub struct GetCaptureStatusTool { storage: Arc<dyn StorageEngine> }
impl GetCaptureStatusTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for GetCaptureStatusTool {
    fn name(&self) -> &str { "get_capture_status" }
    fn description(&self) -> &str { "Reports whether HTTP Debugger is currently capturing." }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type":"object","properties":{},"additionalProperties":false}) }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let sessions = self.storage.list_sessions()?;
        let total: u64 = sessions.iter().map(|s| s.bytes_sent + s.bytes_received).sum();
        Ok(serde_json::json!({"capturing":true,"session_transaction_count":sessions.len(),"total_bytes":total,"status":"running"}))
    }
}

pub struct DiagnoseCaptureTool { storage: Arc<dyn StorageEngine> }
impl DiagnoseCaptureTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for DiagnoseCaptureTool {
    fn name(&self) -> &str { "diagnose_capture" }
    fn description(&self) -> &str { "Explains why there is no traffic or why HTTPS is not being decrypted." }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type":"object","properties":{},"additionalProperties":false}) }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let sessions = self.storage.list_sessions()?;
        let mut findings = Vec::new();
        if sessions.is_empty() {
            findings.push(serde_json::json!({"category":"capture","severity":"warning","description":"No traffic captured yet.","hint":"Start capture and make a request."}));
        } else {
            findings.push(serde_json::json!({"category":"capture","severity":"info","description":format!("Capturing {} transactions",sessions.len()),"hint":"Use list_transactions to view captured traffic."}));
        }
        Ok(serde_json::json!({"findings":findings,"status":if sessions.is_empty(){"empty"}else{"healthy"}}))
    }
}

pub struct ListEndpointsTool { storage: Arc<dyn StorageEngine> }
impl ListEndpointsTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for ListEndpointsTool {
    fn name(&self) -> &str { "list_endpoints" }
    fn description(&self) -> &str { "Groups the session by host, method and path template." }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type":"object","properties":{},"additionalProperties":false}) }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let sessions = self.storage.list_sessions()?;
        let mut stats: HashMap<String, u64> = HashMap::new();
        for s in &sessions {
            let key = format!("{} {}", s.conn.dst_addr, s.process.name);
            *stats.entry(key).or_insert(0) += 1;
        }
        let mut result: Vec<serde_json::Value> = stats.iter().map(|(k, v)| {
            serde_json::json!({"endpoint": k, "transaction_count": v})
        }).collect();
        result.sort_by(|a, b| b["transaction_count"].as_u64().unwrap_or(0).cmp(&a["transaction_count"].as_u64().unwrap_or(0)));
        Ok(serde_json::json!({"endpoints": result, "count": result.len()}))
    }
}

pub struct ListTransactionsTool { storage: Arc<dyn StorageEngine> }
impl ListTransactionsTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for ListTransactionsTool {
    fn name(&self) -> &str { "list_transactions" }
    fn description(&self) -> &str { "Lists captured transactions with metadata and body sizes." }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{
            "skip":{"type":"integer","description":"Skip (default 0)"},
            "limit":{"type":"integer","description":"Limit (default 50)"},
            "host":{"type":"string","description":"Filter by host substring"},
            "method":{"type":"string","description":"Filter by method"},
            "status":{"type":"integer","description":"Filter by status code"},
            "url_contains":{"type":"string","description":"Filter by URL substring"}
        },"additionalProperties":false})
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let skip = args.get("skip").and_then(|v| v.as_u64()).unwrap_or(0);
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
        let host_f = args.get("host").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
        let method_f = args.get("method").and_then(|v| v.as_str()).map(|s| s.to_uppercase());
        let status_f = args.get("status").and_then(|v| v.as_u64());
        let url_f = args.get("url_contains").and_then(|v| v.as_str()).map(|s| s.to_lowercase());

        let sessions = self.storage.list_sessions()?;
        let filtered: Vec<_> = sessions.into_iter().filter(|s| {
            if let Some(ref h) = host_f {
                if !s.conn.dst_addr.to_string().to_lowercase().contains(h) { return false; }
            }
            if let Some(ref m) = method_f {
                if let Some(ref req) = s.http_request {
                    if req.method.to_uppercase() != *m { return false; }
                } else { return false; }
            }
            if let Some(st) = status_f {
                if let Some(ref resp) = s.http_response {
                    if resp.status_code != st as u16 { return false; }
                } else { return false; }
            }
            if let Some(ref u) = url_f {
                if let Some(ref req) = s.http_request {
                    if !req.url.to_lowercase().contains(u) { return false; }
                } else { return false; }
            }
            true
        }).skip(skip as usize).take(limit as usize).collect();

        let total = filtered.len();
        let result: Vec<serde_json::Value> = filtered.iter().map(|s| {
            serde_json::json!({
                "id": s.session_id.0,
                "host": s.conn.dst_addr.to_string(),
                "method": s.http_request.as_ref().map(|r| r.method.as_str()).unwrap_or("?"),
                "url": s.http_request.as_ref().map(|r| r.url.as_str()).unwrap_or("?"),
                "status": s.http_response.as_ref().map(|r| r.status_code).unwrap_or(0),
                "process": s.process.name,
                "request_bytes": s.bytes_sent,
                "response_bytes": s.bytes_received,
                "outcome": if s.http_response.is_some() { "complete" } else { "no_response" }
            })
        }).collect();

        Ok(serde_json::json!({"transactions": result, "count": result.len(), "total": total, "skip": skip, "limit": limit}))
    }
}
pub struct GetTransactionTool { storage: Arc<dyn StorageEngine> }
impl GetTransactionTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for GetTransactionTool {
    fn name(&self) -> &str { "get_transaction" }
    fn description(&self) -> &str { "Returns one transaction with headers and body slice." }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["id"],"properties":{
            "id":{"type":"integer","description":"Transaction ID"},
            "side":{"type":"string","enum":["request","response"],"description":"Body side (default request)"},
            "byte_offset":{"type":"integer","description":"Body slice start (default 0)"}
        },"additionalProperties":false})
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id: u64 = args.get("id").and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: id"))?;
        let side = args.get("side").and_then(|v| v.as_str()).unwrap_or("request");
        let byte_offset = args.get("byte_offset").and_then(|v| v.as_u64()).unwrap_or(0);

        let session = self.storage.get_session(SessionId(id))?;
        match session {
            Some(s) => {
                let (headers, body) = match side {
                    "request" => {
                        if let Some(ref req) = s.http_request {
                            let h = serde_json::Value::Array(req.headers.iter().map(|(k,v)| serde_json::json!({"name": k, "value": v})).collect());
                            (h, req.body.clone().unwrap_or_default())
                        } else { (serde_json::json!([]), Vec::new()) }
                    }
                    "response" => {
                        if let Some(ref resp) = s.http_response {
                            let h = serde_json::Value::Array(resp.headers.iter().map(|(k,v)| serde_json::json!({"name": k, "value": v})).collect());
                            (h, resp.body.clone().unwrap_or_default())
                        } else { (serde_json::json!([]), Vec::new()) }
                    }
                    _ => return Err(anyhow::anyhow!("Invalid side: {}", side)),
                };
let slice = if (byte_offset as usize) < body.len() { &body[byte_offset as usize..] } else { &[] };
let truncated = (byte_offset as usize) < body.len() && slice.len() < body.len();
                let content = String::from_utf8_lossy(slice).to_string();
                Ok(serde_json::json!({
                    "id": s.session_id.0, "side": side, "byte_offset": byte_offset,
                    "headers": headers, "body_size": body.len(), "returned": slice.len(),
                    "truncated": truncated,
                    "content": content, "complete": s.http_response.is_some(),
                    "outcome": if s.http_response.is_some() { "complete" } else { "no_response" }
                }))
            }
            None => Err(anyhow::anyhow!("Transaction {} is not in the session", id)),
        }
    }
}

pub struct SearchTransactionsTool { storage: Arc<dyn StorageEngine> }
impl SearchTransactionsTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for SearchTransactionsTool {
    fn name(&self) -> &str { "search_transactions" }
    fn description(&self) -> &str { "Searches URLs, headers and bodies." }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["query"],"properties":{
            "query":{"type":"string","description":"Search query"},
            "case_sensitive":{"type":"boolean","description":"Case sensitive (default false)"}
        },"additionalProperties":false})
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let query = args.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: query"))?;
        let case_sensitive = args.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);
        let sessions = self.storage.list_sessions()?;
        let q = if case_sensitive { query.to_string() } else { query.to_lowercase() };
        let mut matches = Vec::new();
        for s in &sessions {
            let mut fields = Vec::new();
            if let Some(ref req) = s.http_request {
                let url = if case_sensitive { req.url.clone() } else { req.url.to_lowercase() };
                if url.contains(&q) { fields.push("url".to_string()); }
                for (n, v) in &req.headers {
                    let val = if case_sensitive { v.clone() } else { v.to_lowercase() };
                    if val.contains(&q) { fields.push(format!("request_header.{}", n)); }
                }
            }
            if let Some(ref resp) = s.http_response {
                for (n, v) in &resp.headers {
                    let val = if case_sensitive { v.clone() } else { v.to_lowercase() };
                    if val.contains(&q) { fields.push(format!("response_header.{}", n)); }
                }
            }
            if !fields.is_empty() {
                matches.push(serde_json::json!({
                    "id": s.session_id.0,
                    "host": s.conn.dst_addr.to_string(),
                    "method": s.http_request.as_ref().map(|r| r.method.as_str()).unwrap_or("?"),
                    "url": s.http_request.as_ref().map(|r| r.url.as_str()).unwrap_or("?"),
                    "matched_fields": fields
                }));
            }
        }
        Ok(serde_json::json!({"matches": matches, "count": matches.len(), "complete": true}))
    }
}
pub struct ExportAsCurlTool { storage: Arc<dyn StorageEngine> }
impl ExportAsCurlTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for ExportAsCurlTool {
    fn name(&self) -> &str { "export_as_curl" }
    fn description(&self) -> &str { "Rebuilds a captured request as a cURL command." }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["id"],"properties":{"id":{"type":"integer","description":"Transaction ID"}},"additionalProperties":false})
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id: u64 = args.get("id").and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: id"))?;
        let session = self.storage.get_session(SessionId(id))?;
        match session {
            Some(s) => {
                let req = s.http_request.ok_or_else(|| anyhow::anyhow!("No request"))?;
                let mut cmd = format!("curl -X {}", req.method);
                cmd.push_str(&format!(" '{}'", req.url));
                for (name, value) in &req.headers {
                    if name.to_lowercase() == "content-length" || name.to_lowercase() == "host" { continue; }
                    cmd.push_str(&format!(" -H '{}: {}'", name, value));
                }
                if let Some(ref body) = req.body {
                    if !body.is_empty() {
                        cmd.push_str(&format!(" --data-binary '{}'", String::from_utf8_lossy(body)));
                    }
                }
                Ok(serde_json::json!({"id": s.session_id.0, "curl": cmd, "body_omitted": req.body.is_none() || req.body.as_ref().unwrap().is_empty()}))
            }
            None => Err(anyhow::anyhow!("Transaction {} is not in the session", id)),
        }
    }
}

pub struct GetSessionStatsTool { storage: Arc<dyn StorageEngine> }
impl GetSessionStatsTool { pub fn new(s: Arc<dyn StorageEngine>) -> Self { Self { storage: s } } }
#[async_trait]
impl McpTool for GetSessionStatsTool {
    fn name(&self) -> &str { "get_session_stats" }
    fn description(&self) -> &str { "Summarises the session: counts, status classes, hosts, methods, byte totals." }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type":"object","properties":{},"additionalProperties":false}) }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let sessions = self.storage.list_sessions()?;
        let mut status_classes: HashMap<u16, u64> = HashMap::new();
        let mut methods: HashMap<String, u64> = HashMap::new();
        let mut hosts: Vec<String> = Vec::new();
        let mut total_req = 0u64; let mut total_resp = 0u64;
        let mut ws_count = 0u64; let mut sse_count = 0u64;
        for s in &sessions {
            if let Some(ref resp) = s.http_response {
                *status_classes.entry(resp.status_code / 100 * 100).or_insert(0) += 1;
            }
            if let Some(ref req) = s.http_request {
                *methods.entry(req.method.clone()).or_insert(0) += 1;
            }
            let host = s.conn.dst_addr.to_string();
            if !hosts.contains(&host) { hosts.push(host); }
            total_req += s.bytes_sent;
            total_resp += s.bytes_received;
            if !s.websocket_frames.is_empty() { ws_count += 1; }
            if !s.sse_events.is_empty() { sse_count += 1; }
        }
        let dur_total: u64 = sessions.iter().map(|s| {
            (s.ended_at.unwrap_or(s.started_at) - s.started_at) as u64
        }).sum();
        Ok(serde_json::json!({
            "transaction_count": sessions.len(), "websocket_count": ws_count, "sse_count": sse_count,
            "status_classes": status_classes, "hosts": hosts, "methods": methods,
            "total_request_bytes": total_req, "total_response_bytes": total_resp,
            "total_bytes": total_req + total_resp,
            "average_duration_ms": if sessions.len() > 0 { dur_total / sessions.len() as u64 } else { 0 }
        }))
    }
}
