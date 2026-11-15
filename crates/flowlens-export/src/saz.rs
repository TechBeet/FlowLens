//! SAZ (Fiddler session archive) importer for FlowLens.

use anyhow::{Context, Result};
use flowlens_core::models::{
    ConnectionKey, HttpRequest, HttpResponse, HttpVersion, MatchQuality, ProcessArch, ProcessInfo,
    ProtocolKind, SessionId, TrafficSession, TransportProtocol,
};
use std::io::Read;
use std::path::Path;

pub struct SazImporter;

impl SazImporter {
    pub fn new() -> Self {
        Self
    }

    pub fn import(&self, path: &Path) -> Result<Vec<TrafficSession>> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("failed to open SAZ file: {}", path.display()))?;
        let mut archive = zip::ZipArchive::new(file).context("failed to read SAZ zip archive")?;

        let mut client_indices: Vec<usize> = Vec::new();
        for i in 0..archive.len() {
            let name = archive.by_index(i)?.name().to_string();
            if name.starts_with("raw/") && name.ends_with("_c.txt") {
                client_indices.push(i);
            }
        }
        client_indices.sort();

        let mut sessions = Vec::with_capacity(client_indices.len());

        for &idx in &client_indices {
            let client_name = archive.by_index(idx)?.name().to_string();
            let mut client_raw = String::new();
            {
                let mut entry = archive.by_index(idx)?;
                entry.read_to_string(&mut client_raw)?;
            }

            let (req, method, url) = parse_client_request(&client_raw);

            let server_name = client_name.replace("_c.txt", "_s.txt");
            let mut resp_raw = String::new();
            for i in 0..archive.len() {
                if archive.by_index(i)?.name() == server_name {
                    let mut entry = archive.by_index(i)?;
                    entry.read_to_string(&mut resp_raw)?;
                    break;
                }
            }

            let resp = parse_server_response(&resp_raw);

            let session = TrafficSession {
                session_id: SessionId(sessions.len() as u64 + 1),
                conn: ConnectionKey {
                    src_addr: "0.0.0.0:0".parse().unwrap(),
                    dst_addr: "0.0.0.0:0".parse().unwrap(),
                    protocol: TransportProtocol::Tcp,
                    connection_id: sessions.len() as u64 + 1,
                    timestamp: 0,
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
                started_at: 0,
                ended_at: None,
                bytes_sent: 0,
                bytes_received: 0,
            };

            let _ = (method, url);
            sessions.push(session);
        }

        Ok(sessions)
    }
}

impl Default for SazImporter {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_client_request(raw: &str) -> (HttpRequest, String, String) {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut parsed = httparse::Request::new(&mut headers);

    let mut method = String::new();
    let mut url = String::new();
    let mut version = HttpVersion::Http11;
    let mut headers_vec: Vec<(String, String)> = Vec::new();

    if let Ok(httparse::Status::Complete(_)) = parsed.parse(raw.as_bytes()) {
        method = parsed.method.unwrap_or("").to_string();
        url = parsed.path.unwrap_or("").to_string();
        version = match parsed.version {
            Some(0) => HttpVersion::Http10,
            Some(1) => HttpVersion::Http11,
            _ => HttpVersion::Unknown,
        };
        headers_vec = parsed
            .headers
            .iter()
            .map(|h| {
                (
                    h.name.to_string(),
                    h.value.iter().map(|&b| b as char).collect::<String>(),
                )
            })
            .collect();
    }

    let req = HttpRequest {
        method: method.clone(),
        url: url.clone(),
        path: url.split('?').next().unwrap_or("").to_string(),
        request_line_version: version,
        headers: headers_vec,
        body: None,
        body_size: 0,
        timestamp: 0,
    };

    (req, method, url)
}

fn parse_server_response(raw: &str) -> HttpResponse {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut parsed = httparse::Response::new(&mut headers);

    let mut status = 0u16;
    let mut version = HttpVersion::Http11;

    if let Ok(httparse::Status::Complete(_)) = parsed.parse(raw.as_bytes()) {
        status = parsed.code.unwrap_or(0);
        version = match parsed.version {
            Some(0) => HttpVersion::Http10,
            Some(1) => HttpVersion::Http11,
            _ => HttpVersion::Unknown,
        };
    }

    let parsed_headers: Vec<(String, String)> = parsed
        .headers
        .iter()
        .map(|h| {
            (
                h.name.to_string(),
                h.value.iter().map(|&b| b as char).collect::<String>(),
            )
        })
        .collect();

    HttpResponse {
        status_code: status,
        status_text: reason_phrase(status).to_string(),
        version,
        headers: parsed_headers,
        body: None,
        body_size: 0,
        timestamp: 0,
    }
}

fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a temporary SAZ archive with the given raw client/server pairs.
    fn build_saz(path: &Path, pairs: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        for (i, (client, server)) in pairs.iter().enumerate() {
            let name = format!("raw/{:02}_c.txt", i + 1);
            zip.start_file(name, options).unwrap();
            zip.write_all(client.as_bytes()).unwrap();

            let name = format!("raw/{:02}_s.txt", i + 1);
            zip.start_file(name, options).unwrap();
            zip.write_all(server.as_bytes()).unwrap();

            let name = format!("raw/{:02}_m.xml", i + 1);
            zip.start_file(name, options).unwrap();
            zip.write_all(b"<Session><SessionTimers ClientConnected=\"0\" /></Session>")
                .unwrap();
        }
        zip.finish().unwrap();
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("flowlens_test_{}.saz", name))
    }

    #[test]
    fn test_parse_client_request() {
        let raw = "GET /api/v1/users?page=2 HTTP/1.1\r\nHost: example.com\r\nAccept: */*\r\n\r\n";
        let (req, method, url) = parse_client_request(raw);
        assert_eq!(method, "GET");
        assert_eq!(url, "/api/v1/users?page=2");
        assert_eq!(req.path, "/api/v1/users");
        assert_eq!(req.request_line_version, HttpVersion::Http11);
        assert!(req.headers.iter().any(|(k, v)| k == "Host" && v == "example.com"));
    }

    #[test]
    fn test_parse_server_response() {
        let raw = "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\n\r\n";
        let resp = parse_server_response(raw);
        assert_eq!(resp.status_code, 404);
        assert_eq!(resp.status_text, "Not Found");
        assert_eq!(resp.version, HttpVersion::Http11);
        assert!(resp
            .headers
            .iter()
            .any(|(k, v)| k == "Content-Type" && v == "text/html"));
    }

    #[test]
    fn test_import_saz_file() {
        let path = temp_path("basic");
        build_saz(
            &path,
            &[(
                "GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello",
            )],
        );

        let sessions = SazImporter::new().import(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let req = sessions[0].http_request.as_ref().unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.url, "/index.html");
        let resp = sessions[0].http_response.as_ref().unwrap();
        assert_eq!(resp.status_code, 200);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_import_multiple_sessions() {
        let path = temp_path("multi");
        build_saz(
            &path,
            &[
                (
                    "GET /a HTTP/1.1\r\nHost: a.com\r\n\r\n",
                    "HTTP/1.1 200 OK\r\n\r\n",
                ),
                (
                    "POST /b HTTP/1.1\r\nHost: b.com\r\n\r\n",
                    "HTTP/1.1 500 Internal Server Error\r\n\r\n",
                ),
            ],
        );

        let sessions = SazImporter::new().import(&path).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(
            sessions[0].http_request.as_ref().unwrap().method,
            "GET"
        );
        assert_eq!(
            sessions[1].http_response.as_ref().unwrap().status_code,
            500
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_import_empty_saz() {
        let path = temp_path("empty");
        build_saz(&path, &[]);
        let sessions = SazImporter::new().import(&path).unwrap();
        assert!(sessions.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_import_missing_file_returns_error() {
        let path = temp_path("missing");
        let result = SazImporter::new().import(&path);
        assert!(result.is_err());
    }
}
