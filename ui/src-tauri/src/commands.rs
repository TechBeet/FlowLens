use super::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinHandle;
use flowlens_core::{EventBus, TrafficEvent, TrafficFilter, TrafficRow};
use flowlens_capture::Engine;
use flowlens_tls::cert_store;
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
) -> Result<(), String> {
    if state.running.load(Ordering::SeqCst) {
        return Ok(());
    }
    state.running.store(true, Ordering::SeqCst);
    {
        let mut s = state.stats.lock().map_err(|e| e.to_string())?;
        *s = flowlens_core::TrafficStats::default();
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
                    {
                        let mut s = stats.lock().unwrap();
                        tally(&mut s, &event);
                    }
                    if let Ok(mut events) = events_store.lock() {
                        events.push(event.clone());
                    }
                    if let Some(row) = TrafficRow::from_event(&event) {
                        counter += 1;
                        let ui_event = UiEvent::from_row(&row, counter);
                        let _ = emit_handle.emit("traffic-event", ui_event);
                    }
                }
                Err(_) => break, // Channel closed; exit.
            }
        }
    });
    state.pump_task.lock().unwrap().replace(handle);

    // Start the actual capture engine. Npcap is preferred for real capture.
    // When Npcap is active we ALSO run the mock producer so the UI still shows
    // a live event stream if the chosen interface is idle or Npcap lacks the
    // privileges to observe traffic. When Npcap is unavailable we only run the
    // mock producer (avoids duplicate event streams).
    let engine = Engine::auto();
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
    output_path: PathBuf,
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
    let mut file = File::create(&output_path).map_err(|e| e.to_string())?;
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
    state: State<'_, AppState>,
    id_a: u64,
    id_b: u64,
) -> Result<serde_json::Value, String> {
    // Placeholder for future session comparison logic
    let result = serde_json::json!({
        "session_a": id_a,
        "session_b": id_b,
        "comparison": "TBD - implement session comparison logic"
    });
    Ok(result)
}

#[tauri::command]
pub async fn get_ca_pem() -> Result<Option<String>, String> {
    Ok(None)
}
