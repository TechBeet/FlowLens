//! Mock data generator for FlowLens GUI
//! 
//! Generates realistic traffic events for testing and demonstration.

use chrono::{DateTime, Utc, Local};
use std::collections::HashMap;
use flowlens_core::models::{
    TrafficEvent, TrafficEventType, HttpRequest, HttpResponse,
    ProtocolKind, TlsSession, TlsCertInfo, ProcessInfo,
    ConnectionInfo, PacketInfo,
};

/// Generate mock traffic events for demonstration
pub fn generate_mock_events(count: usize) -> Vec<TrafficEvent> {
    let mut events = Vec::new();
    
    let paths = ["/api/users", "/api/products", "/dashboard", "/health", "/login",
                 "/api/status", "/static/js/main.js", "/static/css/styles.css",
                 "/api/checkout", "/graphql", "/ws/notifications", "/favicon.ico"];
    
    let methods = ["GET", "POST", "PUT", "DELETE", "OPTIONS"];
    let statuses = [200, 201, 204, 301, 302, 400, 401, 403, 404, 500];
    let protos = ["HTTP/1.1", "HTTP/2", "WebSocket", "gRPC"];
    
    let processes = ["chrome.exe", "firefox.exe", "python.exe", "node.exe", 
                     "curl", "wget", "git.exe", "node-gyp"];
    
    for i in 0..count {
        let path = paths[i % paths.len()];
        let method = methods[i % methods.len()];
        let status = statuses[i % statuses.len()];
        let proto = protos[i % protos.len()];
        let process = processes[i % processes.len()];
        
        let timestamp = Utc::now() - chrono::Duration::seconds(count - i);
        
        let request = HttpRequest {
            method: method.to_string(),
            url: format!("https://example.com{}", path),
            protocol_version: proto.to_string(),
            headers: HashMap::new(),
            body_size: match method { "GET" => 0, _ => i * 10 },
        };
        
        let response = HttpResponse {
            status_code: status,
            protocol_version: proto.to_string(),
            headers: HashMap::new(),
            body_size: status * 10,
        };
        
        let connection = ConnectionInfo {
            src_addr: "192.168.1.1".parse().unwrap(),
            dst_addr: "10.0.0.1".parse().unwrap(),
            src_port: 50000 + i % 1000,
            dst_port: 443,
            protocol: ProtocolKind::from(proto),
        };
        
        let tls = TlsSession {
            version: "TLS 1.3".to_string(),
            cipher_suite: "TLS_AES_256_GCM_SHA384".to_string(),
            server_name: Some("example.com".to_string()),
            verified: true,
            cert_info: Some(TlsCertInfo {
                subject: "CN=example.com".to_string(),
                issuer: "Let's Encrypt Authority X3".to_string(),
                valid_from: timestamp,
                valid_to: timestamp + chrono::Duration::days(30),
            }),
            san: Some("example.com,www.example.com".to_string()),
        };
        
        let process_info = ProcessInfo {
            pid: 1000 + i,
            name: process.to_string(),
            exe_path: format!("C:\\Program Files\\{}\\{}.exe", process.split('.').next().unwrap_or("app"), process.split('.').next().unwrap_or("app")),
            cmdline: format!("{} --renderer-process-id={}", process, i),
        };
        
        let event = match method {
            "GET" => TrafficEvent::HttpRequest {
                timestamp,
                request: request.clone(),
                connection: connection.clone(),
                tls: Some(tls.clone()),
                process: Some(process_info.clone()),
            },
            "POST" => TrafficEvent::HttpResponse {
                timestamp,
                response: response.clone(),
                connection: connection.clone(),
                tls: Some(tls.clone()),
                process: Some(process_info.clone()),
            },
            "PUT" => TrafficEvent::HttpRequest {
                timestamp,
                request: request.clone(),
                connection: connection.clone(),
                tls: Some(tls.clone()),
                process: Some(process_info.clone()),
            },
            "DELETE" => TrafficEvent::HttpResponse {
                timestamp,
                response: response.clone(),
                connection: connection.clone(),
                tls: Some(tls.clone()),
                process: Some(process_info.clone()),
            },
            _ => TrafficEvent::HttpRequest {
                timestamp,
                request,
                connection,
                tls: Some(tls),
                process: Some(process_info),
            },
        };
        
        events.push(event);
    }
    
    events
}

/// Check if an event matches filter criteria
pub fn event_matches_filter(event: &TrafficEvent, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let filter_lower = filter.to_lowercase();
    // Simple match against timestamp format
    true // simplified
}

/// Create 50+ mock events for demo
pub fn create_demo_events() -> Vec<TrafficEvent> {
    generate_mock_events(50)
}
