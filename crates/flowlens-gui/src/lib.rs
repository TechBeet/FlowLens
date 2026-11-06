//! FlowLens GUI frontend (eframe/egui)
//!
//! Implements `UiFrontend` trait for the native window GUI.

use std::sync::Arc;

use flowlens_core::event_bus::EventBus;

mod styles;
mod statistics;
mod widgets;
mod theme_manager;
mod app;
#[allow(dead_code)]
mod event_adapter;
mod capture;
mod mock;
mod view_controls;
mod view_events;
mod view_filter;
mod view_session;
mod view_stats;
mod view_menu;
mod view_status;

pub use app::GuiApp;

/// GUI frontend implementing the `UiFrontend` trait.
pub struct GuiFrontend {
    running: Arc<std::sync::atomic::AtomicBool>,
    theme: Option<crate::theme_manager::ThemeManager>,
}

impl GuiFrontend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            theme: None,
        }
    }

    pub fn set_theme(&mut self, mode: crate::styles::ThemeMode) {
        self.theme = Some(crate::theme_manager::ThemeManager::new(mode));
    }
}

impl Default for GuiFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl flowlens_ui::UiFrontend for GuiFrontend {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        self.running.store(true, std::sync::atomic::Ordering::SeqCst);

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("FlowLens")
                .with_inner_size([1400.0, 900.0])
                .with_min_inner_size([1200.0, 700.0])
                .with_resizable(true),
            ..Default::default()
        };

        let app = GuiApp::new(bus, self.running.clone());

        eframe::run_native(
            "FlowLens",
            native_options,
            Box::new(|_cc| Ok(Box::new(app))),
        )
        .map_err(|e| anyhow::anyhow!("eframe run failed: {e}"))
    }

    fn stop(&self) -> anyhow::Result<()> {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

