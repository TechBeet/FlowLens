use std::sync::Mutex;
use flowlens_core::BroadcastEventBus;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct TrafficStats {
    pub connections: u64,
    pub http: u64,
    pub tls: u64,
    pub sse: u64,
    pub websocket: u64,
    pub total: u64,
}

pub struct AppState {
    pub event_bus: BroadcastEventBus,
    pub stats: Mutex<TrafficStats>,
}

impl AppState {
    pub fn new(event_bus: BroadcastEventBus) -> Self {
        Self {
            event_bus,
            stats: Mutex::new(TrafficStats {
                connections: 0,
                http: 0,
                tls: 0,
                sse: 0,
                websocket: 0,
                total: 0,
            }),
        }
    }
}