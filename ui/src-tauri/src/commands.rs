use super::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinHandle;
use flowlens_core::{EventBus, TrafficEvent, TrafficFilter, TrafficRow};
use flowlens_capture::Engine;

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

    // Start the actual capture engine. Npcap is preferred; when it is not
    // available we fall back to the deterministic mock producer so the UI
    // still demonstrates a live event stream.
    let engine = Engine::auto();
    let bus: Arc<dyn EventBus> = state.event_bus.clone();
    if matches!(engine, Engine::Pcap(_)) {
        println!("Npcap engine selected");
        flowlens_capture::spawn_engine_pump(engine, bus);
    } else {
        println!("Npcap unavailable, using mock producer");
        flowlens_capture::spawn_mock_producer(bus);
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
