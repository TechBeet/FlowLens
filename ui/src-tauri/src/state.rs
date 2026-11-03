use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, Arc};
use tokio::task::JoinHandle;
use flowlens_core::TrafficStats;
use flowlens_core::BroadcastEventBus;
use flowlens_core::TrafficEvent;
use flowlens_core::InterceptEngine;
use flowlens_core::RewriteEngine;

pub struct AppState {
    pub event_bus: Arc<BroadcastEventBus>,
    pub stats: Arc<Mutex<TrafficStats>>,
    pub events: Arc<Mutex<Vec<TrafficEvent>>>,
    pub running: Arc<AtomicBool>,
    pub pump_task: Mutex<Option<JoinHandle<()>>>,
    pub intercept: Arc<InterceptEngine>,
    pub rewrite_engine: Mutex<RewriteEngine>,
}

impl AppState {
    pub fn new(event_bus: BroadcastEventBus) -> Self {
        Self {
            event_bus: Arc::new(event_bus),
            stats: Arc::new(Mutex::new(TrafficStats::default())),
            events: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            pump_task: Mutex::new(None),
            intercept: Arc::new(InterceptEngine::new()),
            rewrite_engine: Mutex::new(RewriteEngine::new()),
        }
    }
}
