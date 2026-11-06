//! Capture controls for FlowLens GUI
//! 
//! Start, Stop, Pause, Resume buttons with session info display.

use egui::{Ui, Button, Frame, Stroke, Color32};
use crate::styles::ThemeMode;

/// Capture state enum
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Stopped,
    Running,
    Paused,
}

/// Render the capture controls panel
pub fn render_capture_controls(ui: &mut Ui, state: &mut CaptureState, session_name: &str, duration: f64, events_count: usize) {
    ui.horizontal(|ui| {
        // Start/Stop button
        if *state == CaptureState::Stopped {
            if ui.button("Start").clicked() {
                *state = CaptureState::Running;
            }
        } else {
            if ui.button("Stop").clicked() {
                *state = CaptureState::Stopped;
            }
        }
        
        // Pause/Resume button
        if *state == CaptureState::Running {
            if ui.button("Pause").clicked() {
                *state = CaptureState::Paused;
            }
        } else if *state == CaptureState::Paused {
            if ui.button("Resume").clicked() {
                *state = CaptureState::Running;
            }
        }
        
        // Mock mode toggle
        ui.checkbox(&mut false, "Mock Mode");
    });
    
    // Session info display - simplified, no Group API
    ui.horizontal(|ui| {
        ui.label("Session:");
        ui.monospace(session_name);
    });
    ui.horizontal(|ui| {
        ui.label("Duration:");
        ui.monospace(format!("{:.2}s", duration));
    });
    ui.horizontal(|ui| {
        ui.label("Events:");
        ui.monospace(format!("{}", events_count));
    });
}

/// Styled control buttons with hover/press states
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