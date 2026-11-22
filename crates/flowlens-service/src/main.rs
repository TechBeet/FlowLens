//! FlowLens Windows service entry point.
//!
//! Supports two modes:
//! - `--console`: run in foreground (for development/debugging).
//! - (default): register/run as a Windows service.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info};

mod ipc;
mod service;

use flowlens_capture::Engine;
use flowlens_storage::{InMemoryStorage, StorageEngine};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let console_mode = args.iter().any(|a| a == "--console" || a == "-c");

    info!("FlowLens service starting (console_mode={})", console_mode);

    // Shared state
    let storage = Arc::new(InMemoryStorage::new());
    let _ = storage.create_tables();

    let state = service::ServiceState {
        storage: storage.clone(),
        capture_running: Arc::new(RwLock::new(false)),
        engine: Arc::new(RwLock::new(None::<Engine>)),
    };

    if console_mode {
        info!("Running in console mode");
        if let Err(e) = service::run_console(state).await {
            error!("Console mode failed: {}", e);
            std::process::exit(1);
        }
    } else {
        #[cfg(windows)]
        {
            if let Err(e) = service::run_as_service(state).await {
                error!("Service mode failed: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(not(windows))]
        {
            info!("Non-Windows: falling back to console mode");
            if let Err(e) = service::run_console(state).await {
                error!("Console mode failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}