//! GUI frontend using egui/eframe

use std::sync::{Arc, Mutex};

use egui::{Color32, Widget};
use eframe::App;

use flowlens_core::{
    event_bus::{BroadcastEventBus, EventBus},
    models::{TrafficFilter, TrafficRow},
};

const MAX_EVENTS: usize = 5000;

struct FlowLensGui {
    events: Arc<Mutex<Vec<TrafficRow>>>,
    bus: Arc<BroadcastEventBus>,
    selected_index: Option<usize>,
    filter: TrafficFilter,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl FlowLensGui {
    fn new(bus: Arc<BroadcastEventBus>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            bus,
            selected_index: None,
            filter: TrafficFilter::default(),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn spawn_poll_task(&self) {
        let events = self.events.clone();
        let running = self.running.clone();
        running.store(true, std::sync::atomic::Ordering::SeqCst);
        let bus = self.bus.clone();
        tokio::spawn(async move {
            let mut rx = bus.subscribe(TrafficFilter::default());
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(50),
                    rx.recv(),
                )
                .await
                {
                    Ok(Ok(event)) => {
                        if let Some(row) = TrafficRow::from_event(&event) {
                            if let Ok(mut evts) = events.lock() {
                                evts.push(row);
                                while evts.len() > MAX_EVENTS {
                                    evts.remove(0);
                                }
                            }
                        }
                    }
                    Ok(Err(_)) => break,
                    Err(_) => continue,
                }
            }
        });
    }

    fn stop_poll_task(&self) {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn get_selected_event(&self) -> Option<TrafficRow> {
        self.selected_index.and_then(|i| {
            let events = self.events.lock().unwrap();
            events.get(i).cloned()
        })
    }

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

impl Drop for FlowLensGui {
    fn drop(&mut self) {
        self.stop_poll_task();
    }
}

impl App for FlowLensGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let repaint_ms = if self.events.lock().unwrap().is_empty() { 100u64 } else { 16 };
        ctx.request_repaint_after(std::time::Duration::from_millis(repaint_ms));

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FlowLens");
                ui.separator();
                let (conn, http, tls, sse, ws) = self.compute_stats();
                let count = self.events.lock().unwrap().len();
                ui.label(format!(
                    "conn={} http={} tls={} sse={} ws={} total={}",
                    conn, http, tls, sse, ws, count
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        self.events.lock().unwrap().clear();
                        self.selected_index = None;
                    }
                });
            });
        });

        egui::SidePanel::left("filter_panel").default_width(200.0).show(ctx, |ui| {
            ui.heading("Filter");
            ui.separator();
            ui.label("Host filter (comma sep): ");
            let mut host_str = self
                .filter
                .hosts
                .as_ref()
                .map(|h| h.join(","))
                .unwrap_or_default();
            egui::TextEdit::singleline(&mut host_str).ui(ui);
            if ui.button("Apply").clicked() {
                let new_hosts: Vec<String> = host_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                self.filter.hosts = if new_hosts.is_empty() {
                    None
                } else {
                    Some(new_hosts)
                };
                tracing::info!("Host filter updated: {:?}", self.filter.hosts);
            }
            if ui.button("Reset").clicked() {
                self.filter = TrafficFilter::default();
            }
            ui.separator();
            ui.heading("Stats");
            let (conn, http, tls, sse, ws) = self.compute_stats();
            ui.label(format!("Connection: {}", conn));
            ui.label(format!("HTTP: {}", http));
            ui.label(format!("TLS: {}", tls));
            ui.label(format!("SSE: {}", sse));
            ui.label(format!("WebSocket: {}", ws));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let events = self.events.lock().unwrap();
            if events.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for traffic events...\nStart a capture to see events here.");
                });
            } else {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Time");
                        ui.add_space(8.0);
                        ui.strong("Type");
                        ui.add_space(8.0);
                        ui.strong("Source");
                        ui.add_space(8.0);
                        ui.strong("Destination");
                        ui.add_space(8.0);
                        ui.strong("Method");
                        ui.add_space(8.0);
                        ui.strong("Path / Info");
                        ui.add_space(8.0);
                        ui.strong("Status");
                    });
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(f32::INFINITY)
                        .show(ui, |ui| {
                            for (i, row) in events.iter().enumerate() {
                                let color = match row.event_type.as_str() {
                                    "connection" => Color32::GREEN,
                                    "http_request" | "http_response" => Color32::BLUE,
                                    "tls" => Color32::YELLOW,
                                    "websocket" => Color32::from_rgb(200, 100, 255),
                                    "sse" => Color32::from_rgb(100, 200, 255),
                                    "grpc" => Color32::WHITE,
                                    "packet" => Color32::GRAY,
                                    "closed" => Color32::RED,
                                    "dropped" => Color32::RED,
                                    _ => Color32::WHITE,
                                };
                                ui.horizontal(|ui| {
                                    let ts_str = format!("{}", row.timestamp);
                                    ui.label(ts_str.chars().take(14).collect::<String>());
                                    ui.add_space(8.0);
                                    let text = egui::RichText::new(&row.event_type)
                                        .color(color)
                                        .size(11.0);
                                    ui.add(egui::Label::new(text));
                                    ui.add_space(8.0);
                                    ui.monospace(&row.src_addr);
                                    ui.add_space(8.0);
                                    ui.monospace(&row.dst_addr);
                                    ui.add_space(8.0);
                                    ui.label(&row.method);
                                    ui.add_space(8.0);
                                    let path_display = if row.path.len() > 35 {
                                        format!("{}...", &row.path[..35])
                                    } else {
                                        row.path.clone()
                                    };
                                    ui.label(path_display);
                                    ui.add_space(8.0);
                                    ui.monospace(&row.status);
                                });

                                if ui
                                    .interact(
                                        ui.max_rect(),
                                        egui::Id::new(i),
                                        egui::Sense::click(),
                                    )
                                    .clicked()
                                {
                                    if self.selected_index == Some(i) {
                                        self.selected_index = None;
                                    } else {
                                        self.selected_index = Some(i);
                                    }
                                }
                            }
                        });
                });
            }
        });

        if let Some(_sel) = self.selected_index {
            if let Some(row) = self.get_selected_event() {
                egui::SidePanel::right("detail_panel")
                    .default_width(380.0)
                    .show(ctx, |ui| {
                        ui.heading("Event Details");
                        ui.separator();
                        ui.label(format!("Time: {}", row.timestamp));
                        ui.label(format!("Type: {}", row.event_type));
                        ui.label(format!("Source: {}", row.src_addr));
                        ui.label(format!("Destination: {}", row.dst_addr));
                        ui.label(format!("Method: {}", row.method));
                        ui.label(format!("Path: {}", row.path));
                        ui.label(format!("Status: {}", row.status));
                        ui.label(format!("Bytes: {}", row.body_size));
                        ui.separator();
                        if ui.button("Close").clicked() {
                            self.selected_index = None;
                        }
                    });
            } else {
                self.selected_index = None;
            }
        }
    }
}

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
                .with_inner_size([1400.0, 850.0])
                .with_title("FlowLens GUI"),
            ..Default::default()
        };
        eframe::run_native(
            "FlowLens GUI",
            options,
            Box::new(|_cc| {
                let app = FlowLensGui::new(bus_clone.clone());
                app.spawn_poll_task();
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
