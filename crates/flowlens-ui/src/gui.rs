//! GUI frontend using egui/eframe

use std::sync::{Arc, Mutex};

use egui::Color32;
use eframe::App;

use flowlens_core::{
    event_bus::{BroadcastEventBus, EventBus},
    models::{TrafficFilter, TrafficRow},
};

/// Maximum number of events to keep in memory for the GUI
const MAX_EVENTS: usize = 5000;

/// The main GUI app state
struct FlowLensGui {
    events: Arc<Mutex<Vec<TrafficRow>>>,
    bus: Arc<BroadcastEventBus>,
    selected_index: Option<usize>,
    filter: TrafficFilter,
}

impl FlowLensGui {
    fn new(bus: Arc<BroadcastEventBus>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            bus,
            selected_index: None,
            filter: TrafficFilter::default(),
        }
    }

    /// Poll for new events from the event bus
    fn poll_events(&mut self) {
        // Create a fresh receiver each poll to get latest events
        let mut rx = self.bus.subscribe(self.filter.clone());

        // Try to drain all available events without blocking
        let mut count = 0;
        while count < 200 {
            match rx.try_recv() {
                Ok(event) => {
                    if let Some(row) = TrafficRow::from_event(&event) {
                        let mut events = self.events.lock().unwrap();
                        events.push(row);
                        // Keep at most MAX_EVENTS by removing oldest
                        while events.len() > MAX_EVENTS {
                            events.remove(0);
                        }
                    }
                    count += 1;
                }
                Err(_) => break,
            }
        }
    }

    /// Get the currently selected event (returns owned copy)
    fn get_selected_event(&self) -> Option<TrafficRow> {
        self.selected_index.and_then(|i| {
            let events = self.events.lock().unwrap();
            events.get(i).cloned()
        })
    }

    /// Update stats (counts by type)
    fn compute_stats(&self) -> (usize, usize, usize, usize, usize) {
        let events = self.events.lock().unwrap();
        let mut conn = 0;
        let mut http = 0;
        let mut tls = 0;
        let mut sse = 0;
        let mut ws = 0;
        for row in events.iter() {
            match row.event_type.as_str() {
                "connection" => conn += 1,
                "http_request" | "http_response" => http += 1,
                "tls" => tls += 1,
                "sse" => sse += 1,
                "websocket" => ws += 1,
                _ => {}
            }
        }
        (conn, http, tls, sse, ws)
    }
}

impl App for FlowLensGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        // Top panel with title and total count
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FlowLens GUI");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let events = self.events.lock().unwrap();
                    ui.label(format!("Events: {}", events.len()));
                });
            });
        });

        // Left panel with statistics
        egui::SidePanel::left("stats_panel").show(ctx, |ui| {
            ui.heading("Statistics");
            ui.separator();
            let (conn, http, tls, sse, ws) = self.compute_stats();

            ui.collapsing("Connections", |ui| {
                ui.label(format!("{}", conn));
            });
            ui.collapsing("HTTP", |ui| {
                ui.label(format!("{}", http));
            });
            ui.collapsing("TLS", |ui| {
                ui.label(format!("{}", tls));
            });
            ui.collapsing("SSE", |ui| {
                ui.label(format!("{}", sse));
            });
            ui.collapsing("WebSocket", |ui| {
                ui.label(format!("{}", ws));
            });

            ui.separator();
            if ui.button("Clear All").clicked() {
                self.events.lock().unwrap().clear();
                self.selected_index = None;
            }
            if ui.button("Refresh").clicked() {
                self.poll_events();
            }
        });
    }
}

/// GUI frontend implementation
pub struct GuiFrontend {
    bus: Arc<BroadcastEventBus>,
}

impl GuiFrontend {
    pub fn new(bus: Arc<BroadcastEventBus>) -> Self {
        Self { bus }
    }
}

impl crate::UiFrontend for GuiFrontend {
    fn run(&self, _bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        let bus_clone = self.bus.clone();
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 800.0])
                .with_title("FlowLens GUI"),
            ..Default::default()
        };
        eframe::run_native(
            "FlowLens GUI",
            options,
            Box::new(|_cc| {
                let app = FlowLensGui::new(bus_clone.clone());
                Ok(Box::new(app))
            }),
        )
        .map_err(|e| anyhow::anyhow!("GUI error: {:?}", e))?;
        Ok(())
    }

    fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("GUI frontend stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_app_creation() {
        let bus = Arc::new(BroadcastEventBus::new(64));
        let gui = FlowLensGui::new(bus);
        assert_eq!(gui.events.lock().unwrap().len(), 0);
        assert_eq!(gui.selected_index, None);
    }

    #[test]
    fn test_compute_stats_empty() {
        let bus = Arc::new(BroadcastEventBus::new(64));
        let gui = FlowLensGui::new(bus);
        let (conn, http, tls, sse, ws) = gui.compute_stats();
        assert_eq!(conn, 0);
        assert_eq!(http, 0);
        assert_eq!(tls, 0);
        assert_eq!(sse, 0);
        assert_eq!(ws, 0);
    }
}
