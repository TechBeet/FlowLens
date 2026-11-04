//! Styled filter input with focus ring

pub struct FilterView {
    filter: String,
}

impl FilterView {
    pub fn new() -> Self {
        Self {
            filter: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
    }
}