//! TUI and GUI for FlowLens

use std::sync::Arc;

use flowlens_core::event_bus::EventBus;

mod gui;
mod tui;

pub use gui::GuiFrontend;
pub use tui::TuiFrontend;

pub trait UiFrontend: Send + Sync {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()>;
    fn stop(&self) -> anyhow::Result<()>;
}
