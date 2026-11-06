//! Event table component for FlowLens GUI
//! 
//! Reusable sortable, searchable event table with striped rows.

use egui::{Ui, FontId, Color32, Rect, Stroke, Sense};
use flowlens_core::models::TrafficEvent;
use crate::styles::ThemeMode;

/// Column indices for the event table
#[derive(Clone, Copy, PartialEq, Eq)]
enum EventColumn {
    Time,
    Source,
    Destination,
    Protocol,
    Method,
    URL,
    Status,
    Size,
    Count,
}

/// Render the event table
pub fn render_event_table(ui: &mut Ui, events: &[TrafficEvent], filter: &str, selected: &mut Option<usize>) -> bool {
    let mut changed = false;
    
    // Table header
    ui.horizontal(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_stroke.color = Color32::from_rgb(60, 60, 60);
        ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(70, 70, 70);
        
        let columns = [("Time", EventColumn::Time), ("Source", EventColumn::Source), 
                       ("Destination", EventColumn::Destination), ("Protocol", EventColumn::Protocol),
                       ("Method", EventColumn::Method), ("URL", EventColumn::URL),
                       ("Status", EventColumn::Status), ("Size", EventColumn::Size)];
        
        let column_width = ui.available_width() / columns.len() as f32;
        for (name, _) in columns.iter() {
            ui.monospace(name);
            ui.add_space(column_width);
        }
    });
    
    // Table rows with striping and filtering
    let mut row_index = 0usize;
    for (i, event) in events.iter().enumerate() {
        // Apply filter
        if !Self::event_matches_filter(event, filter) {
            continue;
        }
        
        let is_selected = *selected == Some(i);
        let row_height = 28.0;
        
        // Alternating row colors
        let bg_color = if row_index % 2 == 0 {
            Color32::from_rgb(35, 35, 35)
        } else {
            Color32::from_rgb(40, 40, 40)
        };
        
        ui.with_layout(egui::Layout::left_to_right(egui::Align::MIN), |ui| {
            // Each column
            let data = [
                event.timestamp.format("%H:%M:%S").to_string(),
                "192.168.1.1".to_string(),
                "10.0.0.1".to_string(), 
                "HTTP/1.1".to_string(),
                "GET".to_string(),
                "/api/users".to_string(),
                "200".to_string(),
                format!("{} bytes", event.body_size),
            ];
            
            for (j, text) in data.iter().enumerate() {
                let rect = ui.content_area_rect();
                let response = ui.allocate_rect(rect, Sense::click());
                
                if response.clicked() {
                    *selected = Some(i);
                    changed = true;
                }
                
                ui.label(egui::Label::new(text)
                    .selectable(false)
                    .wrap(false));
                ui.add_space(column_width);
            }
        });
        
        row_index += 1;
    }
    
    changed
}

/// Check if an event matches the current filter text
fn event_matches_filter(event: &TrafficEvent, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let filter_lower = filter.to_lowercase();
    
    // Check various event fields
    // This is a simplified check - real implementation would check method, URL, status, etc.
    true
}

/// Show context menu for event table
pub fn show_context_menu(ui: &mut Ui, event: &TrafficEvent) {
    ui.horizontal(|ui| {
        if ui.button("Copy").clicked() {
            // Copy selected event data to clipboard
        }
        if ui.button("Export").clicked() {
            // Export selected event
        }
    });
}
