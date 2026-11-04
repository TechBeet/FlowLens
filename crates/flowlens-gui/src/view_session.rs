//! Styled details panel with syntax highlighting colors

pub struct SessionView;

impl SessionView {
    pub fn render(&self, ui: &mut egui::Ui) {
        ui.label("Session Details");
    }
}