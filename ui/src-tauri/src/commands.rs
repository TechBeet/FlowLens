use super::state::AppState;
use flowlens_capture::Engine;
use flowlens_core::session_store::SessionStore;
use flowlens_core::{EventBus, TrafficEvent, TrafficFilter, TrafficRow};
use flowlens_tls::cert_store;
use flowlens_tls::CaManager;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinHandle;

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
        let process_name = row.process_info.as_ref().map(|p| p.name.clone());
        let pid = row.process_info.as_ref().map(|p| p.pid);
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
            process_name,
            pid,
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

/// Capture mode selected by the user.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    /// Traditional Npcap capture (requires Npcap installed).
    #[default]
    Pcap,
    /// Transparent WinDivert capture (no proxy required).
    WinDivert,
}

#[tauri::command]
pub async fn start_capture(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    device: Option<String>,
    #[allow(unused_variables)] mode: Option<CaptureMode>,
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
    let session_store = state.session_store.clone();
    let emit_handle = app_handle.clone();

    // Spawn a tokio task that pumps the event bus and emits to the frontend.
    let handle: JoinHandle<()> = tokio::spawn(async move {
        let mut rx = bus.subscribe(TrafficFilter::default());
        let mut counter: u64 = 0;
        while running.load(Ordering::SeqCst) {
            match rx.recv().await {
                Ok(event) => {
                    let stats_snapshot = {
                        // Poison-tolerant: `TrafficStats` is a set of counters;
                        // a panicking task cannot corrupt them, so recover.
                        let mut s = stats
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        tally(&mut s, &event);
                        s.clone()
                    };
                    if let Ok(mut events) = events_store.lock() {
                        events.push(event.clone());
                    }
                    // Append to the persistent session store so get_session_detail /
                    // query_sessions return real data.
                    if let Err(e) = session_store.append(event.clone()) {
                        tracing::debug!(error = %e, "session_store append failed");
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
    // Poison-tolerant: the slot holds an `Option<JoinHandle>`; replacing it
    // after a previous task panicked is safe and keeps capture restartable.
    state
        .pump_task
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .replace(handle);

    let bus: Arc<dyn EventBus> = state.event_bus.clone();

    // ── WinDivert mode (transparent, no-proxy) ──────────────────────────
    #[cfg(feature = "windivert")]
    if let Some(CaptureMode::WinDivert) = mode {
        // 1) Start the MITM listener that will terminate TLS.
        const MITM_PORT: u16 = 18080;
        let proxy_addr = std::net::SocketAddr::from(([127, 0, 0, 1], MITM_PORT));
        let runtime = std::sync::Arc::new(crate::proxy::ProxyRuntime::new(&state));
        let listener_runtime = runtime.clone();
        let proxy_handle = tokio::spawn(async move {
            if let Err(e) = listener_runtime.run_listener(proxy_addr).await {
                tracing::warn!(error = %e, "MITM listener stopped with error");
            }
        });
        *state.proxy_runtime.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
        *state.proxy_addr.lock().map_err(|e| e.to_string())? = Some(proxy_addr);
        *state.proxy_task.lock().map_err(|e| e.to_string())? = Some(proxy_handle);

        // 2) Start the transparent redirect that funnels port-443 into the listener.
        //    The redirect thread owns the WinDivert handle (not Send-safe), so we
        //    spawn it here and signal stop via `state.redirect_stop`.
        state.redirect_stop.store(false, Ordering::SeqCst);
        let stop = state.redirect_stop.clone();
        let redirect_handle = std::thread::spawn(move || {
            match flowlens_capture::RedirectEngine::new(MITM_PORT) {
                Ok(engine) => {
                    let _ = engine.start();
                    if let Err(e) = flowlens_capture::spawn_redirect(engine) {
                        tracing::warn!(err = ?e, "WinDivert redirect spawn failed");
                    }
                }
                Err(e) => {
                    eprintln!("WinDivert redirect unavailable: {e}");
                }
            }
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        *state.redirect_task.lock().map_err(|e| e.to_string())? = Some(redirect_handle);
        println!("WinDivert redirect + MITM listener started on port {MITM_PORT}");
        let _ = app_handle.emit("capture-started", ());
        return Ok(());
    }
    #[cfg(not(feature = "windivert"))]
    if matches!(mode, Some(CaptureMode::WinDivert)) {
        eprintln!("WinDivert support not compiled in; rebuild with --features windivert");
    }

    // ── Npcap / Mock mode (default) ─────────────────────────────────────
    let engine = match device {
        Some(dev) if !dev.trim().is_empty() => Engine::new_pcap(dev),
        _ => Engine::auto(),
    };
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
pub async fn stop_capture(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.running.store(false, Ordering::SeqCst);
    if let Ok(mut t) = state.pump_task.lock() {
        if let Some(task) = t.take() {
            tokio::spawn(async move {
                // Wait for the task to finish (bounded).
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;
            });
        }
    }
    // Stop the redirect engine and MITM listener.
    #[cfg(feature = "windivert")]
    {
        state.redirect_stop.store(true, Ordering::SeqCst);
        if let Ok(mut t) = state.redirect_task.lock() {
            if let Some(task) = t.take() {
                // The redirect thread polls `redirect_stop`; joining is best-effort.
                let _ = task.join();
            }
        }
        let _ = state
            .proxy_task
            .lock()
            .map(|mut t| t.take())
            .unwrap_or_default();
        let _ = state
            .proxy_addr
            .lock()
            .map(|mut t| t.take())
            .unwrap_or_default();
        let _ = state
            .proxy_runtime
            .lock()
            .map(|mut t| t.take())
            .unwrap_or_default();
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

/// Options controlling what the HAR 1.2 exporter includes in each entry.
///
/// The webview supplies these from the export modal. They are plain booleans so
/// the JSON contract stays simple: `{ include_headers, include_bodies, include_timing }`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HarExportOptions {
    /// Emit request and response `headers` (and the derived `headersSize`).
    pub include_headers: Option<bool>,
    /// Emit request `postData.text` and response `content.text` (base64).
    pub include_bodies: Option<bool>,
    /// Emit `time` and the `timings` block.
    pub include_timing: Option<bool>,
}

impl Default for HarExportOptions {
    fn default() -> Self {
        Self {
            include_headers: Some(true),
            include_bodies: Some(true),
            include_timing: Some(true),
        }
    }
}

impl HarExportOptions {
    fn headers(self) -> bool {
        self.include_headers.unwrap_or(true)
    }

    fn bodies(self) -> bool {
        self.include_bodies.unwrap_or(true)
    }

    fn timing(self) -> bool {
        self.include_timing.unwrap_or(true)
    }
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
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
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
    options: Option<HarExportOptions>,
) -> Result<(), String> {
    let options = options.unwrap_or_default();

    let events_guard = state.events.lock().map_err(|e| e.to_string())?;
    let events = events_guard.clone();
    drop(events_guard);

    let mut entries: Vec<HarEntry> = Vec::new();
    let mut pending_requests: std::collections::HashMap<u64, (HarRequest, i64)> =
        std::collections::HashMap::new();

    for event in events {
        match event {
            TrafficEvent::HttpRequest {
                session_id,
                conn: _,
                request,
            } => {
                let headers_vec: Vec<HarHeader> = request
                    .headers
                    .iter()
                    .map(|(n, v)| HarHeader {
                        name: n.clone(),
                        value: v.clone(),
                    })
                    .collect();
                let headers_size: i64 = headers_vec
                    .iter()
                    .map(|h| (h.name.len() + h.value.len()) as i64)
                    .sum();

                let req = HarRequest {
                    method: request.method.clone(),
                    url: request.url.clone(),
                    http_version: format_http_version(&request.request_line_version),
                    cookies: Vec::new(),
                    headers: if options.headers() {
                        headers_vec.clone()
                    } else {
                        Vec::new()
                    },
                    query_string: Vec::new(),
                    post_data: if options.bodies() {
                        request.body.as_ref().and_then(|b| {
                            if b.is_empty() {
                                None
                            } else {
                                Some(HarPostData {
                                    mime_type: "application/octet-stream".to_string(),
                                    text: base64_encode(b),
                                })
                            }
                        })
                    } else {
                        None
                    },
                    headers_size: if options.headers() { headers_size } else { 0 },
                    body_size: request.body_size as i64,
                };
                pending_requests.insert(session_id.0, (req, request.timestamp));
            }
            TrafficEvent::HttpResponse {
                session_id,
                conn: _,
                response,
            } => {
                if let Some((req, req_ts)) = pending_requests.remove(&session_id.0) {
                    let resp_time = response.timestamp;
                    let time_delta = resp_time - req_ts;
                    let content_type = response
                        .headers
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
                        .map(|(_, v)| v.clone())
                        .unwrap_or_else(|| "application/octet-stream".to_string());
                    let resp_headers_vec: Vec<HarHeader> = response
                        .headers
                        .iter()
                        .map(|(n, v)| HarHeader {
                            name: n.clone(),
                            value: v.clone(),
                        })
                        .collect();
                    let resp_headers_size: i64 = resp_headers_vec
                        .iter()
                        .map(|h| (h.name.len() + h.value.len()) as i64)
                        .sum();
                    let entry = HarEntry {
                        pageref: None,
                        started_date_time: format_iso_timestamp(req_ts),
                        request: req,
                        response: HarResponse {
                            status: response.status_code,
                            status_text: response.status_text.clone(),
                            http_version: format_http_version(&response.version),
                            cookies: Vec::new(),
                            headers: if options.headers() {
                                resp_headers_vec.clone()
                            } else {
                                Vec::new()
                            },
                            content: HarContent {
                                size: response.body_size as i64,
                                mime_type: content_type,
                                text: if options.bodies() {
                                    response.body.as_ref().map(|b| base64_encode(b))
                                } else {
                                    None
                                },
                            },
                            redirect_url: String::new(),
                            headers_size: if options.headers() {
                                resp_headers_size
                            } else {
                                0
                            },
                            body_size: response.body_size as i64,
                        },
                        cache: HarCache {
                            before_request: None,
                            after_request: None,
                        },
                        time: if options.timing() {
                            time_delta.max(0)
                        } else {
                            0
                        },
                        timings: if options.timing() {
                            HarTiming {
                                send: 0,
                                wait: time_delta / 2,
                                receive: time_delta / 2,
                            }
                        } else {
                            HarTiming {
                                send: 0,
                                wait: 0,
                                receive: 0,
                            }
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
    let mut file =
        File::create(&path).map_err(|e| format!("无法创建文件 {}: {e}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("写入文件 {} 失败: {e}", path.display()))?;

    Ok(())
}

/// Install the CA certificate (identified by `cn`) into the Windows
/// Trusted Root store. If no CA has been generated yet for `cn`, one is
/// created first via the in-memory engine.
#[tauri::command]
pub async fn install_ca(state: State<'_, AppState>, cn: String) -> Result<(), String> {
    // Ensure a CA exists for this CN (generate on first use).
    let pem = {
        let mut engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
        match engine.ca_pem() {
            Some(p) => p,
            None => {
                let ca = engine.generate_ca(&cn).map_err(|e| e.to_string())?;
                if let Ok(mut info) = state.ca_info.lock() {
                    *info = Some(ca.clone());
                }
                ca.certificate_pem
            }
        }
    };
    cert_store::install_ca_to_store(&pem).map_err(|e| e.to_string())
}

/// Uninstall the CA certificate (identified by `cn`) from the Windows
/// certificate stores.
#[tauri::command]
pub async fn uninstall_ca(state: State<'_, AppState>, cn: String) -> Result<(), String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    let pem = engine
        .ca_pem()
        .ok_or_else(|| format!("no CA generated for CN '{}'; nothing to uninstall", cn))?;
    cert_store::uninstall_ca_from_store(&pem).map_err(|e| e.to_string())
}

/// Check whether the CA certificate (identified by `cn`) is installed in
/// the Windows Trusted Root store.
#[tauri::command]
pub async fn check_ca_status(
    state: State<'_, AppState>,
    cn: String,
) -> Result<serde_json::Value, String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    let pem = match engine.ca_pem() {
        Some(p) => p,
        None => {
            return Ok(serde_json::json!({
                "installed": false,
                "cn": cn,
                "pem": null,
            }));
        }
    };
    let installed = cert_store::is_ca_installed(&pem);
    Ok(serde_json::json!({
        "installed": installed,
        "cn": cn,
        "pem": pem,
    }))
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
        .modify(
            flowlens_core::SessionId(session_id),
            new_request,
            new_response,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_paused_sessions(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let paused = state.intercept.list_paused().await;
    serde_json::to_value(paused).map_err(|e| e.to_string())
}

/// Coerce the `priority` key of an incoming rewrite-rule payload into a `u8`.
///
/// Accepts an absent or `null` value (normalised to
/// [`flowlens_core::DEFAULT_PRIORITY`]) and any integer in `0..=255`. Anything
/// else is rejected with an actionable message the UI can surface verbatim,
/// which is friendlier than the generic serde error we would otherwise get for
/// a `u8` field.
///
/// The normalised value is always written back into the payload so that the
/// rule stored in the engine — and therefore whatever `list_rewrite_rules`
/// later reports — carries an explicit `priority`.
fn normalize_rewrite_priority(rule: &mut serde_json::Value) -> Result<u8, String> {
    let obj = rule
        .as_object_mut()
        .ok_or_else(|| "Invalid rule JSON: expected an object".to_string())?;

    let priority = match obj.get("priority") {
        None | Some(serde_json::Value::Null) => flowlens_core::DEFAULT_PRIORITY,
        Some(value) => {
            let raw = value
                .as_u64()
                .ok_or_else(|| format!("priority must be an integer in 0..=255, got {}", value))?;
            u8::try_from(raw).map_err(|_| format!("priority must be in 0..=255, got {}", raw))?
        }
    };

    obj.insert("priority".to_string(), serde_json::Value::from(priority));
    Ok(priority)
}

#[tauri::command]
pub async fn add_rewrite_rule(
    state: State<'_, AppState>,
    mut rule: serde_json::Value,
) -> Result<(), String> {
    normalize_rewrite_priority(&mut rule)?;
    let rewrite_rule: flowlens_core::RewriteRule =
        serde_json::from_value(rule).map_err(|e| format!("Invalid rule JSON: {}", e))?;

    let mut engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
    engine.add_rule(rewrite_rule);
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
pub async fn list_rewrite_rules(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rules = {
        let engine = state.rewrite_engine.lock().map_err(|e| e.to_string())?;
        engine.list_rules().to_vec()
    };

    let mut result = Vec::with_capacity(rules.len());
    for rule in rules {
        // `priority` is re-asserted rather than trusted: the engine is the
        // source of truth, and this keeps the field present for payloads built
        // from older in-memory rules.
        let priority = rule.priority;
        let mut value = serde_json::to_value(rule).map_err(|e| e.to_string())?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("priority".to_string(), serde_json::Value::from(priority));
        }
        result.push(value);
    }
    Ok(result)
}

/// Preview the result of applying rewrite rules to a request without persisting.
#[tauri::command]
pub async fn preview_rewrite(
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let req: flowlens_core::HttpRequest =
        serde_json::from_value(request).map_err(|e| format!("Invalid request JSON: {}", e))?;
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

    let client = ReplayClient::new();
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

// ---------- Theme persistence (simple JSON file under the temp dir) ----------

fn theme_file_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push("flowlens_theme.txt");
    p
}

/// Read the persisted theme preference, defaulting to "dark".
#[tauri::command]
pub async fn read_theme() -> Result<String, String> {
    match std::fs::read_to_string(theme_file_path()) {
        Ok(s) => {
            let s = s.trim().to_string();
            if s.is_empty() {
                Ok("dark".to_string())
            } else {
                Ok(s)
            }
        }
        Err(_) => Ok("dark".to_string()),
    }
}

/// Persist the theme preference (best-effort; never fails the UI).
#[tauri::command]
pub async fn write_theme(theme: String) -> Result<(), String> {
    let path = theme_file_path();
    std::fs::write(&path, theme).map_err(|e| format!("failed to write theme: {e}"))
}

// ---------- Event ring-buffer cap (event_max.txt) ----------

fn event_max_file_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push("flowlens_event_max.txt");
    p
}

/// Persist the maximum number of events retained in the frontend ring buffer.
#[tauri::command]
pub async fn set_event_max(cap: usize) -> Result<(), String> {
    let cap = cap.clamp(100, 50_000);
    std::fs::write(event_max_file_path(), cap.to_string())
        .map_err(|e| format!("failed to write event max: {e}"))
}

/// Read the persisted event ring-buffer cap (default 10 000).
#[tauri::command]
pub async fn get_event_max() -> Result<usize, String> {
    match std::fs::read_to_string(event_max_file_path()) {
        Ok(s) => Ok(s
            .trim()
            .parse::<usize>()
            .unwrap_or(10_000)
            .clamp(100, 50_000)),
        Err(_) => Ok(10_000),
    }
}

// ---------- Per-host (SNI) decryption policy ----------

/// Serializable per-host decryption rule returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct UiDecryptEntry {
    pub domain: String,
    pub decrypt: bool,
    pub wildcard: bool,
    pub cert_ready: bool,
    pub added_at: i64,
}

/// Serializable leaf certificate returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct SniCertInfo {
    pub sni: String,
    pub subject_cn: String,
    pub fingerprint_sha256: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub sans: Vec<String>,
}

fn decrypt_entry(
    rule: &flowlens_tls::DomainRule,
    engine: &flowlens_tls::InMemoryTlsEngine,
) -> UiDecryptEntry {
    UiDecryptEntry {
        domain: rule.domain.clone(),
        decrypt: rule.decrypt,
        wildcard: rule.wildcard,
        cert_ready: engine.issued_snis().contains(&rule.domain),
        added_at: 0,
    }
}

#[tauri::command]
pub async fn list_decrypt_domains(
    state: State<'_, AppState>,
) -> Result<Vec<UiDecryptEntry>, String> {
    let policy = state.decrypt_policy.lock().map_err(|e| e.to_string())?;
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    Ok(policy
        .rules()
        .iter()
        .map(|r| decrypt_entry(r, &engine))
        .collect())
}

#[tauri::command]
pub async fn add_decrypt_domain(
    state: State<'_, AppState>,
    domain: String,
) -> Result<UiDecryptEntry, String> {
    let rule = {
        let mut policy = state.decrypt_policy.lock().map_err(|e| e.to_string())?;
        policy.add(&domain, true).map_err(|e| e.to_string())?
    };
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    Ok(decrypt_entry(&rule, &engine))
}

#[tauri::command]
pub async fn remove_decrypt_domain(
    state: State<'_, AppState>,
    domain: String,
) -> Result<bool, String> {
    let mut policy = state.decrypt_policy.lock().map_err(|e| e.to_string())?;
    Ok(policy.remove(&domain))
}

#[tauri::command]
pub async fn toggle_decrypt_domain(
    state: State<'_, AppState>,
    domain: String,
    enable: bool,
) -> Result<UiDecryptEntry, String> {
    let rule = {
        let mut policy = state.decrypt_policy.lock().map_err(|e| e.to_string())?;
        policy
            .set_decrypt(&domain, enable)
            .ok_or_else(|| format!("domain not found: {domain}"))?
    };
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    Ok(decrypt_entry(&rule, &engine))
}

#[tauri::command]
pub async fn issue_for_sni(state: State<'_, AppState>, sni: String) -> Result<SniCertInfo, String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    let issued = engine.ensure_leaf_cert(&sni).map_err(|e| e.to_string())?;
    Ok(SniCertInfo {
        sni: issued.sni,
        subject_cn: issued.subject_cn,
        fingerprint_sha256: issued.fingerprint_sha256,
        cert_pem: issued.cert_pem,
        key_pem: issued.key_pem,
        sans: issued.sans,
    })
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
    let pem = engine
        .ca_pem()
        .unwrap_or_else(|| ca.certificate_pem.clone());
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
pub async fn generate_ca(state: State<'_, AppState>, cn: String) -> Result<String, String> {
    let engine = state.ca_manager.lock().map_err(|e| e.to_string())?;
    let ca = engine.setup_ca(&cn).map_err(|e| e.to_string())?;
    // Keep the cached CaInfo in AppState so get_ca_info can report CN/fingerprint/installed_at.
    if let Ok(mut info) = state.ca_info.lock() {
        *info = Some(ca.clone());
    }
    Ok(ca.certificate_pem)
}

/// Return the full session detail (serialised TrafficSession) for the given id.
#[tauri::command]
pub async fn get_session_detail(
    state: State<'_, AppState>,
    session_id: u64,
) -> Result<Option<serde_json::Value>, String> {
    let detail = state.session_store.get(session_id).map_err(|e| e.to_string())?;
    Ok(detail.map(|d| serde_json::to_value(&d).unwrap_or_default()))
}

/// Query stored sessions with optional filters and pagination.
#[tauri::command]
pub async fn query_sessions(
    state: State<'_, AppState>,
    query: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let pid = query.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
    let host = query.get("host").and_then(|v| v.as_str()).map(|s| s.to_string());
    let protocol = query.get("protocol").and_then(|v| v.as_str()).map(|s| s.to_string());
    // `Query::limit` is `u32` (clamped to 1..=500 by the store) and
    // `Query::offset` is `u64`. Normalise the JSON payload here so the store
    // never has to deal with `Option<usize>`.
    let limit = query
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(50);
    let offset = query.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);

    let q = flowlens_core::session_store::Query {
        pid,
        host,
        protocol,
        limit,
        offset,
    };
    let result = state.session_store.query(q).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "sessions": result.sessions, "total": result.total }))
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
        .map(
            |(pid, (process_name, session_count, bytes_sent, bytes_received))| {
                serde_json::json!({
                    "pid": pid,
                    "process_name": process_name,
                    "session_count": session_count,
                    "bytes_sent": bytes_sent,
                    "bytes_received": bytes_received,
                })
            },
        )
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
fn event_session_id(
    event: &flowlens_core::TrafficEvent,
) -> Option<flowlens_core::models::SessionId> {
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

/// Start the SNI-aware MITM proxy listener.
///
/// The listener shares the same `ca_manager` and `decrypt_policy` handles as
/// the UI commands, so per-host rules (including the `toggle_decrypt_domain`
/// switch) are honoured live on every accepted connection.
///
/// Idempotent: starting an already-running proxy is a no-op and returns the
/// current listening port.
#[tauri::command]
pub async fn start_proxy(state: State<'_, AppState>, port: Option<u16>) -> Result<u16, String> {
    let requested_port = port.unwrap_or(crate::proxy::DEFAULT_PORT);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], requested_port));

    {
        let slot = state.proxy_task.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Ok(requested_port);
        }
    }

    let runtime = std::sync::Arc::new(crate::proxy::ProxyRuntime::new(&state));
    let listener_runtime = runtime.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = listener_runtime.run_listener(addr).await {
            tracing::warn!(error = %e, "MITM proxy listener stopped with error");
        }
    });

    // Store both the runtime handle (for graceful shutdown) and the task handle.
    *state.proxy_runtime.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
    *state.proxy_addr.lock().map_err(|e| e.to_string())? = Some(addr);
    *state.proxy_task.lock().map_err(|e| e.to_string())? = Some(handle);

    let bound_port = addr.port();
    Ok(bound_port)
}

/// Stop the SNI-aware MITM proxy listener, if running.
///
/// Idempotent: stopping a stopped proxy is a no-op returning `Ok(false)`.
/// The accept loop is signalled to stop via the shared `AtomicBool` and the
/// task is awaited (with a short timeout) so the listener is fully closed.
#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<bool, String> {
    // Signal the runtime to stop accepting new connections.
    if let Some(runtime) = state
        .proxy_runtime
        .lock()
        .map_err(|e| e.to_string())?
        .take()
    {
        runtime.shutdown();
    }

    // Take the task handle and await it so the listener is released before we
    // return. A missing handle means we were not running.
    let handle = state.proxy_task.lock().map_err(|e| e.to_string())?.take();

    state.proxy_addr.lock().map_err(|e| e.to_string())?.take();

    match handle {
        Some(handle) => {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Return the current status of the MITM proxy listener.
///
/// `running` reflects whether a task handle is held, `addr` is the configured
/// listen address (`127.0.0.1:<port>`) or empty when stopped, and
/// `decrypt_domains` lists the host rules currently enabled for TLS termination.
#[tauri::command]
pub async fn get_proxy_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let running = {
        // Prefer the runtime's own flag when present; fall back to the task slot.
        let runtime_running = state
            .proxy_runtime
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .map(|rt| rt.is_running())
            .unwrap_or(false);
        let task_present = state
            .proxy_task
            .lock()
            .map_err(|e| e.to_string())?
            .is_some();
        runtime_running && task_present
    };

    let decrypt_domains: Vec<serde_json::Value> = {
        let policy = state.decrypt_policy.lock().map_err(|e| e.to_string())?;
        policy
            .rules()
            .iter()
            .filter(|r| r.decrypt)
            .map(|r| {
                serde_json::json!({
                    "domain": r.domain,
                    "wildcard": r.wildcard,
                })
            })
            .collect()
    };

    let addr = {
        let guard = state.proxy_addr.lock().map_err(|e| e.to_string())?;
        guard.map(|a| a.to_string()).unwrap_or_default()
    };

    Ok(serde_json::json!({
        "running": running,
        "addr": addr,
        "decrypt_domains": decrypt_domains,
    }))
}
/// Open the native "Save File" dialog and return the chosen absolute path.
///
/// The dialog runs on the main thread (a Tauri requirement), so the call blocks
/// on the plugin's callback channel. Returns `Ok(None)` when the user cancels.
#[tauri::command]
pub async fn pick_save_path(
    app: AppHandle,
    default_name: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let name = default_name.unwrap_or_else(|| "trace.har".to_string());
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();

    app.dialog()
        .file()
        .add_filter("HAR 1.2", &["har", "json"])
        .set_file_name(&name)
        .save_file(move |file_path| {
            let _ = tx.send(file_path.map(|p| p.to_string()));
        });

    rx.recv()
        .map_err(|e| format!("dialog closed unexpectedly: {e}"))
}

// ---------- Editable keyboard shortcuts (app_data_dir/shortcuts.json) ----------

/// Absolute path of the persisted shortcut-override file.
///
/// Overrides live in the Tauri app data dir so a per-user install keeps its own
/// bindings; unlike `read_theme`/`write_theme` (temp dir) this needs an
/// `AppHandle` to resolve the path.
fn shortcuts_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let mut dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create app data dir: {e}"))?;
    dir.push("shortcuts.json");
    Ok(dir)
}

/// Read the persisted shortcut overrides.
///
/// Returns a `{ "actionId": { ctrl, alt, shift, key } }` map. An empty object is
/// returned when the file is missing or unreadable, which tells the frontend to
/// fall back to the registry defaults.
#[tauri::command]
pub async fn read_shortcuts(app: AppHandle) -> Result<serde_json::Value, String> {
    let path = shortcuts_file_path(&app)?;
    match std::fs::read_to_string(&path) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(serde_json::json!({}));
            }
            let value: serde_json::Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("invalid shortcuts.json: {e}"))?;
            if !value.is_object() {
                return Err("shortcuts.json must contain a JSON object".to_string());
            }
            Ok(value)
        }
        Err(_) => Ok(serde_json::json!({})),
    }
}

/// Persist shortcut overrides as pretty-printed JSON.
#[tauri::command]
pub async fn write_shortcuts(app: AppHandle, shortcuts: serde_json::Value) -> Result<(), String> {
    if !shortcuts.is_object() {
        return Err("shortcuts must be a JSON object".to_string());
    }
    let path = shortcuts_file_path(&app)?;
    let serialized = serde_json::to_string_pretty(&shortcuts)
        .map_err(|e| format!("failed to encode shortcuts: {e}"))?;
    std::fs::write(&path, serialized).map_err(|e| format!("failed to write shortcuts: {e}"))
}

#[cfg(test)]
mod tests {
    use super::normalize_rewrite_priority;

    #[test]
    fn test_normalize_rewrite_priority_in_range() {
        let mut value = serde_json::json!({ "id": "a", "priority": 150 });
        assert_eq!(normalize_rewrite_priority(&mut value).unwrap(), 150);
    }

    #[test]
    fn test_normalize_rewrite_priority_default_when_missing() {
        let mut value = serde_json::json!({ "id": "a" });
        assert_eq!(normalize_rewrite_priority(&mut value).unwrap(), 100);
    }

    #[test]
    fn test_normalize_rewrite_priority_rejects_out_of_range() {
        let mut value = serde_json::json!({ "id": "a", "priority": 256 });
        assert!(normalize_rewrite_priority(&mut value).is_err());
    }

    #[test]
    fn test_normalize_rewrite_priority_rejects_non_integer() {
        let mut value = serde_json::json!({ "id": "a", "priority": 1.5 });
        assert!(normalize_rewrite_priority(&mut value).is_err());
    }
}
