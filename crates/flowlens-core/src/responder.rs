//! Auto-Responder for FlowLens

use crate::models::{HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponderRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub url_pattern: String,
    pub method: Option<String>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Vec<u8>,
    pub delay_ms: u64,
    pub priority: u8,
}

pub struct ResponderEngine {
    rules: Vec<ResponderRule>,
}

impl Default for ResponderEngine {
    fn default() -> Self { Self::new() }
}

impl ResponderEngine {
    pub fn new() -> Self { Self { rules: Vec::new() } }

    pub fn add_rule(&mut self, rule: ResponderRule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let len = self.rules.len();
        self.rules.retain(|r| r.id != rule_id);
        self.rules.len() < len
    }

    pub fn list_rules(&self) -> &[ResponderRule] { &self.rules }

    pub fn find_matching_rule(&self, request: &HttpRequest) -> Option<&ResponderRule> {
        self.rules.iter().filter(|r| r.enabled).find(|r| {
            request.url.contains(&r.url_pattern)
                && r.method.as_ref().map(|m| m.eq_ignore_ascii_case(&request.method)).unwrap_or(true)
        })
    }

    pub fn generate_response(&self, rule: &ResponderRule) -> HttpResponse {
        HttpResponse {
            status_code: rule.response_status,
            status_text: status_text_for(rule.response_status),
            version: crate::models::HttpVersion::Http11,
            headers: rule.response_headers.clone(),
            body: Some(rule.response_body.clone()),
            body_size: rule.response_body.len(),
            timestamp: crate::models::now_micros(),
        }
    }
}

fn status_text_for(code: u16) -> String {
    match code {
        200 => "OK".to_string(),
        201 => "Created".to_string(),
        204 => "No Content".to_string(),
        301 => "Moved Permanently".to_string(),
        302 => "Found".to_string(),
        400 => "Bad Request".to_string(),
        401 => "Unauthorized".to_string(),
        403 => "Forbidden".to_string(),
        404 => "Not Found".to_string(),
        500 => "Internal Server Error".to_string(),
        _ => "Unknown".to_string(),
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightingRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub condition: String,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterProfile {
    pub id: String,
    pub name: String,
    pub filters: Vec<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Har, Saz, Curl, Json, Xml, Tsv, Excel, Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub labels: Vec<String>,
    pub values: Vec<u64>,
    pub chart_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStats {
    pub domain: String,
    pub request_count: u64,
    pub total_bytes: u64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeDistribution {
    pub range: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDistribution {
    pub status_code: u16,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePoint {
    pub timestamp: i64,
    pub request_count: u64,
    pub bytes_transferred: u64,
}