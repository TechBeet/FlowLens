use super::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinHandle;
use flowlens_core::{EventBus, TrafficEvent, TrafficFilter, TrafficRow};
use flowlens_capture::Engine;
use flowlens_tls::cert_store;
use flowlens_tls::CaManager;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use serde::Serialize;

/// Flattened view of a raw `TrafficEvent`, matching the shape declared in
/// `ui/src/lib/types.ts`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UiEvent {
    pub id: String,
    pub time: String,
    pub kind: String,
    pub info: String,
    pub size: usize,
    pub src_addr: Option<String>,
    pub dst_addr: Option<String>,
    pub method: Option<String>,
    pub url: Option<String>,
    pub status: Option<String>,
    pub process_name: Option<String>,
    pub pid: Option<u32>,
    pub protocol: Option<String>,
    pub sni: Option<String>,
    pub tls_version: Option<String>,
    pub paused: bool,
    pub rewrite_applied: bool,
}

impl UiEvent {
    fn from_row(row: &TrafficRow, id: u64) -> Self {
        Self {
            id: id.to_string(),
            time: format_ts(row.timestamp),
            kind: row.event_type.clone(),
            info: build_info(row),
            size: row.body_size,
            src_addr: non_empty(&row.src_addr),
            dst_addr: non_empty(&row.dst_addr),
            method: non_empty(&row.method),
            url: non_empty(&row.path),
            status: non_empty(&row.status),
            process_name: None,
            pid: None,
            protocol: None,
            sni: None,
            tls_version: None,
            paused: false,
            rewrite_applied: false,
        }
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Format a microsecond timestamp as `HH:MM:SS.d`.
fn format_ts(micros: i64) -> String {
    if micros <= 0 {
        return "--:--:--".to_string();
    }
    let secs = micros / 1_000_000;
    let frac = (micros % 1_000_000) / 100_000;
    format!(
        "{:02}:{:02}:{:02}.{}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        frac
    )
}

/// Build a short human-readable description of an event row.
fn build_info(r: &TrafficRow) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(4);
    if !r.method.is_empty() {
        parts.push(&r.method);
    }
    if !r.path.is_empty() {
        parts.push(&r.path);
    }
    if !r.status.is_empty() {
        parts.push(&r.status);
    }
    if !r.src_addr.is_empty() {
        parts.push(&r.src_addr);
    }
    if parts.is_empty() {
        return r.event_type.clone();
    }
    parts.join(" ")
}

/// Classify an event into the per-protocol counters shown in the sidebar.
fn tally(stats: &mut flowlens_core::TrafficStats, event: &TrafficEvent) {
    stats.total += 1;
    match event {
        TrafficEvent::ConnectionEstablished { .. } | TrafficEvent::ConnectionClosed { .. } => {
            stats.connections += 1;
        }
        TrafficEvent::HttpRequest { .. } | TrafficEvent::HttpResponse { .. } => {
            stats.http += 1;
        }
        TrafficEvent::TlsHandshake { .. } => stats.tls += 1,
        TrafficEvent::SseEvent { .. } => stats.sse += 1,
        TrafficEvent::WebSocketFrame { .. } => stats.websocket += 1,
        _ => {}
    }
}

#[tauri::command]
pub async fn start_capture(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    device: Option<String>,
) -> Result<(), String> {
    if state.running.load(Ordering::SeqCst) {
        return Ok(());
    }
    state.running.store(true, Ordering::SeqCst);
    {
        let mut s = state.stats.lock().map_err(|e| e.to_string())?;
        *s = flowlens_core::TrafficStats::default();
    }
    // Reset captured events on a fresh capture run.
    if let Ok(mut events) = state.events.lock() {
        events.clear();
    }

    let bus = state.event_bus.clone();
    let running = state.running.clone();
    let stats = state.stats.clone();
    let events_store = state.events.clone();
    let emit_handle = app_handle.clone();

    // Spawn a tokio task that pumps the event bus and emits to the frontend.
    let handle: JoinHandle<()> = tokio::spawn(async move {
        let mut rx = bus.subscribe(TrafficFilter::default());
        let mut counter: u64 = 0;
        while running.load(Ordering::SeqCst) {
            match rx.recv().await {
                Ok(event) => {
                    let stats_snapshot = {
                        let mut s = stats.lock().unwrap();
                        tally(&mut s, &event);
                        s.clone()
                    };
                    if let Ok(mut events) = events_store.lock() {
                        events.push(event.clone());
                    }
                    if let Some(row) = TrafficRow::from_event(&event) {
                        counter += 1;
                        let ui_event = UiEvent::from_row(&row, counter);
                        let _ = emit_handle.emit("traffic-event", ui_event);
                    }
                    // Push stats to the frontend so dashboards stay live.
                    let _ = emit_handle.emit("stats-updated", stats_snapshot);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    // A Lagged receiver means we fell behind the producer; log and
                    // continue rather than terminating the pump task.
                    eprintln!("capture pump lagged, skipped {} events", skipped);
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break; // Channel closed; exit.
                }
            }
        }
    });
    state.pump_task.lock().unwrap().replace(handle);

    // Start the actual capture engine. Honour an explicitly requested device when
    // provided, otherwise auto-detect.
    let engine = match device {
        Some(dev) if !dev.trim().is_empty() => Engine::new_pcap(dev),
        _ => Engine::auto(),
    };
    let bus: Arc<dyn EventBus> = state.event_bus.clone();
    match engine {
        Engine::Pcap(_) => {
            println!("Npcap engine selected");
            flowlens_capture::spawn_engine_pump(engine, bus.clone());
            flowlens_capture::spawn_mock_producer(bus);
        }
        Engine::Mock(_) => {
            println!("Npcap unavailable, using mock producer");
            flowlens_capture::spawn_mock_producer(bus);
        }
    }

    let _ = app_handle.emit("capture-started", ());
    Ok(())
}

#[tauri::command]
pub async fn stop_capture(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.running.store(false, Ordering::SeqCst);
    if let Ok(mut t) = state.pump_task.lock() {
        if let Some(task) = t.take() {
            tokio::spawn(async move {
                // Wait for the task to finish (bounded).
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;
            });
        }
    }
    let _ = app_handle.emit("capture-stopped", ());
    Ok(())
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<flowlens_core::TrafficStats, String> {
    let s = state.stats.lock().map_err(|e| e.to_string())?;
    Ok(s.clone())
}

#[tauri::command]
pub async fn clear_events(state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut events = state.events.lock().map_err(|e| e.to_string())?;
        events.clear();
    }
    {
        let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
        *stats = flowlens_core::TrafficStats::default();
    }
    Ok(())
}

#[tauri::command]
pub async fn list_devices() -> Result<Vec<String>, String> {
    Ok(flowlens_capture::list_available_devices())
}

/// HAR 1.2 entry request/response timing info
#[derive(Debug, Clone, Serialize)]
struct HarTiming {
    send: i64,
    wait: i64,
    receive: i64,
}

/// HAR 1.2 header
#[derive(Debug, Clone, Serialize)]
struct HarHeader {
    name: String,
    value: String,
}

/// HAR 1.2 post data
#[derive(Debug, Clone, Serialize)]
struct HarPostData {
    mime_type: String,
    text: String,
}

/// HAR 1.2 request object
#[derive(Debug, Clone, Serialize)]
struct HarRequest {
    method: String,
    url: String,
    http_version: String,
    cookies: Vec<serde_json::Value>,
    headers: Vec<HarHeader>,
    query_string: Vec<serde_json::Value>,
    post_data: Option<HarPostData>,
    headers_size: i64,
    body_size: i64,
}

/// HAR 1.2 response object
#[derive(Debug, Clone, Serialize)]
struct HarResponse {
    status: u16,
    status_text: String,
    http_version: String,
    cookies: Vec<serde_json::Value>,
    headers: Vec<HarHeader>,
    content: HarContent,
    redirect_url: String,
    headers_size: i64,
    body_size: i64,
}

/// HAR 1.2 content object
#[derive(Debug, Clone, Serialize)]
struct HarContent {
    size: i64,
    mime_type: String,
    text: Option<String>,
}

/// HAR 1.2 cache object
#[derive(Debug, Clone, Serialize)]
struct HarCache {
    before_request: Option<serde_json::Value>,
    after_request: Option<serde_json::Value>,
}

/// HAR 1.2 log entry
#[derive(Debug, Clone, Serialize)]
struct HarEntry {
    pageref: Option<String>,
    started_date_time: String,
    request: HarRequest,
    response: HarResponse,
    cache: HarCache,
    time: i64,
    timings: HarTiming,
}

/// HAR 1.2 log object
#[derive(Debug, Clone, Serialize)]
struct HarLog {
    version: String,
    creator: HarCreator,
    browser: Option<HarCreator>,
    pages: Vec<serde_json::Value>,
    entries: Vec<HarEntry>,
}

/// HAR 1.2 creator
#[derive(Debug, Clone, Serialize)]
struct HarCreator {
    name: String,
    version: String,
    comment: Option<String>,
}

/// HAR archive container
#[derive(Debug, Clone, Serialize)]
struct HarArchive {
    log: HarLog,
}

fn format_http_version(v: &flowlens_core::models::HttpVersion) -> String {
    match v {
        flowlens_core::models::HttpVersion::Http10 => "HTTP/1.0".to_string(),
        flowlens_core::models::HttpVersion::Http11 => "HTTP/1.1".to_string(),
        flowlens_core::models::HttpVersion::Http2 => "HTTP/2.0".to_string(),
        flowlens_core::models::HttpVersion::Http3 => "HTTP/3.0".to_string(),
        flowlens_core::models::HttpVersion::Unknown => "HTTP/1.1".to_string(),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = data.get(i + 1).copied().unwrap_or(0);
        let b2 = data.get(i + 2).copied().unwrap_or(0);
        result.push(ALPHABET[(b0 >> 2) as usize] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4 | (b1 >> 4)) as usize] as char);
        if i + 1 < data.len() {
            result.push(ALPHABET[((b1 & 0x0f) << 2 | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if i + 2 < data.len() {
            result.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}

fn format_iso_timestamp(micros: i64) -> String {
    let secs = micros / 1_000_000;
    let nanos = (micros % 1_000_000) * 1000;
    let dt = chrono::DateTime::from_timestamp(secs, nanos as u32)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[tauri::command]
pub async fn export_har(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<(), String> {
    let events_guard = state.events.lock().map_err(|e| e.to_string())?;
    let events = events_guard.clone();
    drop(events_guard);

    let mut entries: Vec<HarEntry> = Vec::new();
    let mut pending_requests: std::collections::HashMap<u64, (HarRequest, i64)> =
        std::collections::HashMap::new();

    for event in events {
        match event {
            TrafficEvent::HttpRequest { session_id, conn: _, request } => {
                let req = HarRequest {
                    method: request.method.clone(),
                    url: request.url.clone(),
                    http_version: format_http_version(&request.request_line_version),
                    cookies: Vec::new(),
                    headers: request.headers.iter()
                        .map(|(n, v)| HarHeader { name: n.clone(), value: v.clone() })
                        .collect(),
                    query_string: Vec::new(),
                    post_data: request.body.as_ref().and_then(|b| {
                        if b.is_empty() { None } else {
                            Some(HarPostData {
                                mime_type: "application/octet-stream".to_string(),
                                text: base64_encode(b),
                            })
                        }
                    }),
                    headers_size: request.headers.iter()
                        .map(|(n, v)| (n.len() + v.len()) as i64).sum(),
                    body_size: request.body_size as i64,
                };
                pending_requests.insert(session_id.0, (req, request.timestamp));
            }
            TrafficEvent::HttpResponse { session_id, conn: _, response } => {
                if let Some((req, req_ts)) = pending_requests.remove(&session_id.0) {
                    let resp_time = response.timestamp;
                    let time_delta = resp_time - req_ts;
                    let content_type = response.headers.iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
                        .map(|(_, v)| v.clone())
                        .unwrap_or_else(|| "application/octet-stream".to_string());
                    let entry = HarEntry {
                        pageref: None,
                        started_date_time: format_iso_timestamp(req_ts),
                        request: req,
                        response: HarResponse {
                            status: response.status_code,
                            status_text: response.status_text.clone(),
                            http_version: format_http_version(&response.version),
                            cookies: Vec::new(),
                            headers: response.headers.iter()
                                .map(|(n, v)| HarHeader { name: n.clone(), value: v.clone() })
                                .collect(),
                            content: HarContent {
                                size: response.body_size as i64,
                                mime_type: content_type,
                                text: response.body.as_ref().map(|b| base64_encode(b)),
                            },
                            redirect_url: String::new(),
                            headers_size: response.headers.iter()
                                .map(|(n, v)| (n.len() + v.len()) as i64).sum(),
                            body_size: response.body_size as i64,
                        },
                        cache: HarCache { before_request: None, after_request: None },
                        time: time_delta.max(0),
                        timings: HarTiming {
                            send: 0,
                            wait: time_delta / 2,
                            receive: time_delta / 2,
                        },
                    };
                    entries.push(entry);
                }
            }
            _ => {}
        }
    }

    let har = HarArchive {
        log: HarLog {
            version: "1.2".to_string(),
            creator: HarCreator {
                name: "FlowLens".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                comment: Some("HTTP Archive export from FlowLens".to_string()),
            },
            browser: None,
            pages: Vec::new(),
            entries,
        },
    };

    let json = serde_json::to_string_pretty(&har).map_err(|e| e.to_string())?;
    let mut file = File::create(&path).map_err(|e| e.to_string())?;
    file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn install_ca(pem: String) -> Result<(), String> {
    cert_store::install_ca_to_store(&pem).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn uninstall_ca(pem: String) -> Result<(), String> {
    cert_store::uninstall_ca_from_store(&pem).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_ca_status(pem: String) -> Result<bool, String> {
    Ok(cert_store::is_ca_installed(&pem))
}

#[tauri::command]
pub async fn pause_session(state: State<'_, AppState>, session_id: u64) -> Result<(), String> {
    state
        .intercept
        .pause(flowlens_core::SessionId(session_id), None, None, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn release_session(state: State<'_, AppState>, session_id: u64) -> Result<(), String> {
    state
        .intercept
        .release(flowlens_core::SessionId(session_id))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn modify_session(
    state: State<'_, AppState>,
    session_id: u64,
    new_data: serde_json::Value,
) -> Result<(), String> {
    // new_data is a JSON object that may contain "request" and/or "response" fields
    let new_request = new_data
        .get("request")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let new_response = new_data
        .get("response")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    state
        .intercept
        .modify(flowlens_core::SessionId(session_id), new_request, new_response)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_paused_sessions(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let paused = state.intercept.list_paused().await;
    serde_json::to_value(paused).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_rewrite_rule(
    state: State<'_, AppState>,
    rule: serde_json::Value,
) -> Result<(), String> {
    let rewrite_rule: flowlens_core::RewriteRule = serde_json::from_value(rule)
        .map_err(|e| format!("Invalid rule JSON: {}", e))?;
    
    let mut engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    engine.add_rule(rewrite_rule);
    drop(engine);
    Ok(())
}

#[tauri::command]
pub async fn remove_rewrite_rule(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let mut engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    Ok(engine.remove_rule(&id))
}

#[tauri::command]
pub async fn toggle_rewrite_rule(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<bool, String> {
    let mut engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    Ok(engine.toggle_rule(&id, enabled))
}

#[tauri::command]
pub async fn list_rewrite_rules(state: State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    let engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    let rules = engine.list_rules().to_vec();
    let mut result = Vec::new();
    for r in rules {
        result.push(serde_json::to_value(r).map_err(|e| e.to_string())?);
    }
    Ok(result)
}

/// Preview the result of applying rewrite rules to a request without persisting.
#[tauri::command]
pub async fn preview_rewrite(
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let req: flowlens_core::HttpRequest = serde_json::from_value(request)
        .map_err(|e| format!("Invalid request JSON: {}", e))?;
    let engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    let modified = engine.apply_rules(req);
    Ok(serde_json::json!({
        "original": serde_json::to_value(&modified.original).unwrap_or(serde_json::Value::Null),
        "modified": serde_json::to_value(&modified.modified).unwrap_or(serde_json::Value::Null),
        "applied_rules": modified.applied_rules
    }))
}

/// Apply rewrite rules to a captured session and return the modified request.
#[tauri::command]
pub async fn apply_rewrite_to_session(
    state: State<'_, AppState>,
    session_id: u64,
) -> Result<serde_json::Value, String> {
    let request = state
        .intercept
        .get_request(flowlens_core::SessionId(session_id))
        .await
        .ok_or_else(|| format!("Request not found for session {}", session_id))?;
    let engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    let modified = engine.apply_rules(request.clone());
    Ok(serde_json::json!({
        "original": serde_json::to_value(&request).unwrap_or(serde_json::Value::Null),
        "modified": serde_json::to_value(&modified.modified).unwrap_or(serde_json::Value::Null),
        "applied_rules": modified.applied_rules
    }))
}

#[tauri::command]
pub async fn replay_request(
    state: State<'_, AppState>,
    session_id: u64,
) -> Result<serde_json::Value, String> {
    use flowlens_replay::ReplayClient;
    
    let request = state
        .intercept
        .get_request(flowlens_core::SessionId(session_id))
        .await
        .ok_or_else(|| format!("Request not found for session {}", session_id))?;
    
    let client = ReplayClient::new().map_err(|e| e.to_string())?;
    let response = client.replay(&request).await.map_err(|e| e.to_string())?;
    
    serde_json::to_value(response).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn compare_sessions(
    _state: State<'_, AppState>,
    id_a: u64,
    id_b: u64,
) -> Result<serde_json::Value, String> {
    // TODO: Wire up persistent storage and use diff
    let result = serde_json::json!({
        "session_a": id_a,
        "session_b": id_b,
        "comparison": "Pending - implement storage-backed comparison"
    });
    Ok(result)
}

/// Serializable view of CA information returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CaInfo {
    pub cn: String,
    pub certificate_pem: String,
    pub fingerprint_sha256: String,
    pub installed_at: i64,
}

#[tauri::command]
pub async fn auto_release_paused(
    state: State<'_, AppState>,
    session_id: u64,
    timeout_ms: u64,
) -> Result<(), String> {
    state
        .intercept
        .auto_release_paused(flowlens_core::SessionId(session_id), timeout_ms)
        .await
        .map_err(|e| e.to_string())
}

/// Return full CA information (CN, PEM, fingerprint, installed_at).
#[tauri::command]
pub async fn get_ca_pem(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    Ok(engine.ca_pem())
}

/// Return full CA information (CN, PEM, fingerprint, installed_at).
#[tauri::command]
pub async fn get_ca_info(state: State<'_, AppState>) -> Result<Option<CaInfo>, String> {
    let mut engine = state.ca_manager.lock().map_err(|e| e.to_string())?;

    // Prefer the cached CaInfo; fall back to generating/retrieving via the engine.
    let stored = state.ca_info.lock().map_err(|e| e.to_string())?.clone();
    let ca = match stored {
        Some(ca) => ca,
        None => {
            let ca = engine
                .generate_ca("FlowLens CA")
                .map_err(|e| e.to_string())?;
            if let Ok(mut info) = state.ca_info.lock() {
                *info = Some(ca.clone());
            }
            ca
        }
    };

    // Exercise both required CaManager accessors.
    let pem = engine.ca_pem().unwrap_or_else(|| ca.certificate_pem.clone());
    let _installed = engine.check_installed(&ca.cn);

    Ok(Some(CaInfo {
        cn: ca.cn,
        certificate_pem: pem,
        fingerprint_sha256: ca.fingerprint_sha256,
        installed_at: ca.installed_at,
    }))
}

/// Generate a QR code (PNG, base64-encoded) for the CA certificate PEM.
#[tauri::command]
pub async fn export_ca_qr(state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    let pem = engine
        .ca_pem()
        .ok_or_else(|| "CA not generated yet".to_string())?;
    drop(engine);

    // Try to render a real QR code PNG; fall back to base64-encoded PEM on any failure.
    match render_ca_qr_png(&pem) {
        Ok(b64) => Ok(b64),
        Err(_) => Ok(base64_encode(pem.as_bytes())),
    }
}

/// Render `data` as a QR code PNG and return base64-encoded PNG bytes.
fn render_ca_qr_png(data: &str) -> Result<String, String> {
    use image::ImageFormat;
    use std::io::Cursor;

    let code = qrcode::QrCode::new(data.as_bytes()).map_err(|e| e.to_string())?;
    let img = code.render::<image::Luma<u8>>().build();
    let mut cursor = Cursor::new(Vec::new());
    img.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(base64_encode(cursor.into_inner().as_slice()))
}

/// Generate a new CA with the given CN and return the PEM.
#[tauri::command]
pub async fn generate_ca(
    state: State<'_, AppState>,
    cn: String,
) -> Result<String, String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    let ca = engine.setup_ca(&cn).map_err(|e| e.to_string())?;
    // Keep the cached CaInfo in AppState so get_ca_info can report CN/fingerprint/installed_at.
    if let Ok(mut info) = state.ca_info.lock() {
        *info = Some(ca.clone());
    }
    Ok(ca.certificate_pem)
}

/// Return the full session detail (serialised TrafficSession) for the given id.
/// Placeholder until persistent session storage is wired into AppState.
#[tauri::command]
pub async fn get_session_detail(
    state: State<'_, AppState>,
    session_id: u64,
) -> Result<Option<serde_json::Value>, String> {
    let _ = (state, session_id);
    Ok(None)
}

/// Query stored sessions with optional filters and pagination.
#[tauri::command]
pub async fn query_sessions(
    state: State<'_, AppState>,
    query: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let pid = query.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
    let host = query.get("host").and_then(|v| v.as_str()).map(|s| s.to_string());
    let protocol = query
        .get("protocol")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let limit = query.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
    let offset = query.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);

    let sessions = state.intercept.list_paused().await; // placeholder; use session manager when wired
    let _ = (pid, host, protocol, limit, offset, sessions);
    Ok(serde_json::json!({ "sessions": [], "total": 0 }))
}

/// Delete all captured sessions.
#[tauri::command]
pub async fn delete_sessions(
    state: State<'_, AppState>,
    _before: Option<i64>,
) -> Result<u64, String> {
    let mut events = state.events.lock().map_err(|e| e.to_string())?;
    let len = events.len();
    events.clear();
    Ok(len as u64)
}

/// Aggregate captured traffic by process ID.
#[tauri::command]
pub async fn aggregate_by_pid(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    use std::collections::HashMap;

    let events = state.events.lock().map_err(|e| e.to_string())?;
    let mut stats: HashMap<u32, (String, u64, u64, u64)> = HashMap::new();

    for event in events.iter() {
        if let flowlens_core::TrafficEvent::ConnectionEstablished { process, .. } = event {
            let entry = stats
                .entry(process.pid)
                .or_insert_with(|| (process.name.clone(), 0, 0, 0));
            entry.1 += 1;
        }
    }

    let result: Vec<serde_json::Value> = stats
        .into_iter()
        .map(|(pid, (process_name, session_count, bytes_sent, bytes_received))| {
            serde_json::json!({
                "pid": pid,
                "process_name": process_name,
                "session_count": session_count,
                "bytes_sent": bytes_sent,
                "bytes_received": bytes_received,
            })
        })
        .collect();

    Ok(serde_json::json!(result))
}

/// Aggregate captured traffic by destination host.
#[tauri::command]
pub async fn aggregate_by_host(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    use std::collections::HashMap;

    let events = state.events.lock().map_err(|e| e.to_string())?;
    let mut stats: HashMap<String, (u64, u64, u64)> = HashMap::new();

    for event in events.iter() {
        match event {
            flowlens_core::TrafficEvent::ConnectionEstablished { conn, .. } => {
                let host = conn.dst_addr.to_string();
                let entry = stats.entry(host).or_insert((0, 0, 0));
                entry.0 += 1;
            }
            flowlens_core::TrafficEvent::HttpRequest { conn, request, .. } => {
                let host = conn.dst_addr.to_string();
                let entry = stats.entry(host).or_insert((0, 0, 0));
                entry.1 += request.body_size as u64;
            }
            flowlens_core::TrafficEvent::HttpResponse { conn, response, .. } => {
                let host = conn.dst_addr.to_string();
                let entry = stats.entry(host).or_insert((0, 0, 0));
                entry.2 += response.body_size as u64;
            }
            _ => {}
        }
    }

    let result: Vec<serde_json::Value> = stats
        .into_iter()
        .map(|(host, (session_count, bytes_sent, bytes_received))| {
            serde_json::json!({
                "host": host,
                "session_count": session_count,
                "bytes_sent": bytes_sent,
                "bytes_received": bytes_received,
            })
        })
        .collect();

    Ok(serde_json::json!(result))
}

/// Extract a session id from an event when present.
fn event_session_id(event: &flowlens_core::TrafficEvent) -> Option<flowlens_core::models::SessionId> {
    match event {
        flowlens_core::TrafficEvent::HttpRequest { session_id, .. } => Some(*session_id),
        flowlens_core::TrafficEvent::HttpResponse { session_id, .. } => Some(*session_id),
        flowlens_core::TrafficEvent::WebSocketFrame { session_id, .. } => Some(*session_id),
        flowlens_core::TrafficEvent::SseEvent { session_id, .. } => Some(*session_id),
        flowlens_core::TrafficEvent::GrpcCall { session_id, .. } => Some(*session_id),
        _ => None,
    }
}

/// Return a paginated list of captured sessions/events.
#[tauri::command]
pub async fn get_session_list(
    state: State<'_, AppState>,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, String> {
    let events = state.events.lock().map_err(|e| e.to_string())?;
    let total = events.len();
    let page: Vec<serde_json::Value> = events
        .iter()
        .skip(offset)
        .take(limit)
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();

    Ok(serde_json::json!({
        "sessions": page,
        "total": total,
        "offset": offset,
        "limit": limit,
    }))
}

/// Delete a single session (and its related captured events).
#[tauri::command]
pub async fn delete_session(
    state: State<'_, AppState>,
    session_id: u64,
) -> Result<serde_json::Value, String> {
    let mut events = state.events.lock().map_err(|e| e.to_string())?;
    let before = events.len();
    events.retain(|e| {
        !event_session_id(e)
            .map(|sid| sid.0 == session_id)
            .unwrap_or(false)
    });
    let deleted = (before - events.len()) as u64;
    Ok(serde_json::json!(deleted))
}
