//! Statistics dashboard for FlowLens GUI
//! 
//! Protocol distribution chart, bytes/packets counters, top talkers.

use egui::Ui;

/// Render the statistics dashboard panel
pub fn render_statistics_panel(ui: &mut Ui, statistics: &crate::statistics::AppStatistics, palette: &crate::widgets::ColorPalette) {
    ui.heading("Statistics Dashboard");

    // Protocol distribution (text-based, no plot crate needed)
    ui.add_space(10.0);
    ui.strong("Protocol Distribution");
    for (k, v) in &statistics.protocol_counts {
        ui.label(format!("{}: {}", k, v));
    }

    // Bytes counters
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.strong("Bytes Sent:");
        ui.label(statistics.bytes_sent.to_string());
        ui.add_space(20.0);
        ui.strong("Bytes Received:");
        ui.label(statistics.bytes_received.to_string());
    });

    // Packets captured
    ui.horizontal(|ui| {
        ui.strong("Packets Captured:");
        ui.label(statistics.packets_captured.to_string());
    });

    // Top talkers
    ui.add_space(10.0);
    ui.strong("Top Talkers");
    ui.label("Loading top talkers...");
}

