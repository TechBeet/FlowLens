//! Service control and state management.

use std::sync::Arc;

use flowlens_capture::Engine;
use flowlens_storage::InMemoryStorage;
use tokio::sync::RwLock;

pub struct ServiceState {
    pub storage: Arc<InMemoryStorage>,
    pub capture_running: Arc<RwLock<bool>>,
    pub engine: Arc<RwLock<Option<Engine>>>,
}

impl ServiceState {
    pub async fn start_capture(&self) -> anyhow::Result<()> {
        let mut running = self.capture_running.write().await;
        if *running {
            return Ok(());
        }
        // Engine selection: default to Mock for now; real engine via config.
        let engine = Engine::new_mock();
        *self.engine.write().await = Some(engine);
        *running = true;
        tracing::info!("Capture started");
        Ok(())
    }

    pub async fn stop_capture(&self) -> anyhow::Result<()> {
        let mut running = self.capture_running.write().await;
        if !*running {
            return Ok(());
        }
        *self.engine.write().await = None;
        *running = false;
        tracing::info!("Capture stopped");
        Ok(())
    }

    pub async fn is_capturing(&self) -> bool {
        *self.capture_running.read().await
    }
}

pub async fn run_console(state: ServiceState) -> anyhow::Result<()> {
    tracing::info!("FlowLens service running in console mode");
    // Start IPC server
    let ipc_state = state.clone_for_ipc();
    tokio::spawn(async move {
        if let Err(e) = crate::ipc::run_ipc_server(ipc_state).await {
            tracing::error!("IPC server error: {}", e);
        }
    });

    // Keep running until Ctrl+C
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");
    state.stop_capture().await.ok();
    Ok(())
}

#[cfg(windows)]
pub async fn run_as_service(state: ServiceState) -> anyhow::Result<()> {
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState as WinServiceState,
        ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => ServiceControlHandlerResult::NoError,
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("FlowLens", event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: WinServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    tracing::info!("Windows service running");
    // Start IPC
    let ipc_state = state.clone_for_ipc();
    tokio::spawn(async move {
        if let Err(e) = crate::ipc::run_ipc_server(ipc_state).await {
            tracing::error!("IPC server error: {}", e);
        }
    });

    // Run until stopped
    tokio::signal::ctrl_c().await.ok();
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: WinServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;
    Ok(())
}

// Helper to clone state for IPC (storage is Arc, so fine)
impl ServiceState {
    pub fn clone_for_ipc(&self) -> ServiceState {
        ServiceState {
            storage: self.storage.clone(),
            capture_running: self.capture_running.clone(),
            engine: self.engine.clone(),
        }
    }
}