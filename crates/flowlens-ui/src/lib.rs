//! TUI and GUI for FlowLens

use std::sync::Arc;

use flowlens_core::event_bus::EventBus;

mod fonts;
mod gui;
mod tui;

pub use fonts::{find_cjk_font, install as install_fonts};
pub use gui::GuiFrontend;
pub use tui::TuiFrontend;

pub trait UiFrontend: Send + Sync {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()>;
    fn stop(&self) -> anyhow::Result<()>;
}
