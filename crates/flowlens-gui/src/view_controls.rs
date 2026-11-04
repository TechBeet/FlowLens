//! Styled control buttons with hover/press states

pub struct ControlsView;

impl ControlsView {
    #[allow(dead_code)]
    pub fn render(&self, ui: &mut egui::Ui) {
        ui.button("Start").clicked();
        ui.button("Stop").clicked();
        ui.button("Pause").clicked();
        ui.button("Resume").clicked();
    }
}