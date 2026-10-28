use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, Arc};
use tokio::task::JoinHandle;
use flowlens_core::TrafficStats;
use flowlens_core::BroadcastEventBus;

pub struct AppState {
    pub event_bus: Arc<BroadcastEventBus>,
    pub stats: Arc<Mutex<TrafficStats>>,
    pub running: Arc<AtomicBool>,
    pub pump_task: Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    pub fn new(event_bus: BroadcastEventBus) -> Self {
        Self {
            event_bus: Arc::new(event_bus),
            stats: Arc::new(Mutex::new(TrafficStats::default())),
            running: Arc::new(AtomicBool::new(false)),
            pump_task: Mutex::new(None),
        }
    }
}
