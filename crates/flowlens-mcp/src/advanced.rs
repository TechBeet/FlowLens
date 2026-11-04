//! Advanced MCP tools for replay, diff, breakpoints, and rewrite rules.
//!
//! These tools operate on captured sessions stored in the flowlens storage
//! engine and expose debugger-grade capabilities over JSON-RPC.

use std::sync::Arc;

use async_trait::async_trait;
use flowlens_core::intercept::InterceptEngine;
use flowlens_core::models::SessionId;
use flowlens_core::rewrite::{RewriteAction, RewriteEngine, RewriteRule};
use flowlens_replay::{diff, ReplayClient};
use flowlens_storage::StorageEngine;

use crate::tools::McpTool;

/// Shared state for advanced tools.
pub struct AdvancedState {
    pub storage: Arc<dyn StorageEngine>,
    pub intercept: Arc<tokio::sync::Mutex<InterceptEngine>>,
    pub rewrite: Arc<tokio::sync::Mutex<RewriteEngine>>,
}

impl AdvancedState {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        Self {
            storage,
            intercept: Arc::new(tokio::sync::Mutex::new(InterceptEngine::new())),
            rewrite: Arc::new(tokio::sync::Mutex::new(RewriteEngine::new())),
        }
    }
}

/// ReplayTool - replays a captured HTTP request.
pub struct ReplayTool {
    state: Arc<AdvancedState>,
}

impl ReplayTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for ReplayTool {
    fn name(&self) -> &str {
        "replay_session"
    }
    fn description(&self) -> &str {
        "Replay a captured HTTP request. Returns response status, headers, body."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "required": ["session_id"], "properties": { "session_id": { "type": "integer" } }, "additionalProperties": false })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let session_id: u64 = args
            .get("session_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id"))?;
        let session = self
            .state
            .storage
            .get_session(SessionId(session_id))?
            .ok_or_else(|| anyhow::anyhow!("Session {} not found", session_id))?;
        let request = session
            .http_request
            .ok_or_else(|| anyhow::anyhow!("No HTTP request found in session {}", session_id))?;
        let client = ReplayClient::new()?;
        let result = client.replay(&request).await?;
        Ok(serde_json::json!({
            "session_id": session_id,
            "status": result.status_code,
            "status_text": result.status_text,
            "headers": result.headers,
            "body_size": result.body_size
        }))
    }
}

/// DiffTool - compares two captured sessions.
pub struct DiffTool {
    state: Arc<AdvancedState>,
}

impl DiffTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for DiffTool {
    fn name(&self) -> &str {
        "compare_sessions"
    }
    fn description(&self) -> &str {
        "Diff two captured sessions. Returns field-level differences."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "required": ["session_id_a", "session_id_b"], "properties": { "session_id_a": { "type": "integer" }, "session_id_b": { "type": "integer" } }, "additionalProperties": false })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id_a: u64 = args
            .get("session_id_a")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id_a"))?;
        let id_b: u64 = args
            .get("session_id_b")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id_b"))?;
        let s_a = self
            .state
            .storage
            .get_session(SessionId(id_a))?
            .ok_or_else(|| anyhow::anyhow!("Session {} not found", id_a))?;
        let s_b = self
            .state
            .storage
            .get_session(SessionId(id_b))?
            .ok_or_else(|| anyhow::anyhow!("Session {} not found", id_b))?;
        let diffs: Vec<serde_json::Value> = match (&s_a.http_request, &s_b.http_request) {
            (Some(r_a), Some(r_b)) => diff::compare_requests(r_a, r_b, "")
                .into_iter()
                .map(|d| {
                    let kind = match d.kind {
                        diff::DiffKind::Added => "added".to_string(),
                        diff::DiffKind::Removed => "removed".to_string(),
                        diff::DiffKind::Changed { ref old, ref new } => {
                            format!("changed ({} -> {})", old, new)
                        }
                    };
                    serde_json::json!({ "field": d.field, "kind": kind })
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(serde_json::json!({
            "session_id_a": id_a,
            "session_id_b": id_b,
            "diffs": diffs,
            "total_diffs": diffs.len()
        }))
    }
}

/// PauseSessionTool - pauses a session at a breakpoint.
pub struct PauseSessionTool {
    state: Arc<AdvancedState>,
}

impl PauseSessionTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for PauseSessionTool {
    fn name(&self) -> &str {
        "pause_session"
    }
    fn description(&self) -> &str {
        "Pause a captured session at a breakpoint for inspection/modification."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "required": ["session_id"], "properties": { "session_id": { "type": "integer" } }, "additionalProperties": false })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let session_id: u64 = args
            .get("session_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id"))?;
        let intercept = self.state.intercept.lock().await;
        intercept
            .pause(SessionId(session_id), None, None, None)
            .await?;
        Ok(serde_json::json!({ "session_id": session_id, "status": "paused" }))
    }
}

/// ReleaseSessionTool - releases a paused session.
pub struct ReleaseSessionTool {
    state: Arc<AdvancedState>,
}

impl ReleaseSessionTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for ReleaseSessionTool {
    fn name(&self) -> &str {
        "release_session"
    }
    fn description(&self) -> &str {
        "Release a paused session to continue processing."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "required": ["session_id"], "properties": { "session_id": { "type": "integer" } }, "additionalProperties": false })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let session_id: u64 = args
            .get("session_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id"))?;
        let intercept = self.state.intercept.lock().await;
        intercept.release(SessionId(session_id)).await?;
        Ok(serde_json::json!({ "session_id": session_id, "status": "released" }))
    }
}

/// ModifySessionTool - modifies a paused session's request/response.
pub struct ModifySessionTool {
    state: Arc<AdvancedState>,
}

impl ModifySessionTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for ModifySessionTool {
    fn name(&self) -> &str {
        "modify_session"
    }
    fn description(&self) -> &str {
        "Modify headers of a paused session before release."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "required": ["session_id"], "properties": { "session_id": { "type": "integer" }, "header": { "type": "string" }, "value": { "type": "string" } }, "additionalProperties": false })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let session_id: u64 = args
            .get("session_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id"))?;
        let header = args
            .get("header")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let session = self
            .state
            .storage
            .get_session(SessionId(session_id))?
            .ok_or_else(|| anyhow::anyhow!("Session {} not found", session_id))?;
        let mut new_request = session
            .http_request
            .ok_or_else(|| anyhow::anyhow!("No HTTP request found"))?;
        if !header.is_empty() {
            new_request.headers.retain(|(k, _)| k != &header);
            new_request.headers.push((header.clone(), value.clone()));
        }
        let intercept = self.state.intercept.lock().await;
        intercept
            .modify(SessionId(session_id), Some(new_request), None)
            .await?;
        Ok(serde_json::json!({
            "session_id": session_id,
            "header": header,
            "value": value
        }))
    }
}

/// AddRewriteRuleTool - adds a rewrite rule.
pub struct AddRewriteRuleTool {
    state: Arc<AdvancedState>,
}

impl AddRewriteRuleTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for AddRewriteRuleTool {
    fn name(&self) -> &str {
        "add_rewrite_rule"
    }
    fn description(&self) -> &str {
        "Add a request rewrite rule (method/url match -> header/url/body action)."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "required": ["id", "name", "url_regex", "action", "action_value"], "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "url_regex": { "type": "string" }, "action": { "type": "string" }, "action_value": { "type": "string" } }, "additionalProperties": false })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: id"))?;
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;
        let url_regex = args
            .get("url_regex")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: url_regex"))?;
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: action"))?;
        let action_value = args
            .get("action_value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: action_value"))?;
        let rule_action = match action {
            "set_url" => RewriteAction::SetUrl(action_value.to_string()),
            "set_method" => RewriteAction::SetMethod(action_value.to_string()),
            "set_header" => {
                let parts: Vec<&str> = action_value.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("set_header expects header:value"));
                }
                RewriteAction::SetHeader(parts[0].to_string(), parts[1].to_string())
            }
            "add_header" => {
                let parts: Vec<&str> = action_value.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("add_header expects header:value"));
                }
                RewriteAction::AddHeader(parts[0].to_string(), parts[1].to_string())
            }
            "remove_header" => RewriteAction::RemoveHeader(action_value.to_string()),
            _ => return Err(anyhow::anyhow!("Unknown action: {}", action)),
        };
        let rule = RewriteRule {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            priority: 0,
            method_regex: None,
            url_regex: Some(url_regex.to_string()),
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![rule_action],
        };
        let mut rewrite = self.state.rewrite.lock().await;
        rewrite.add_rule(rule);
        Ok(serde_json::json!({ "status": "added", "id": id }))
    }
}

/// ListRewriteRulesTool - lists rewrite rules.
pub struct ListRewriteRulesTool {
    state: Arc<AdvancedState>,
}

impl ListRewriteRulesTool {
    pub fn new(state: Arc<AdvancedState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl McpTool for ListRewriteRulesTool {
    fn name(&self) -> &str {
        "list_rewrite_rules"
    }
    fn description(&self) -> &str {
        "List all registered rewrite rules."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {}, "additionalProperties": false })
    }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let rewrite = self.state.rewrite.lock().await;
        let rules: Vec<serde_json::Value> = rewrite
            .list_rules()
            .iter()
            .map(|r| {
                serde_json::json!({ "id": r.id, "name": r.name, "enabled": r.enabled, "priority": r.priority, "url_regex": r.url_regex })
            })
            .collect();
        Ok(serde_json::json!({ "rules": rules }))
    }
}
