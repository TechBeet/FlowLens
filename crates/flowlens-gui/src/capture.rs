//! Capture module for FlowLens GUI
//! 
//! Manages capture state and session information.

use egui::{Ui, Button, Label};
use crate::styles::ThemeMode;

/// Capture state enum
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Stopped,
    Running,
    Paused,
}

/// Session information
#[derive(Clone)]
pub struct SessionInfo {
    pub name: String,
    pub start_time: std::time::Instant,
    pub events_count: usize,
    pub duration: f64,
}

impl SessionInfo {
    pub fn new(name: String) -> Self {
        Self {
            name,
            start_time: std::time::Instant::now(),
            events_count: 0,
            duration: 0.0,
        }
    }
    
    pub fn update(&mut self) {
        self.duration = self.start_time.elapsed().as_secs_f64();
    }
}

/// Render capture controls
pub fn render_capture_controls(ui: &mut Ui, state: &mut CaptureState, session: &mut SessionInfo) {
    session.update();
    
    ui.horizontal(|ui| {
        match state {
            CaptureState::Stopped => {
                if ui.button("Start").clicked() {
                    *state = CaptureState::Running;
                }
            }
            CaptureState::Running => {
                if ui.button("Stop").clicked() {
                    *state = CaptureState::Stopped;
                    session = &SessionInfo::new("New Session".to_string());
                }
                if ui.button("Pause").clicked() {
                    *state = CaptureState::Paused;
                }
            }
            CaptureState::Paused => {
                if ui.button("Resume").clicked() {
                    *state = CaptureState::Running;
                }
            }
        }
        
        // Session info
        ui.label(format!("{} ({:.1}s, {} events)", session.name, session.duration, session.events_count));
        
        // Mock mode
        ui.checkbox(&mut false, "Mock");
    });

}
