//! HAR 1.2 exporter/importer for FlowLens

use chrono::TimeZone;
use flowlens_core::models::{
    ConnectionKey, HttpVersion, HttpRequest, HttpResponse, MatchQuality, ProcessArch, ProcessInfo,
    ProtocolKind, SessionId, TrafficRow, TrafficSession, TransportProtocol,
};
use serde::{Deserialize, Serialize};

fn format_datetime(ts: i64) -> String {
    let secs = ts / 1_000_000;
    let nsecs = ((ts % 1_000_000) * 1000) as u32;
    let dt = chrono::Utc
        .timestamp_opt(secs, nsecs)
        .single()
        .unwrap_or_default();
    dt.format("%Y-%m-%dT%H:%M:%S.%3fZ").to_string()
}

fn duration_ms(start: i64, end: i64) -> i64 {
    if start > 0 && end > start {
        (end - start) / 1000
    } else {
        0
    }
}

fn headers_to_har(headers: &[(String, String)]) -> Vec<HareHeader> {
    headers
        .iter()
        .map(|(k, v)| HareHeader {
            name: k.clone(),
            value: v.clone(),
            comment: None,
        })
        .collect()
}

fn extract_query(url: &str) -> Vec<HareQueryString> {
    if !url.contains('?') {
        return vec![];
    }
    let (_, query) = url.split_once('?').unwrap_or(("", url));
    query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some(HareQueryString {
                name: k.to_string(),
                value: v.to_string(),
            })
        })
        .collect()
}

fn content_type(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.split(';').next().unwrap_or(v).trim().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

// ── HAR types ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct HarLog {
    pub log: Log,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    pub version: String,
    pub creator: Creator,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Creator {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    #[serde(rename = "startedDateTime")]
    pub started_datetime: String,
    #[serde(rename = "time")]
    pub time_ms: i64,
    pub request: Request,
    pub response: Response,
    pub cache: Cache,
    pub timings: Timings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub url: String,
    #[serde(rename = "httpVersion")]
    pub http_version: String,
    pub headers: Vec<HareHeader>,
    #[serde(rename = "queryString")]
    pub query_string: Vec<HareQueryString>,
    #[serde(rename = "headersSize")]
    pub headers_size: i64,
    #[serde(rename = "bodySize")]
    pub body_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HareHeader {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HareQueryString {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub status: i32,
    #[serde(rename = "statusText")]
    pub status_text: String,
    #[serde(rename = "httpVersion")]
    pub http_version: String,
    pub headers: Vec<HareHeader>,
    pub content: Content,
    #[serde(rename = "headersSize")]
    pub headers_size: i64,
    #[serde(rename = "bodySize")]
    pub body_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub size: i64,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cache {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Timings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect: Option<i64>,
    pub send: i64,
    pub wait: i64,
    pub receive: i64,
}

// ── Main implementation ─────────────────────────────────────────────────────

/// Render an HttpVersion as the HAR `httpVersion` string.
fn version_str(v: &HttpVersion) -> String {
    match v {
        HttpVersion::Http10 => "HTTP/1.0".to_string(),
        HttpVersion::Http11 => "HTTP/1.1".to_string(),
        HttpVersion::Http2 => "HTTP/2".to_string(),
        HttpVersion::Http3 => "HTTP/3".to_string(),
        HttpVersion::Unknown => "HTTP/0.9".to_string(),
    }
}

pub struct HarExporter;

impl HarExporter {
    pub fn new() -> Self {
        Self
    }

    /// Export traffic sessions to a HAR 1.2 JSON string.
    pub fn export_sessions(sessions: &[TrafficSession]) -> Result<String, serde_json::Error> {
        let mut entries = Vec::new();

        for s in sessions {
            let req = match &s.http_request {
                Some(r) => r,
                None => continue,
            };
            let resp = match &s.http_response {
                Some(r) => r,
                None => continue,
            };

            let end_ts = s.ended_at.unwrap_or(s.started_at);
            let entry_time = duration_ms(s.started_at, end_ts);

            let (resp_text, resp_size) = match &resp.body {
                Some(b) if !b.is_empty() => {
                    let text: String = String::from_utf8_lossy(b).chars().take(5000).collect();
                    let len = text.len() as i64;
                    (text, len)
                }
                _ => (String::new(), 0),
            };

            entries.push(Entry {
                started_datetime: format_datetime(s.started_at),
                time_ms: entry_time,
                request: Request {
                    method: req.method.clone(),
                    url: req.url.clone(),
                    http_version: version_str(&req.request_line_version),
                    headers: headers_to_har(&req.headers),
                    query_string: extract_query(&req.url),
                    headers_size: req.body_size as i64,
                    body_size: req.body_size as i64,
                },
                response: Response {
                    status: resp.status_code as i32,
                    status_text: resp.status_text.clone(),
                    http_version: version_str(&resp.version),
                    headers: headers_to_har(&resp.headers),
                    content: Content {
                        size: resp_size,
                        mime_type: content_type(&resp.headers),
                        text: if !resp_text.is_empty() {
                            Some(resp_text)
                        } else {
                            None
                        },
                    },
                    headers_size: resp.body_size as i64,
                    body_size: resp.body_size as i64,
                },
                cache: Cache {},
                timings: Timings {
                    dns: None,
                    connect: None,
                    send: 0,
                    wait: entry_time,
                    receive: 0,
                },
            });
        }

        let log = Log {
            version: "1.2".to_string(),
            creator: Creator {
                name: "FlowLens".to_string(),
                version: "0.1.0".to_string(),
            },
            entries,
        };

        serde_json::to_string_pretty(&HarLog { log })
    }

    /// Export flattened traffic rows to a HAR 1.2 JSON string.
    pub fn export_rows(rows: &[TrafficRow]) -> Result<String, serde_json::Error> {
        let mut sessions = Vec::with_capacity(rows.len());
        for r in rows {
            let status_code: u16 = r.status.parse().unwrap_or(0);
            let req = flowlens_core::models::HttpRequest {
                method: r.method.clone(),
                url: r.path.clone(),
                path: r.path.clone(),
                request_line_version: HttpVersion::Http11,
                headers: vec![],
                body: None,
                body_size: 0,
                timestamp: r.timestamp,
            };
            let resp = flowlens_core::models::HttpResponse {
                status_code,
                status_text: String::new(),
                version: HttpVersion::Http11,
                headers: vec![],
                body: None,
                body_size: r.body_size,
                timestamp: r.timestamp,
            };
            sessions.push(TrafficSession {
                session_id: flowlens_core::models::SessionId(0),
                conn: flowlens_core::models::ConnectionKey {
                    src_addr: r
                        .src_addr
                        .parse()
                        .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                    dst_addr: r
                        .dst_addr
                        .parse()
                        .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                    protocol: flowlens_core::models::TransportProtocol::Tcp,
                    connection_id: 0,
                    timestamp: r.timestamp,
                    interface_guid: None,
                },
                process: flowlens_core::models::ProcessInfo {
                    pid: 0,
                    name: "export".to_string(),
                    exe_path: None,
                    user_sid: String::new(),
                    arch: flowlens_core::models::ProcessArch::X64,
                    match_quality: flowlens_core::models::MatchQuality::Unspecified,
                },
                tls: None,
                protocol: flowlens_core::models::ProtocolKind::Http1,
                http_request: Some(req),
                http_response: Some(resp),
                websocket_frames: vec![],
                sse_events: vec![],
                grpc_call: None,
                started_at: r.timestamp,
                ended_at: Some(r.timestamp),
                bytes_sent: 0,
                bytes_received: r.body_size as u64,
            });
        }
        Self::export_sessions(&sessions)
    }

    /// Export to a file.
    pub fn export_to_file(
        &self,
        sessions: &[TrafficSession],
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let content = Self::export_sessions(sessions)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl Default for HarExporter {
    fn default() -> Self {
        Self::new()
    }
}

// ── HAR import ────────────────────────────────────────────────────────────

/// Parse an RFC 3339 / ISO 8601 timestamp into microseconds since the Unix epoch.
fn parse_har_datetime(s: &str) -> i64 {
    match chrono::DateTime::parse_from_rfc3339(s) {
        Ok(dt) => dt.timestamp_micros(),
        Err(_) => 0,
    }
}

/// Map a HAR `httpVersion` string back to the `HttpVersion` enum.
fn version_from_str(s: &str) -> HttpVersion {
    match s.to_ascii_uppercase().as_str() {
        "HTTP/1.0" => HttpVersion::Http10,
        "HTTP/1.1" => HttpVersion::Http11,
        "HTTP/2" => HttpVersion::Http2,
        "HTTP/3" => HttpVersion::Http3,
        _ => HttpVersion::Unknown,
    }
}

/// Reconstruct an `HttpRequest` from a HAR request entry.
fn request_from_har(req: &Request, started_at: i64) -> HttpRequest {
    let headers: Vec<(String, String)> = req
        .headers
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect();
    let path = req
        .url
        .split('?')
        .next()
        .unwrap_or(&req.url)
        .to_string();
    HttpRequest {
        method: req.method.clone(),
        url: req.url.clone(),
        path,
        request_line_version: version_from_str(&req.http_version),
        headers,
        body: None,
        body_size: req.body_size.max(0) as usize,
        timestamp: started_at,
    }
}

/// Reconstruct an `HttpResponse` from a HAR response entry.
fn response_from_har(resp: &Response, started_at: i64) -> HttpResponse {
    let headers: Vec<(String, String)> = resp
        .headers
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect();
    let body: Option<Vec<u8>> = resp
        .content
        .text
        .as_ref()
        .map(|t| t.as_bytes().to_vec());
    let body_size = body.as_ref().map(|b| b.len()).unwrap_or(0);
    HttpResponse {
        status_code: resp.status.max(0) as u16,
        status_text: resp.status_text.clone(),
        version: version_from_str(&resp.http_version),
        headers,
        body,
        body_size,
        timestamp: started_at,
    }
}

/// HAR 1.2 importer: reconstructs `TrafficSession`s from a HAR document.
pub struct HarImporter;

impl HarImporter {
    pub fn new() -> Self {
        Self
    }

    /// Import traffic sessions from a HAR 1.2 JSON string.
    pub fn import_str(json: &str) -> Result<Vec<TrafficSession>, serde_json::Error> {
        let har: HarLog = serde_json::from_str(json)?;
        let mut sessions = Vec::with_capacity(har.log.entries.len());
        for (i, entry) in har.log.entries.into_iter().enumerate() {
            let started_at = parse_har_datetime(&entry.started_datetime);
            let ended_at = if entry.time_ms > 0 {
                started_at + entry.time_ms * 1000
            } else {
                started_at
            };
            let req = request_from_har(&entry.request, started_at);
            let resp = response_from_har(&entry.response, started_at);
            let bytes_sent = req.body_size as u64;
            let bytes_received = resp.body_size as u64;
            sessions.push(TrafficSession {
                session_id: SessionId(i as u64 + 1),
                conn: ConnectionKey {
                    src_addr: "0.0.0.0:0".parse().unwrap(),
                    dst_addr: "0.0.0.0:0".parse().unwrap(),
                    protocol: TransportProtocol::Tcp,
                    connection_id: i as u64 + 1,
                    timestamp: started_at,
                    interface_guid: None,
                },
                process: ProcessInfo {
                    pid: 0,
                    name: "imported".to_string(),
                    exe_path: None,
                    user_sid: String::new(),
                    arch: ProcessArch::X64,
                    match_quality: MatchQuality::Unspecified,
                },
                tls: None,
                protocol: ProtocolKind::Http1,
                http_request: Some(req),
                http_response: Some(resp),
                websocket_frames: vec![],
                sse_events: vec![],
                grpc_call: None,
                started_at,
                ended_at: Some(ended_at),
                bytes_sent,
                bytes_received,
            });
        }
        Ok(sessions)
    }

    /// Import traffic sessions from a HAR file on disk.
    pub fn import_file(path: &std::path::Path) -> anyhow::Result<Vec<TrafficSession>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::import_str(&content)?)
    }
}

impl Default for HarImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlens_core::models::ConnectionKey;
    use flowlens_core::models::HttpRequest;
    use flowlens_core::models::HttpResponse;
    use flowlens_core::models::ProcessInfo;
    use flowlens_core::models::ProtocolKind;
    use flowlens_core::models::SessionId;
    use flowlens_core::models::TransportProtocol;

    fn test_session(method: &str, url: &str, status: u16) -> TrafficSession {
        TrafficSession {
            session_id: SessionId(1),
            conn: ConnectionKey {
                src_addr: "127.0.0.1:12345".parse().unwrap(),
                dst_addr: "93.184.216.34:443".parse().unwrap(),
                protocol: TransportProtocol::Tcp,
                connection_id: 1,
                timestamp: 0,
                interface_guid: None,
            },
            process: ProcessInfo {
                pid: 1234,
                name: "test.exe".to_string(),
                exe_path: None,
                user_sid: String::new(),
                arch: flowlens_core::models::ProcessArch::X64,
                match_quality: flowlens_core::models::MatchQuality::Heuristic,
            },
            tls: None,
            protocol: ProtocolKind::Http1,
            http_request: Some(HttpRequest {
                method: method.to_string(),
                url: url.to_string(),
                path: "/".to_string(),
                request_line_version: HttpVersion::Http11,
                headers: vec![("Host".to_string(), "example.com".to_string())],
                body: None,
                body_size: 0,
                timestamp: 0,
            }),
            http_response: Some(HttpResponse {
                status_code: status,
                status_text: "OK".to_string(),
                version: HttpVersion::Http11,
                headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                body: Some(b"{}".to_vec()),
                body_size: 2,
                timestamp: 0,
            }),
            websocket_frames: vec![],
            sse_events: vec![],
            grpc_call: None,
            started_at: 1700000000000000, // microseconds
            ended_at: Some(1700000001000000),
            bytes_sent: 100,
            bytes_received: 200,
        }
    }

    #[test]
    fn test_export_empty() {
        let result = HarExporter::export_sessions(&[]).unwrap();
        assert!(result.contains("\"version\": \"1.2\""));
        assert!(result.contains("\"entries\": []"));
    }

    #[test]
    fn test_export_single_session() {
        let sessions = vec![test_session("GET", "https://example.com/api?foo=bar", 200)];
        let result = HarExporter::export_sessions(&sessions).unwrap();
        let har: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(har["log"]["version"], "1.2");
        assert_eq!(har["log"]["creator"]["name"], "FlowLens");
        assert_eq!(har["log"]["entries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_export_request_fields() {
        let sessions = vec![test_session("POST", "https://example.com/api?foo=bar", 201)];
        let result = HarExporter::export_sessions(&sessions).unwrap();
        let har: serde_json::Value = serde_json::from_str(&result).unwrap();
        let entry = &har["log"]["entries"][0];
        assert_eq!(entry["request"]["method"], "POST");
        assert_eq!(entry["request"]["url"], "https://example.com/api?foo=bar");
        assert_eq!(entry["request"]["httpVersion"], "HTTP/1.1");
        assert_eq!(entry["request"]["queryString"][0]["name"], "foo");
        assert_eq!(entry["request"]["queryString"][0]["value"], "bar");
    }

    #[test]
    fn test_export_response_fields() {
        let sessions = vec![test_session("GET", "https://example.com/", 404)];
        let result = HarExporter::export_sessions(&sessions).unwrap();
        let har: serde_json::Value = serde_json::from_str(&result).unwrap();
        let resp = &har["log"]["entries"][0]["response"];
        assert_eq!(resp["status"], 404);
        assert_eq!(resp["content"]["mimeType"], "application/json");
        assert_eq!(resp["content"]["text"], "{}");
    }

    #[test]
    fn test_export_skips_sessions_without_http() {
        let mut s = test_session("GET", "https://example.com/", 200);
        s.http_request = None;
        let result = HarExporter::export_sessions(&[s]).unwrap();
        let har: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(har["log"]["entries"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_export_to_file() {
        let sessions = vec![test_session("GET", "https://example.com/", 200)];
        let tmp = std::env::temp_dir().join("flowlens_test.har");
        HarExporter::new().export_to_file(&sessions, &tmp).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("\"version\": \"1.2\""));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_import_roundtrip() {
        // Export a session, then import it back and verify the fields survive.
        let sessions = vec![test_session("GET", "https://example.com/api?foo=bar", 200)];
        let json = HarExporter::export_sessions(&sessions).unwrap();
        let imported = HarImporter::import_str(&json).unwrap();
        assert_eq!(imported.len(), 1);
        let req = imported[0].http_request.as_ref().unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.url, "https://example.com/api?foo=bar");
        assert_eq!(req.path, "https://example.com/api");
        let resp = imported[0].http_response.as_ref().unwrap();
        assert_eq!(resp.status_code, 200);
        assert!(resp.body.is_some());
    }

    #[test]
    fn test_import_minimal_har() {
        let json = r#"{
            "log": {
                "version": "1.2",
                "creator": { "name": "test", "version": "1.0" },
                "entries": [
                    {
                        "startedDateTime": "2024-01-01T00:00:00.000Z",
                        "time": 42,
                        "request": {
                            "method": "POST",
                            "url": "https://api.example.com/v1/items",
                            "httpVersion": "HTTP/2",
                            "headers": [{ "name": "content-type", "value": "application/json" }],
                            "queryString": [],
                            "headersSize": 0,
                            "bodySize": 10
                        },
                        "response": {
                            "status": 201,
                            "statusText": "Created",
                            "httpVersion": "HTTP/2",
                            "headers": [],
                            "content": { "size": 2, "mimeType": "application/json", "text": "{}" },
                            "headersSize": 0,
                            "bodySize": 2
                        },
                        "cache": {},
                        "timings": { "send": 0, "wait": 42, "receive": 0 }
                    }
                ]
            }
        }"#;
        let imported = HarImporter::import_str(json).unwrap();
        assert_eq!(imported.len(), 1);
        let req = imported[0].http_request.as_ref().unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.request_line_version, HttpVersion::Http2);
        assert_eq!(req.headers[0].0, "content-type");
        let resp = imported[0].http_response.as_ref().unwrap();
        assert_eq!(resp.status_code, 201);
        assert_eq!(resp.body.as_deref(), Some(b"{}".as_slice()));
        assert_eq!(imported[0].started_at, 1704067200000000);
    }

    #[test]
    fn test_import_invalid_json_errors() {
        assert!(HarImporter::import_str("not json").is_err());
    }
}
