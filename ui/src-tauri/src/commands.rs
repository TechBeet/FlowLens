use super::state::{AppState, TrafficStats};
use serde::Serialize;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, State};
use flowlens_core::{TrafficEvent, ConnectionKey, TransportProtocol, ProcessInfo, ProcessArch, MatchQuality, EventBus};

pub fn init() {
    println!("[FlowLens] Tauri backend initialized");
}

fn sample_conn() -> ConnectionKey {
    ConnectionKey {
        src_addr: "127.0.0.1:12345".parse().unwrap(),
        dst_addr: "93.184.216.34:80".parse().unwrap(),
        protocol: TransportProtocol::Tcp,
        connection_id: 1,
        timestamp: 0,
        interface_guid: None,
    }
}

fn sample_process() -> ProcessInfo {
    ProcessInfo {
        pid: 1234,
        name: "flowlens.exe".to_string(),
        exe_path: None,
        user_sid: "S-1-5-21-0".to_string(),
        arch: ProcessArch::X64,
        match_quality: MatchQuality::ExactTuple,
    }
}

#[tauri::command]
pub async fn start_capture(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut s = state.stats.lock().map_err(|e| e.to_string())?;
        if s.total > 0 {
            *s = TrafficStats {
                connections: 0,
                http: 0,
                tls: 0,
                sse: 0,
                websocket: 0,
                total: 0,
            };
        }
        s.total += 1;
        s.http += 1;
        s.connections += 1;
    }

    let conn = sample_conn();
    let event = TrafficEvent::ConnectionEstablished {
        conn: conn.clone(),
        process: sample_process(),
    };

    state.event_bus.publish(event.clone()).map_err(|e| e.to_string())?;

    app_handle.emit("traffic-event", event).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_capture(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut s = state.stats.lock().map_err(|e| e.to_string())?;
        s.total += 1;
        s.http += 1;
        s.connections += 1;
    }

    let conn = sample_conn();
    let event = TrafficEvent::ConnectionClosed { conn };

    state.event_bus.publish(event.clone()).map_err(|e| e.to_string())?;

    app_handle.emit("traffic-event", event).map_err(|e| e.to_string())?;

    Ok(())
}