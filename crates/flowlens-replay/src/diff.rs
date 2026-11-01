//! HTTP session comparison utilities for FlowLens.

use flowlens_core::models::{HttpRequest, HttpResponse, TrafficEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Difference types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffKind {
    Added,
    Removed,
    Changed { old: String, new: String },
}

/// Field-level difference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    pub field: String,
    pub kind: DiffKind,
}

/// Request/Response pair diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDiff {
    pub session_id: u64,
    pub method: String,
    pub url: String,
    pub diffs: Vec<FieldDiff>,
}

/// Full session comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub session_a_id: u64,
    pub session_b_id: u64,
    pub added_requests: Vec<RequestDiff>,
    pub removed_requests: Vec<RequestDiff>,
    pub changed_requests: Vec<RequestDiff>,
    pub total_diffs: usize,
}
/// Extract requests from a session
pub fn extract_requests(events: &[TrafficEvent]) -> HashMap<String, (HttpRequest, Option<HttpResponse>)> {
    let mut map = HashMap::new();
    let mut current_request: Option<(String, HttpRequest)> = None;

    for event in events {
        match event {
            TrafficEvent::HttpRequest { conn, request, .. } => {
                let key = format!("{}:{}", conn.src_addr, request.path);
                current_request = Some((key.clone(), request.clone()));
                map.insert(key, (request.clone(), None));
            }
            TrafficEvent::HttpResponse { response, .. } => {
                if let Some((key, _req)) = &current_request {
                    if let Some(entry) = map.get_mut(key) {
                        entry.1 = Some(response.clone());
                    }
                }
            }
            _ => {}
        }
    }

    map
}

/// Compare two requests and return field differences
pub fn compare_requests(a: &HttpRequest, b: &HttpRequest, _url: &str) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();

    if a.method != b.method {
        diffs.push(FieldDiff {
            field: "method".to_string(),
            kind: DiffKind::Changed {
                old: a.method.clone(),
                new: b.method.clone(),
            },
        });
    }

    if a.url != b.url {
        diffs.push(FieldDiff {
            field: "url".to_string(),
            kind: DiffKind::Changed {
                old: a.url.clone(),
                new: b.url.clone(),
            },
        });
    }

    let a_headers: HashMap<_, _> = a.headers.iter().cloned().collect();
    let b_headers: HashMap<_, _> = b.headers.iter().cloned().collect();

    for (key, val_a) in &a_headers {
        if let Some(val_b) = b_headers.get(key) {
            if val_a != val_b {
                diffs.push(FieldDiff {
                    field: format!("header:{}", key),
                    kind: DiffKind::Changed {
                        old: val_a.clone(),
                        new: val_b.clone(),
                    },
                });
            }
        } else {
            diffs.push(FieldDiff {
                field: format!("header:{}", key),
                kind: DiffKind::Removed,
            });
        }
    }

    for (key, val_b) in &b_headers {
        if !a_headers.contains_key(key) {
            diffs.push(FieldDiff {
                field: format!("header:{}", key),
                kind: DiffKind::Added,
            });
        }
    }

    match (&a.body, &b.body) {
        (Some(a_body), Some(b_body)) => {
            if a_body != b_body {
                let a_preview = String::from_utf8_lossy(&a_body[..a_body.len().min(100)]).to_string();
                let b_preview = String::from_utf8_lossy(&b_body[..b_body.len().min(100)]).to_string();
                diffs.push(FieldDiff {
                    field: "body".to_string(),
                    kind: DiffKind::Changed {
                        old: a_preview,
                        new: b_preview,
                    },
                });
            }
        }
        (Some(_), None) => {
            diffs.push(FieldDiff {
                field: "body".to_string(),
                kind: DiffKind::Removed,
            });
        }
        (None, Some(_)) => {
            diffs.push(FieldDiff {
                field: "body".to_string(),
                kind: DiffKind::Added,
            });
        }
        _ => {}
    }

    diffs
}

/// Compare two sessions and return a structured diff result
pub fn diff_sessions(
    events_a: &[TrafficEvent],
    events_b: &[TrafficEvent],
    session_a_id: u64,
    session_b_id: u64,
) -> DiffResult {
    let requests_a = extract_requests(events_a);
    let requests_b = extract_requests(events_b);

    let mut added_requests = Vec::new();
    let mut removed_requests = Vec::new();
    let mut changed_requests = Vec::new();

    for (url, (req_b, _resp_b)) in &requests_b {
        if let Some((req_a, _resp_a)) = requests_a.get(url) {
            let diffs = compare_requests(req_a, req_b, url);
            if !diffs.is_empty() {
                changed_requests.push(RequestDiff {
                    session_id: session_b_id,
                    method: req_b.method.clone(),
                    url: req_b.url.clone(),
                    diffs,
                });
            }
        } else {
            added_requests.push(RequestDiff {
                session_id: session_b_id,
                method: req_b.method.clone(),
                url: req_b.url.clone(),
                diffs: vec![FieldDiff {
                    field: "request".to_string(),
                    kind: DiffKind::Added,
                }],
            });
        }
    }

    for (url, (req_a, _)) in &requests_a {
        if !requests_b.contains_key(url) {
            removed_requests.push(RequestDiff {
                session_id: session_a_id,
                method: req_a.method.clone(),
                url: req_a.url.clone(),
                diffs: vec![FieldDiff {
                    field: "request".to_string(),
                    kind: DiffKind::Removed,
                }],
            });
        }
    }

    let total_diffs = added_requests.len() + removed_requests.len() + changed_requests.len();

    DiffResult {
        session_a_id,
        session_b_id,
        added_requests,
        removed_requests,
        changed_requests,
        total_diffs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_core::models::{ConnectionKey, HttpVersion, SessionId, TransportProtocol};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn make_request(method: &str, url: &str, path: &str) -> HttpRequest {
        HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            path: path.to_string(),
            request_line_version: HttpVersion::Http11,
            headers: vec![("Host".to_string(), "example.com".to_string())],
            body: None,
            body_size: 0,
            timestamp: 0,
        }
    }

    fn make_conn() -> ConnectionKey {
        ConnectionKey {
            src_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            dst_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            protocol: TransportProtocol::Tcp,
            connection_id: 1,
            timestamp: 0,
            interface_guid: None,
        }
    }

    fn make_event(session_id: u64, req: HttpRequest) -> TrafficEvent {
        TrafficEvent::HttpRequest {
            session_id: SessionId(session_id),
            conn: make_conn(),
            request: req,
        }
    }

    #[test]
    fn test_diff_sessions_empty() {
        let result = diff_sessions(&[], &[], 1, 2);
        assert_eq!(result.total_diffs, 0);
        assert!(result.added_requests.is_empty());
        assert!(result.removed_requests.is_empty());
        assert!(result.changed_requests.is_empty());
    }

    #[test]
    fn test_diff_sessions_added() {
        let req = make_request("GET", "http://example.com/test", "/test");
        let events_b = vec![make_event(2, req)];
        let result = diff_sessions(&[], &events_b, 1, 2);
        assert_eq!(result.total_diffs, 1);
        assert_eq!(result.added_requests.len(), 1);
    }

    #[test]
    fn test_diff_sessions_removed() {
        let req = make_request("GET", "http://example.com/test", "/test");
        let events_a = vec![make_event(1, req)];
        let result = diff_sessions(&events_a, &[], 1, 2);
        assert_eq!(result.total_diffs, 1);
        assert_eq!(result.removed_requests.len(), 1);
    }

    #[test]
    fn test_diff_sessions_changed() {
        let req_a = make_request("GET", "http://example.com/test", "/test");
        let req_b = make_request("POST", "http://example.com/test", "/test");
        let events_a = vec![make_event(1, req_a)];
        let events_b = vec![make_event(2, req_b)];
        let result = diff_sessions(&events_a, &events_b, 1, 2);
        assert_eq!(result.total_diffs, 1);
        assert_eq!(result.changed_requests.len(), 1);
    }
}