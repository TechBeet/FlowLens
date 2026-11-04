//! Custom styled widgets with rounded corners, shadows, and hover effects

use egui::{Ui, Color32, vec2, Stroke, FontId};
use crate::styles::{ColorPalette, ThemeMode};

pub fn status_badge(ui: &mut Ui, text: &str, palette: &ColorPalette) -> egui::Response {
    let color = palette.primary;
    let text_color = Color32::WHITE;
    let desired_size = vec2(100.0 + 16.0, 24.0);
    let rect = egui::Rect::from_center_size(ui.cursor().min, desired_size);
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    ui.painter().rect_filled(rect, 12.0, color);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text.to_string(), egui::FontId::default(), text_color);
    response
}

/// Render a top header bar with title and theme toggle
pub fn header_bar(ui: &mut Ui, palette: &ColorPalette) -> egui::Response {
    let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), 56.0));
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.primary);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "FlowLens".to_string(), egui::FontId::default(), Color32::WHITE);
    response
}

/// Render a unicode icon for each protocol type
pub fn protocol_icon(protocol: &str, palette: &ColorPalette) -> Color32 {
    palette.primary
}


/// Render a card-based event item with hover effects
pub fn event_card(ui: &mut Ui, palette: &ColorPalette, protocol: &str, timestamp: String, source: String, dest: String, length: u64) -> egui::Response {
    let color = palette.primary;
    let card_height = 64.0;
    let card_width = ui.available_width();
    let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(card_width, card_height));
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    let painter = ui.painter();
    let bg_color = if response.hovered() { palette.bg_tertiary } else { palette.bg_secondary };
    painter.rect_filled(rect, 8.0, bg_color);
    painter.line_segment([rect.min, rect.max], Stroke::new(1.0_f32, palette.border_light));
    let indicator_rect = egui::Rect::from_min_size(rect.min + vec2(4.0, (card_height - 20.0) / 2.0), vec2(3.0, 20.0));
    painter.rect_filled(indicator_rect, 3.0, color);
    ui.painter().text(rect.min + vec2(12.0, 4.0), egui::Align2::LEFT_TOP, timestamp, egui::FontId::default(), palette.text);
    ui.painter().text(rect.min + vec2(12.0, 22.0), egui::Align2::LEFT_TOP, source, egui::FontId::default(), palette.text_muted);
    ui.painter().text(rect.min + vec2(12.0, 40.0), egui::Align2::LEFT_TOP, dest, egui::FontId::default(), palette.text_muted);
    let length_text = format!("{} bytes", length);
    ui.painter().text(rect.min + vec2(card_width - 150.0, 4.0), egui::Align2::LEFT_TOP, length_text, egui::FontId::default(), palette.text);
    let badge_rect = egui::Rect::from_min_size(rect.min + vec2(card_width - 110.0, card_height - 32.0), vec2(80.0, 24.0));
    painter.rect_filled(badge_rect, 12.0, color);
    ui.painter().text(badge_rect.center(), egui::Align2::CENTER_CENTER, protocol.to_string(), egui::FontId::default(), Color32::WHITE);
    response
}

/// Render a theme toggle button with icons
pub fn theme_toggle_button(ui: &mut egui::Ui, palette: &ColorPalette, current_mode: ThemeMode) -> egui::Response {
    let icon = match current_mode {
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
        ThemeMode::HighContrast => "HC",
    };

    let desired_size = vec2(40.0, 40.0);
    let rect = egui::Rect::from_center_size(ui.cursor().min, desired_size);
    let response = ui.interact(rect, egui::Id::new("theme_toggle"), egui::Sense::click());

    let painter = ui.painter();
    painter.rect_filled(rect, 12.0, palette.primary);
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(18.0), Color32::WHITE);

    response
}

/// Render a sidebar navigation item
pub fn sidebar_item(ui: &mut egui::Ui, palette: &ColorPalette, icon: &str, text: &str, selected: bool) -> egui::Response {
    let bg_color = if selected { palette.primary_hover } else { palette.surface_elevated };
    let text_color = if selected { Color32::WHITE } else { palette.text_muted };

    let desired_height = 48.0;
    let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), desired_height));
    let response = ui.allocate_rect(rect, egui::Sense::click());

    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, bg_color);

    painter.text(rect.min + vec2(12.0, 14.0), egui::Align2::LEFT_TOP, icon, egui::FontId::proportional(20.0), text_color);
    painter.text(rect.min + vec2(52.0, 14.0), egui::Align2::LEFT_TOP, text, egui::FontId::proportional(14.0), text_color);

    response
}

/// Render a right sidebar panel with details
pub fn right_panel(ui: &mut egui::Ui, palette: &ColorPalette, title: &str, content: &str) -> egui::Response {
    let width = 320.0;
    let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(width, ui.available_height()));
    let response = ui.allocate_rect(rect, egui::Sense::hover());

    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.surface_elevated);
    painter.line_segment([rect.min, rect.max], Stroke::new(1.0_f32, palette.border));

    painter.text(rect.min + vec2(12.0, 12.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(16.0, ), palette.text);
    painter.text(rect.min + vec2(12.0, 36.0), egui::Align2::LEFT_TOP, content, egui::FontId::proportional(14.0), palette.text_muted);

    response
}

/// Render a dashboard stat card with number and label
pub fn stat_card(ui: &mut Ui, palette: &ColorPalette, label: &str, value: &str, icon: &str) -> egui::Response {
    let card_width = ui.available_width();
    let card_height = 80.0;
    let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(card_width, card_height));
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, palette.surface_elevated);
    ui.painter().text(rect.min + vec2(12.0, 12.0), egui::Align2::LEFT_TOP, icon, egui::FontId::default(), palette.text_muted);
    ui.painter().text(rect.min + vec2(12.0, 32.0), egui::Align2::LEFT_TOP, value, egui::FontId::default(), palette.text);
    ui.painter().text(rect.min + vec2(12.0, 56.0), egui::Align2::LEFT_TOP, label, egui::FontId::default(), palette.text_muted);
    response
}



