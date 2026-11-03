//! Phase C: Breakpoint / Interception architecture.

use crate::models::{HttpRequest, HttpResponse, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub method: Option<String>,
    pub url_regex: Option<String>,
    pub status_code: Option<u16>,
    pub header: Option<(String, String)>,
    pub body_contains: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterceptState {
    Paused,
    Released,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PausePoint {
    pub session_id: SessionId,
    pub timestamp: i64,
    pub rule_id: Option<String>,
    pub request: Option<HttpRequest>,
    pub response: Option<HttpResponse>,
    pub state: InterceptState,
}

#[derive(Debug, Error)]
pub enum InterceptError {
    #[error("Session {0:?} is not paused")]
    NotPaused(SessionId),
    #[error("Session {0:?} is already paused")]
    AlreadyPaused(SessionId),
    #[error("Session {0:?} not found in pause points")]
    NotFound(SessionId),
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug)]
pub struct InterceptEngine {
    rules: Arc<Mutex<Vec<BreakpointRule>>>,
    pause_points: Arc<Mutex<HashMap<SessionId, PausePoint>>>,
}

impl Default for InterceptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InterceptEngine {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(Mutex::new(Vec::new())),
            pause_points: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_rule(&self, rule: BreakpointRule) {
        let mut guard = self.rules.lock().await;
        guard.push(rule);
    }

    pub async fn remove_rule(&self, rule_id: &str) -> bool {
        let mut guard = self.rules.lock().await;
        let len = guard.len();
        guard.retain(|r| r.id != rule_id);
        guard.len() < len
    }

    pub async fn list_rules(&self) -> Vec<BreakpointRule> {
        let guard = self.rules.lock().await;
        guard.clone()
    }
}

impl InterceptEngine {
    pub async fn pause(
        &self,
        session_id: SessionId,
        rule_id: Option<String>,
        request: Option<HttpRequest>,
        response: Option<HttpResponse>,
    ) -> Result<PausePoint, InterceptError> {
        let mut guard = self.pause_points.lock().await;
        if guard.contains_key(&session_id) {
            return Err(InterceptError::AlreadyPaused(session_id));
        }
        let pp = PausePoint {
            session_id,
            timestamp: time_now(),
            rule_id,
            request,
            response,
            state: InterceptState::Paused,
        };
        guard.insert(session_id, pp.clone());
        Ok(pp)
    }

    pub async fn release(&self, session_id: SessionId) -> Result<PausePoint, InterceptError> {
        let mut guard = self.pause_points.lock().await;
        guard
            .remove(&session_id)
            .ok_or(InterceptError::NotPaused(session_id))
    }

    pub async fn modify(
        &self,
        session_id: SessionId,
        new_request: Option<HttpRequest>,
        new_response: Option<HttpResponse>,
    ) -> Result<PausePoint, InterceptError> {
        let mut guard = self.pause_points.lock().await;
        let pp = guard
            .get_mut(&session_id)
            .ok_or(InterceptError::NotPaused(session_id))?;
        if let Some(req) = new_request {
            pp.request = Some(req);
        }
        if let Some(resp) = new_response {
            pp.response = Some(resp);
        }
        pp.state = InterceptState::Modified;
        Ok(pp.clone())
    }

    pub async fn is_paused(&self, session_id: SessionId) -> bool {
        let guard = self.pause_points.lock().await;
        guard.contains_key(&session_id)
    }

    pub async fn list_paused(&self) -> Vec<PausePoint> {
        let guard = self.pause_points.lock().await;
        guard.values().cloned().collect()
    }

    pub async fn get_pause_point(&self, session_id: SessionId) -> Option<PausePoint> {
        let guard = self.pause_points.lock().await;
        guard.get(&session_id).cloned()
    }
    pub async fn get_request(&self, session_id: SessionId) -> Option<HttpRequest> {
        let guard = self.pause_points.lock().await;
        guard.get(&session_id).and_then(|pp| pp.request.clone())
    }

    pub async fn get_response(&self, session_id: SessionId) -> Option<HttpResponse> {
        let guard = self.pause_points.lock().await;
        guard.get(&session_id).and_then(|pp| pp.response.clone())
    }
}

#[allow(dead_code)]
fn rule_matches(
    rule: &BreakpointRule,
    request: Option<&HttpRequest>,
    response: Option<&HttpResponse>,
) -> bool {
    if !rule.enabled {
        return false;
    }

    if let Some(ref pattern) = rule.method {
        let method = request.map(|r| r.method.as_str()).unwrap_or("");
        if let Ok(re) = regex_lite::Regex::new(pattern) {
            if !re.is_match(method) {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(ref pattern) = rule.url_regex {
        let url = request.map(|r| r.url.as_str()).unwrap_or("");
        if let Ok(re) = regex_lite::Regex::new(pattern) {
            if !re.is_match(url) {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(status) = rule.status_code {
        let resp_status = response.map(|r| r.status_code).unwrap_or(0);
        if resp_status != status {
            return false;
        }
    }

    if let Some((ref key, ref value)) = rule.header {
        let mut headers: Vec<&(String, String)> = Vec::new();
        if let Some(r) = request {
            headers.extend(r.headers.iter());
        }
        if let Some(r) = response {
            headers.extend(r.headers.iter());
        }
        let hm = headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case(key) && v == value);
        if !hm {
            return false;
        }
    }

    if let Some(needle) = &rule.body_contains {
        let body_found = request
            .as_ref()
            .and_then(|r| r.body.as_deref())
            .map(|b| contains_slice(b, needle))
            .unwrap_or(false)
            || response
                .as_ref()
                .and_then(|r| r.body.as_deref())
                .map(|b| contains_slice(b, needle))
                .unwrap_or(false);
        if !body_found {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HttpVersion;

    fn make_request(method: &str, url: &str, body: Option<&[u8]>) -> HttpRequest {
        HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            path: url.to_string(),
            request_line_version: HttpVersion::Http11,
            headers: vec![],
            body: body.map(|b| b.to_vec()),
            body_size: body.map(|b| b.len()).unwrap_or(0),
            timestamp: 0,
        }
    }

    #[tokio::test]
    async fn test_engine_pause_release_flow() {
        let engine = InterceptEngine::new();
        let sid = SessionId(1);
        assert!(!engine.is_paused(sid).await);
        let pp = engine
            .pause(
                sid,
                Some("rule-1".to_string()),
                Some(make_request("GET", "http://example.com", None)),
                None,
            )
            .await
            .unwrap();
        assert_eq!(pp.state, InterceptState::Paused);
        assert!(engine.is_paused(sid).await);
        let err = engine.pause(sid, None, None, None).await.unwrap_err();
        assert!(matches!(err, InterceptError::AlreadyPaused(_)));
        let released = engine.release(sid).await.unwrap();
        assert_eq!(released.session_id, sid);
        assert!(!engine.is_paused(sid).await);
        let err = engine.release(sid).await.unwrap_err();
        assert!(matches!(err, InterceptError::NotPaused(_)));
    }

    #[tokio::test]
    async fn test_engine_modify() {
        let engine = InterceptEngine::new();
        let sid = SessionId(2);
        engine
            .pause(
                sid,
                None,
                Some(make_request("GET", "http://example.com", None)),
                None,
            )
            .await
            .unwrap();
        let modified_req = make_request("POST", "http://modified.com", Some(b"new body"));
        let pp = engine
            .modify(sid, Some(modified_req.clone()), None)
            .await
            .unwrap();
        assert_eq!(pp.state, InterceptState::Modified);
        assert_eq!(pp.request.as_ref().unwrap().method, "POST");
        assert_eq!(pp.request.as_ref().unwrap().url, "http://modified.com");
        engine.release(sid).await.unwrap();
    }
}

fn time_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

fn contains_slice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    for i in 0..=(haystack.len() - needle.len()) {
        if haystack[i..i + needle.len()] == *needle {
            return true;
        }
    }
    false
}
