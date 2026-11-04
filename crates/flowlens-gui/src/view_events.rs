//! Card-based event view with colors

use flowlens_core::models::TrafficEvent;
use crate::styles::ColorPalette;

pub struct EventTableView {
    events: Vec<TrafficEvent>,
}

impl EventTableView {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, palette: &ColorPalette) {
        ui.label("Events");
    }
}