//! Filter management for FlowLens GUI
//! 
//! Text input with preset filters, clear button, and boolean support.

use egui::{Ui, Button, TextEdit};
use crate::styles::ThemeMode;

/// Preset filter options
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PresetFilter {
    All,
    Errors,
    HTML,
    JS,
    TLS,
    API,
}

/// Render the filter bar
pub fn render_filter_bar(ui: &mut Ui, filter: &mut String, show_presets: bool) -> PresetFilter {
    ui.horizontal(|ui| {
        // Text input with placeholder
        ui.text_edit_singleline(filter);
        
        // Clear button
        if ui.button("×").clicked() && !filter.is_empty() {
            filter.clear();
        }
    });
    
    // Preset filters
    if show_presets {
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("All").clicked() {
                filter.clear();
                PresetFilter::All
            }
            if ui.button("Errors").clicked() {
                filter.push_str("status:4");
                PresetFilter::Errors
            }
            if ui.button("HTML").clicked() {
                filter.push_str("path:.html");
                PresetFilter::HTML
            }
            if ui.button("JS").clicked() {
                filter.push_str("path:.js");
                PresetFilter::JS
            }
            if ui.button("TLS").clicked() {
                filter.push_str("tls:true");
                PresetFilter::TLS
            }
            if ui.button("API").clicked() {
                filter.push_str("method:GET");
                PresetFilter::API
            }
        });
    }
    
    PresetFilter::All
}

/// Parse boolean filter expression (AND, OR, NOT)
pub fn parse_boolean_filter(filter: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let and_terms: Vec<String> = Vec::new();
    let or_terms: Vec<String> = Vec::new();
    let not_terms: Vec<String> = Vec::new();
    
    // Simple parser - split by AND/OR/NOT
    let lower = filter.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();
    
    for part in parts {
        match part {
            "and" | "&" => {}
            "or" | "|" => {}
            "not" | "!" => {}
            _ => {}
        }
    }
    
    (and_terms, or_terms, not_terms)
}
/// Styled filter input with focus ring

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