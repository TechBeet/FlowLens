//! TUI and GUI for FlowLens

use std::sync::Arc;

use flowlens_core::event_bus::EventBus;

mod tui;
mod gui;

pub use tui::TuiFrontend;
pub use gui::GuiFrontend;

pub trait UiFrontend: Send + Sync {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()>;
    fn stop(&self) -> anyhow::Result<()>;
}
