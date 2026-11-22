# Implementation Plan: Full Open-Source Replication of HTTPDebuggerPro

## Overview

This plan describes a complete open-source reimplementation of HTTPDebuggerPro, a Windows-based HTTP/HTTPS debugging proxy tool. The original application captures system-wide HTTP/HTTPS traffic through a kernel driver, decrypts HTTPS via MITM with a local CA certificate, and presents captured transactions in a rich desktop UI. The replication targets the same feature set: system-wide traffic capture, HTTPS decryption, transaction inspection, filtering, modification, auto-responder, charts, export/import, WebSocket/gRPC/SSE support, and an MCP server for AI integration.

The architecture follows a multi-process design: a kernel driver for traffic interception, a Windows service for protocol filtering and TLS decryption, a desktop UI for visualization, and an MCP server for programmatic access. The technology stack uses Rust for performance-critical components and Tauri + React for the UI.

## Types

### Core Data Types

- **HttpTransaction**: id: u64, request: HttpRequest, response: Option<HttpResponse>, timing: TransactionTiming, outcome: TransactionOutcome {Complete, ShortBody, NoResponse, StreamClosed}, has_http_error: bool, has_connection_error: bool, websocket_close: Option<WebSocketCloseInfo>, sse_captured_bytes: u64, websocket_bytes_on_wire: u64, grpc_messages: Vec<GrpcMessage>, grpc_message_count: u32, grpc_encoding: String, grpc_accept_encoding: String, application: String, host: String, request_raw_size: u64, request_unzip_size: u64, response_raw_size: u64, response_unzip_size: u64, request_message_count: u32, response_message_count: u32

- **HttpBody**: content: Vec<u8>, is_text: bool, text_encoding: Option<String>, encoding_source: Option<EncodingSource>, declared_charset_unusable: bool, text_lossy: bool, body_source: BodySource {Wire, Reconstructed}, complete: bool, decode_failed: bool, payload_base64: bool

- **HttpHeader**: name: String, value: String

- **FilterRule**: id: u64, name: String, enabled: bool, matcher: RuleMatcher {method, host, url_contains, status, header_match}, action: RuleAction {Block, Redirect{target_url}, Modify{modifications}, AutoRespond{response}, CaptureOnly}, profile_id: u64

- **AutoResponse**: status: u16, headers: Vec<HttpHeader>, body: Vec<u8>, delay_ms: u32

- **HighlightingRule**: id: u64, name: String, enabled: bool, matcher: RuleMatcher, color: String, bold: bool

- **ChartConfig**: chart_type: ChartType {Domains, Durations, Sizes, Timeline, Types}, time_range: TimeRange, filters: Vec<FilterConfig>

- **RulesProfile**: id: u64, name: String, rules: Vec<FilterRule>, highlighting_rules: Vec<HighlightingRule>, auto_responder_rules: Vec<AutoResponderRule>

- **McpToolRequest**: tool_name: String, arguments: serde_json::Value, request_id: String

- **McpToolResponse**: request_id: String, result: serde_json::Value, is_error: bool

- **SessionStats**: transaction_count: u64, websocket_count: u64, sse_count: u64, status_classes: HashMap<u16, u64>, hosts: Vec<HostStats>, methods: HashMap<String, u64>, total_request_bytes: u64, total_response_bytes: u64, average_duration_ms: f64

- **Endpoint**: host: String, method: String, path_template: String, transaction_count: u64, average_duration_ms: f64

- **DiagnosisResult**: findings: Vec<DiagnosisFinding> where DiagnosisFinding {category, severity, description, hint}

- **CaptureStatus**: capturing: bool, driver_running: bool, session_transaction_count: u64, undecrypted_hosts: Vec<String>, filter_rules_dropped: u64
## Files

### New Files to Create

#### Rust Workspace Root
- `Cargo.toml` - Workspace manifest
- `.cargo/config.toml` - Cargo configuration

#### Core Crates
- `crates/flowlens-core/` - Core data types, traits, shared utilities
- `crates/flowlens-protocol/` - HTTP/HTTPS protocol parsing and filtering
- `crates/flowlens-capture/` - Traffic capture orchestration
- `crates/flowlens-storage/` - Persistent storage (SQLite, export/import)
- `crates/flowlens-tls/` - TLS certificate management (OpenSSL)
- `crates/flowlens-mcp/` - MCP server implementation
- `crates/flowlens-service/` - Windows service entry point

#### UI Application (Tauri)
- `ui/` - Tauri + React frontend
- `ui/src-tauri/` - Tauri backend
- `ui/src/` - React components

#### Kernel Driver
- `driver/` - WFP callout driver (C)

#### Build & Deploy
- `scripts/` - Build, install, uninstall scripts

### Existing Files to Modify
- `README.md` - Project documentation
- `.gitignore` - Add Rust/Tauri/Node ignores

## Functions

### Core Functions (flowlens-core)
- new_transaction(request) -> HttpTransaction
- update_response(&mut self, response)
- calculate_duration(&self) -> u64
- is_websocket(&self) -> bool
- is_grpc(&self) -> bool
- is_sse(&self) -> bool

### Protocol Functions (flowlens-protocol)
- parse_http_request(data) -> Result<HttpRequest, ParseError>
- parse_http_response(data) -> Result<HttpResponse, ParseError>
## Classes

### Rust Structs/Enums
HttpTransaction, HttpRequest, HttpResponse, HttpBody, HttpHeader
FilterRule, HighlightingRule, AutoResponderRule, RulesProfile
Session, Endpoint, SessionStats
McpServer, CaptureEngine, DriverInterface, CertManager, StorageManager

### TypeScript (UI)
Transaction, FilterRule, ChartConfig interfaces
TrafficStore, SettingsStore, ApiService classes

## Dependencies

### Rust Dependencies
serde, serde_json, tokio, thiserror, anyhow, tracing, http, httparse, h2, tokio-tungstenite, prost, rustls, rustls-pemfile, openssl, sqlx, rusqlite, windows, windows-service, jsonrpc-core, tauri, tauri-plugin-shell

### Node.js Dependencies (UI)
react, react-dom, zustand, @tanstack/react-table, recharts, monaco-editor, tailwindcss, @tauri-apps/api, @tauri-apps/plugin-shell, typescript, vite, @vitejs/plugin-react

### External Dependencies
- OpenSSL 3.5+ (TLS/SSL)
- SQLite 3.45+ (Database)
- Windows Driver Kit (WDK) (Kernel driver)
- WinDivert (Open-source traffic interception)

## Testing

### Unit Tests
transaction_test.rs, http_parser_test.rs, filter_engine_test.rs, websocket_test.rs, grpc_test.rs, export_test.rs, import_test.rs, cert_test.rs

### Integration Tests
capture_test.rs, mcp_test.rs, export_import_roundtrip.rs

### UI Tests
TrafficTable.test.tsx, TransactionDetail.test.tsx, FilterRules.test.tsx

### Manual Verification
1. Install CA certificate, verify HTTPS decryption
2. Capture HTTP/HTTPS traffic from browser
3. Verify transaction details (headers, body, timing)
4. Test filter rules (block, redirect, modify, auto-respond)
5. Test export (HAR, SAZ, cURL, JSON, XML, TSV)
6. Test import (HAR, SAZ)
7. Test WebSocket, gRPC, SSE capture
8. Test MCP server tools
9. Test charts and statistics

## Implementation Order

1. Project Setup - Rust workspace, Tauri UI, build system
2. Core Types (flowlens-core) - Data types, MCP protocol types
3. Protocol Parsing (flowlens-protocol) - HTTP/1.x, HTTP/2, WebSocket, gRPC, SSE
4. TLS/SSL (flowlens-tls) - CA cert, per-host certs, TLS decryption
5. Traffic Capture (flowlens-capture) - Driver interface, capture engine
6. Storage (flowlens-storage) - SQLite, export/import
7. Filter Engine (flowlens-protocol) - Rule evaluation, modification, auto-responder
8. Windows Service (flowlens-service) - Service control, IPC server
9. MCP Server (flowlens-mcp) - Named pipe server, all MCP tools
10. UI Application (Tauri + React) - Main window, traffic table, detail view, charts
11. Kernel Driver - WFP callout driver, traffic interception
12. Integration & Testing - End-to-end, UI tests, manual verification
13. Documentation & Packaging - User docs, installer, uninstaller

## Notes & Constraints

- Windows-only kernel driver: WFP-based, Windows-only. Use userspace proxy on non-Windows.
- OpenSSL dependency: Original uses OpenSSL 3.5.6. Rust openssl crate provides bindings.
- NetFilterSDK alternative: Use WinDivert (open-source) or implement WFP callouts directly.
- MFC to Tauri: Original UI is MFC-based. Replication uses Tauri + React.
- MCP integration: Maintain same MCP tool interface for AI assistant compatibility.
- Performance: Use async I/O and efficient data structures.
- Memory management: Stream large bodies to disk.
- Thread safety: IPC must be thread-safe and handle disconnections.

## Verification Checklist

- [ ] Kernel driver loads and intercepts traffic
- [ ] HTTPS decryption works with installed CA certificate
- [ ] HTTP/1.x, HTTP/2, WebSocket, gRPC, SSE traffic captured
- [ ] Transaction list displays correctly
- [ ] Transaction detail shows headers and body
- [ ] Filter rules work (block, redirect, modify, auto-respond)
- [ ] Export works (HAR, SAZ, cURL, JSON, XML, TSV)
- [ ] Import works (HAR, SAZ)
- [ ] Search works across transactions
- [ ] Charts display correctly
- [ ] MCP server responds to all tools
- [ ] Settings persist across restarts
- [ ] Undecrypted traffic is detected and reported
- [ ] Performance is acceptable under load
- [ ] No memory leaks during extended capture
- decode_body(body, headers) -> HttpBody
- evaluate_filter_rules(transaction, rules) -> FilterAction
- apply_modifications(transaction, modifications)
- generate_auto_response(rule, request) -> HttpResponse
- decrypt_tls(data, cert) -> Result<Vec<u8>, TlsError>
- parse_websocket_frame(data) -> Result<WebSocketFrame, ParseError>
- parse_grpc_message(data) -> Result<GrpcMessage, ParseError>
- parse_sse_event(data) -> Result<SseEvent, ParseError>

### Capture Functions (flowlens-capture)
- start_capture() -> Result<(), CaptureError>
- stop_capture() -> Result<(), CaptureError>
- get_capture_status() -> CaptureStatus
- diagnose_capture() -> DiagnosisResult
- resolve_process(pid) -> Option<String>

### Storage Functions (flowlens-storage)
- save_transaction(tx) -> Result<u64, StorageError>
- get_transaction(id) -> Result<HttpTransaction, StorageError>
- list_transactions(filter, skip, limit) -> Result<Vec<HttpTransaction>, StorageError>
- search_transactions(query, case_sensitive) -> Result<SearchResult, StorageError>
- export_har(transactions, path) -> Result<(), ExportError>
- export_saz(transactions, path) -> Result<(), ExportError>
- export_curl(transaction) -> String
- import_har(path) -> Result<Vec<HttpTransaction>, ImportError>
- import_saz(path) -> Result<Vec<HttpTransaction>, ImportError>

### TLS Functions (flowlens-tls)
- generate_ca_cert() -> Result<Certificate, CertError>
- install_ca_cert(cert) -> Result<(), CertError>
- get_cert_for_host(host) -> Result<Certificate, CertError>
- delete_ca_cert() -> Result<(), CertError>

### MCP Functions (flowlens-mcp)
- handle_get_capture_status() -> McpToolResponse
- handle_diagnose_capture() -> McpToolResponse
- handle_list_endpoints(filter) -> McpToolResponse
- handle_list_transactions(filter, skip, limit) -> McpToolResponse
- handle_get_transaction(id, side, byte_offset) -> McpToolResponse
- handle_search_transactions(query, case_sensitive) -> McpToolResponse
- handle_export_as_curl(id) -> McpToolResponse
- handle_get_session_stats() -> McpToolResponse

### UI Functions (Tauri backend)
- start_capture() -> Result<(), String>
- stop_capture() -> Result<(), String>
- get_transactions(filter) -> Result<Vec<HttpTransaction>, String>
- get_transaction_detail(id) -> Result<HttpTransaction, String>
- export_transactions(format, ids, path) -> Result<(), String>
- import_transactions(format, path) -> Result<Vec<HttpTransaction>, String>

### Kernel Driver Types

- **DriverCommand**: command_type: DriverCommandType, parameters: Vec<u8>
- **TrafficEvent**: event_type: TrafficEventType, process_id: u32, process_name: String, local_addr: SocketAddr, remote_addr: SocketAddr, protocol: Protocol, direction: Direction, data: Vec<u8>, timestamp: u64
- **FilterContext**: layer_id: u64, sublayer_id: u64, callout_id: u64, filter_id: u64