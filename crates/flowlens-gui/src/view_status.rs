//! Status bar for FlowLens GUI
//! 
//! Left: running/paused/stopped indicator
//! Center: event count and bytes
//! Right: connection speed and uptime

use egui::{Ui, Label, RichText};
use crate::styles::ThemeMode;

/// Render the status bar at the bottom of the window
pub fn render_status_bar(ui: &mut Ui, state: capture::CaptureState, event_count: usize, total_bytes: u64) {
    ui.horizontal(|ui| {
        // Left: status indicator
        let (text, color) = match state {
            capture::CaptureState::Running => ("Running", Color32::from_rgb(100, 200, 100)),
            capture::CaptureState::Paused => ("Paused", Color32::from_rgb(255, 200, 100)),
            capture::CaptureState::Stopped => ("Stopped", Color32::from_rgb(150, 150, 150)),
        };
        
        ui.put(RichText::new(text).color(color));
        ui.add_space(20.0);
        
        // Center: event count and bytes
        ui.label(RichText::new(format!("{} events | {} bytes", event_count, total_bytes)).strong());
        ui.add_space(20.0);
        
        // Right: connection speed and uptime
        ui.label("1.2 MB/s");
        ui.add_space(10.0);
        ui.label("00:12:34");
    });
}

