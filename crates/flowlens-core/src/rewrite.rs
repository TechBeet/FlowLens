//! Phase C: Request Rewrite Rule Engine.

use crate::models::HttpRequest;
use regex_lite::Regex;
use serde::{Deserialize, Serialize};

/// Rewrite action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RewriteAction {
    SetUrl(String),
    SetMethod(String),
    SetHeader(String, String),
    AddHeader(String, String),
    RemoveHeader(String),
    SetBody(Vec<u8>),
}

/// Rewrite rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub method_regex: Option<String>,
    pub url_regex: Option<String>,
    pub status_code: Option<u16>,
    pub header_match: Option<(String, String)>,
    pub body_contains: Option<Vec<u8>>,
    pub actions: Vec<RewriteAction>,
}

/// Result of rule matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteMatch {
    pub rule_id: String,
    pub rule_name: String,
    pub matched_conditions: Vec<String>,
}

/// Modified request after applying rules
#[derive(Debug, Clone)]
pub struct ModifiedRequest {
    pub original: HttpRequest,
    pub modified: HttpRequest,
    pub applied_rules: Vec<String>,
}

/// Rewrite engine for managing and applying rules
pub struct RewriteEngine {
    rules: Vec<RewriteRule>,
}

impl Default for RewriteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RewriteEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: RewriteRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|a| a.priority);
    }

    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let len = self.rules.len();
        self.rules.retain(|r| r.id != rule_id);
        self.rules.len() < len
    }

    pub fn toggle_rule(&mut self, rule_id: &str, enabled: bool) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            rule.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn list_rules(&self) -> &[RewriteRule] {
        &self.rules
    }

    pub fn matches_request(
        &self,
        request: &HttpRequest,
        rule: &RewriteRule,
    ) -> Option<RewriteMatch> {
        if !rule.enabled {
            return None;
        }
        let mut matched_conditions: Vec<String> = Vec::new();

        if let Some(ref pattern) = rule.method_regex {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(&request.method) {
                    matched_conditions.push(format!("method={}", pattern));
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        if let Some(ref pattern) = rule.url_regex {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(&request.url) {
                    matched_conditions.push(format!("url={}", pattern));
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        if let Some(header_tuple) = &rule.header_match {
            let (key, value) = header_tuple;
            let has_header = request
                .headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case(key) && v == value);
            if !has_header {
                return None;
            }
            matched_conditions.push(format!("header={}:{}", key, value));
        }

        if let Some(ref needle) = rule.body_contains {
            let body_match = request
                .body
                .as_ref()
                .map(|b| contains_slice(b, needle))
                .unwrap_or(false);
            if !body_match {
                return None;
            }
            matched_conditions.push("body_contains".to_string());
        }

        Some(RewriteMatch {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            matched_conditions,
        })
    }

    pub fn apply_rules(&self, request: HttpRequest) -> ModifiedRequest {
        let mut modified = request.clone();
        let mut applied_rules: Vec<String> = Vec::new();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            if self.matches_request(&request, rule).is_some() {
                applied_rules.push(rule.id.clone());
                for action in &rule.actions {
                    match action {
                        RewriteAction::SetUrl(url) => {
                            modified.url = url.clone();
                            modified.path = url.clone();
                        }
                        RewriteAction::SetMethod(method) => {
                            modified.method = method.clone();
                        }
                        RewriteAction::SetHeader(name, value) => {
                            if let Some(pos) = modified
                                .headers
                                .iter()
                                .position(|(k, _)| k.eq_ignore_ascii_case(name))
                            {
                                modified.headers[pos] = (name.clone(), value.clone());
                            } else {
                                modified.headers.push((name.clone(), value.clone()));
                            }
                        }
                        RewriteAction::AddHeader(name, value) => {
                            modified.headers.push((name.clone(), value.clone()));
                        }
                        RewriteAction::RemoveHeader(name) => {
                            modified
                                .headers
                                .retain(|(k, _)| !k.eq_ignore_ascii_case(name));
                        }
                        RewriteAction::SetBody(body) => {
                            modified.body = Some(body.clone());
                            modified.body_size = body.len();
                        }
                    }
                }
            }
        }

        ModifiedRequest {
            original: request,
            modified,
            applied_rules,
        }
    }

    pub fn load_rules(path: &str) -> Result<Vec<RewriteRule>, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let rules: Vec<RewriteRule> = serde_json::from_str(&content)?;
        Ok(rules)
    }

    pub fn save_rules(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(&self.rules)?;
        std::fs::write(path, json)?;
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HttpRequest, HttpVersion};

    fn make_request(method: &str, url: &str) -> HttpRequest {
        HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            path: url.to_string(),
            request_line_version: HttpVersion::Http11,
            headers: vec![],
            body: None,
            body_size: 0,
            timestamp: 0,
        }
    }

    #[test]
    fn test_rule_matches_method() {
        let mut engine = RewriteEngine::new();
        let rule = RewriteRule {
            id: "test-1".to_string(),
            name: "Test Rule".to_string(),
            enabled: true,
            priority: 0,
            method_regex: Some("^POST$".to_string()),
            url_regex: None,
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![],
        };
        engine.add_rule(rule);

        let post_req = make_request("POST", "http://example.com");
        let get_req = make_request("GET", "http://example.com");

        assert!(engine
            .matches_request(&post_req, &engine.rules[0])
            .is_some());
        assert!(engine.matches_request(&get_req, &engine.rules[0]).is_none());
    }

    #[test]
    fn test_apply_set_url() {
        let mut engine = RewriteEngine::new();
        let rule = RewriteRule {
            id: "url-modify".to_string(),
            name: "Modify URL".to_string(),
            enabled: true,
            priority: 0,
            method_regex: Some("^GET$".to_string()),
            url_regex: Some("http://original\\.com".to_string()),
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![RewriteAction::SetUrl("http://modified.com".to_string())],
        };
        engine.add_rule(rule);

        let req = make_request("GET", "http://original.com/path");
        let result = engine.apply_rules(req);

        assert_eq!(result.modified.url, "http://modified.com");
        assert_eq!(result.applied_rules.len(), 1);
        assert_eq!(result.applied_rules[0], "url-modify");
    }

    #[test]
    fn test_apply_add_header() {
        let mut engine = RewriteEngine::new();
        let rule = RewriteRule {
            id: "add-header".to_string(),
            name: "Add Header".to_string(),
            enabled: true,
            priority: 0,
            method_regex: None,
            url_regex: None,
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![
                RewriteAction::AddHeader("X-Custom".to_string(), "value".to_string()),
                RewriteAction::AddHeader("X-Another".to_string(), "data".to_string()),
            ],
        };
        engine.add_rule(rule);

        let mut req = make_request("GET", "http://example.com");
        req.headers = vec![("Host".to_string(), "example.com".to_string())];

        let result = engine.apply_rules(req);

        assert_eq!(result.modified.headers.len(), 3);
        assert!(result
            .modified
            .headers
            .iter()
            .any(|(k, v)| { k == "X-Custom" && v == "value" }));
    }

    #[test]
    fn test_toggle_rule_disables_matching() {
        let mut engine = RewriteEngine::new();
        let rule = RewriteRule {
            id: "disabled-rule".to_string(),
            name: "Disabled".to_string(),
            enabled: true,
            priority: 0,
            method_regex: Some("^GET$".to_string()),
            url_regex: None,
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![RewriteAction::SetUrl("http://modified.com".to_string())],
        };
        engine.add_rule(rule);

        engine.toggle_rule("disabled-rule", false);

        let req = make_request("GET", "http://example.com");
        let result = engine.apply_rules(req);

        assert!(result.applied_rules.is_empty());
        assert_eq!(result.modified.url, "http://example.com");
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = RewriteEngine::new();
        engine.add_rule(RewriteRule {
            id: "to-remove".to_string(),
            name: "To Remove".to_string(),
            enabled: true,
            priority: 0,
            method_regex: None,
            url_regex: None,
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![],
        });
        engine.add_rule(RewriteRule {
            id: "keep".to_string(),
            name: "Keep".to_string(),
            enabled: true,
            priority: 0,
            method_regex: None,
            url_regex: None,
            status_code: None,
            header_match: None,
            body_contains: None,
            actions: vec![],
        });

        assert!(engine.remove_rule("to-remove"));
        assert!(!engine.remove_rule("non-existent"));
        assert_eq!(engine.rules.len(), 1);
        assert_eq!(engine.rules[0].id, "keep");
    }
}
