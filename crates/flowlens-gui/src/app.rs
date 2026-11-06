//! GUI Application module for FlowLens
//! This module provides the GUI application structure and implementation.

use std::sync::Arc;
use std::time::Duration;

use flowlens_core::event_bus::EventBus;
use flowlens_core::models::TrafficEvent;
use flowlens_core::storage::StorageEngine;
use flowlens_core::tls::CaManager;
use flowlens_ui::UiFrontend;
use flowlens_gui::GuiFrontend;

/// The main GUI application struct
#[derive(Debug)]
pub struct GuiApp {
    /// Reference to the event bus for receiving traffic events
    pub event_bus: Arc<EventBus>,
    /// Reference to the storage engine
    pub storage: Option<StorageEngine>,
    /// TLS certificate manager
    pub ca_manager: Option<CaManager>,
    /// Whether using real capture or mock
    pub using_real_capture: bool,
}

impl GuiApp {
    /// Create a new GuiApp instance
    pub fn new(bus: Arc<EventBus>, running: Arc<std::sync::atomic::AtomicBool>) -> Self {
        GuiApp {
            event_bus: bus,
            storage: None,
            ca_manager: None,
            using_real_capture: false,
            running,
        }
    }

    /// Run the GUI application
    pub fn run(&mut self) -> anyhow::Result<()> {
        // Initialize the GUI frontend
        let gui = GuiFrontend::new();
        gui.set_theme(crate::styles::ThemeMode::Dark);

        // Start the GUI event loop
        gui.run(self.event_bus.clone())
    }

    /// Update the capture mode (real vs mock)
    pub fn set_capture_mode(&mut self, using_real: bool) {
        self.using_real_capture = using_real;
    }
}

impl UiFrontend for GuiApp {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        // Run the GUI event loop
        // This is a simplified implementation
        Ok(())
    }

    fn stop(&self) -> anyhow::Result<()> {
        // Cleanup resources
        Ok(())
    }
}