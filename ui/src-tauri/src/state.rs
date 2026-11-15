use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, Arc};
use tokio::task::JoinHandle;
use flowlens_core::TrafficStats;
use flowlens_core::BroadcastEventBus;
use flowlens_core::TrafficEvent;
use flowlens_core::InterceptEngine;
use flowlens_core::RewriteEngine;
use flowlens_tls::{CaInfo, CaManager, DomainPolicy, InMemoryTlsEngine};
use crate::proxy::ProxyRuntime;

pub struct AppState {
    pub event_bus: Arc<BroadcastEventBus>,
    pub stats: Arc<Mutex<TrafficStats>>,
    pub events: Arc<Mutex<Vec<TrafficEvent>>>,
    pub running: Arc<AtomicBool>,
    pub pump_task: Mutex<Option<JoinHandle<()>>>,
    pub proxy_task: Mutex<Option<JoinHandle<()>>>,
    pub proxy_runtime: Mutex<Option<Arc<ProxyRuntime>>>,
    pub proxy_addr: Mutex<Option<std::net::SocketAddr>>,
    pub intercept: Arc<InterceptEngine>,
    pub rewrite_engine: Mutex<RewriteEngine>,
    pub ca_manager: Arc<Mutex<InMemoryTlsEngine>>,
    pub decrypt_policy: Arc<Mutex<DomainPolicy>>,
    pub ca_info: Arc<Mutex<Option<CaInfo>>>,
}

impl AppState {
    pub fn new(event_bus: BroadcastEventBus) -> Self {
        let mut ca_engine = InMemoryTlsEngine::new();
        let ca = ca_engine.generate_ca("FlowLens CA").ok();

        Self {
            event_bus: Arc::new(event_bus),
            stats: Arc::new(Mutex::new(TrafficStats::default())),
            events: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            pump_task: Mutex::new(None),
            proxy_task: Mutex::new(None),
            proxy_runtime: Mutex::new(None),
            proxy_addr: Mutex::new(None),
            intercept: Arc::new(InterceptEngine::new()),
            rewrite_engine: Mutex::new(RewriteEngine::new()),
            ca_manager: Arc::new(Mutex::new(ca_engine)),
            decrypt_policy: Arc::new(Mutex::new(DomainPolicy::new())),
            ca_info: Arc::new(Mutex::new(ca)),
        }
    }
}
