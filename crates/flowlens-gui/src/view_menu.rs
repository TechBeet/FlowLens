//! Menu bar for FlowLens GUI
//! 
//! File, Capture, View, and Help menus.

use egui::Ui;
use crate::styles::ThemeMode;

/// Render the main menu bar
pub fn render_menu_bar(ui: &mut Ui, theme: &ThemeManager) {
    egui::menu::bar(ui, |ui| {
        // File menu
        ui.menu_button("File", |ui| {
            if ui.button("About...").clicked() {
                // Show about dialog
            }
            if ui.button("Check for updates").clicked() {
                // Placeholder
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        
        // Capture menu
        ui.menu_button("Capture", |ui| {
            let running = ui.ctx().data().get_priority(false); // placeholder
            if ui.button(if running { "Stop Capture" } else { "Start Capture" }).clicked() {
                // Toggle capture
            }
            if ui.button("Pause").clicked() {
                // Pause capture
            }
            if ui.button("Resume").clicked() {
                // Resume capture
            }
            ui.checkbox(&mut false, "Mock Mode");
        });
        
        // View menu
        ui.menu_button("View", |ui| {
            if ui.button("Toggle Filter").clicked() {
                // Toggle filter panel
            }
            if ui.button("Toggle Statistics").clicked() {
                // Toggle statistics panel
            }
        });
        
        // Help menu
        ui.menu_button("Help", |ui| {
            if ui.button("About FlowLens").clicked() {
                // Show about
            }
            if ui.button("Documentation").clicked() {
                // Open docs
            }
            if ui.button("Report Issue").clicked() {
                // Open issue tracker
            }
        });
    });
}

