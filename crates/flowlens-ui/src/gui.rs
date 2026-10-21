//! Modern GUI for FlowLens - Network Traffic Analyzer
//!
//! Features:
//! - Real-time traffic event display from EventBus
//! - Keyboard shortcuts for terminal/debug mode control
//! - Debug mode with raw event details
//! - Filter input with keyboard shortcut
//! - stdout logging for screen-reader compatibility

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use egui::{Color32, Widget};
use eframe::App;

use flowlens_core::{
    event_bus::{BroadcastEventBus, EventBus},
    models::{TrafficFilter, TrafficRow},
};

const MAX_EVENTS: usize = 5000;

/// Log UI state change to stdout for screen-reader compatibility
fn log_ui_state(message: &str) {
    eprintln!("[UI] {}", message);
}

#[derive(Clone)]
pub struct Theme {
    pub bg: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub row_even: Color32,
    pub row_odd: Color32,
    pub row_sel: Color32,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            bg: Color32::from_rgb(10, 10, 18),
            panel: Color32::from_rgb(18, 18, 28),
            border: Color32::from_rgb(40, 40, 60),
            text: Color32::from_rgb(235, 235, 250),
            text2: Color32::from_rgb(160, 160, 190),
            accent: Color32::from_rgb(100, 150, 255),
            success: Color32::from_rgb(70, 200, 110),
            warning: Color32::from_rgb(250, 160, 50),
            danger: Color32::from_rgb(255, 80, 80),
            info: Color32::from_rgb(70, 150, 255),
            row_even: Color32::from_rgb(14, 14, 22),
            row_odd: Color32::from_rgb(18, 18, 28),
            row_sel: Color32::from_rgb(35, 50, 80),
        }
    }

    pub fn light() -> Self {
        Theme {
            bg: Color32::from_rgb(248, 248, 252),
            panel: Color32::from_rgb(255, 255, 255),
            border: Color32::from_rgb(220, 220, 235),
            text: Color32::from_rgb(25, 25, 45),
            text2: Color32::from_rgb(80, 80, 110),
            accent: Color32::from_rgb(65, 105, 225),
            success: Color32::from_rgb(52, 168, 83),
            warning: Color32::from_rgb(251, 160, 30),
            danger: Color32::from_rgb(234, 67, 53),
            info: Color32::from_rgb(66, 133, 244),
            row_even: Color32::from_rgb(255, 255, 255),
            row_odd: Color32::from_rgb(250, 250, 254),
            row_sel: Color32::from_rgb(230, 235, 255),
        }
    }
}

fn event_color(event_type: &str, theme: &Theme) -> Color32 {
    match event_type {
        "connection" => theme.success,
        "http_request" => theme.info,
        "http_response" => Color32::from_rgb(70, 220, 140),
        "tls" => theme.warning,
        "websocket" => Color32::from_rgb(170, 80, 255),
        "sse" => Color32::from_rgb(0, 220, 210),
        _ => theme.text2,
    }
}

fn format_ts(ts: i64) -> String {
    if ts <= 0 { return "00:00:00".to_string(); }
    let s = ts / 1000;
    let ms = (ts % 1000) / 100;
    format!("{:02}:{:02}:{:02}.{}", (s/3600)%24, (s/60)%60, s%60, ms)
}

fn fmt_bytes(size: usize) -> String {
    if size >= 1024*1024 { format!("{:.2} MB", size as f64/(1024.0*1024.0)) }
    else if size >= 1024 { format!("{:.2} KB", size as f64/1024.0) }
    else { format!("{} B", size) }
}

fn trunc(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}...", &s[..n-3]) }
}

struct FlowLensGui {
    events: Arc<Mutex<Vec<TrafficRow>>>,
    bus: Arc<BroadcastEventBus>,
    selected: Option<usize>,
    filter: TrafficFilter,
    running: Arc<std::sync::atomic::AtomicBool>,
    theme: Theme,
    dark: bool,
    debug_mode: bool,
    show_filter_input: bool,
    filter_input: String,
}

impl FlowLensGui {
    fn new(bus: Arc<BroadcastEventBus>) -> Self {
        let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::new()));
        
        Self {
            events,
            bus,
            selected: None,
            filter: TrafficFilter::default(),
            running: running.clone(),
            theme: Theme::dark(),
            dark: true,
            debug_mode: false,
            show_filter_input: false,
            filter_input: String::new(),
        }
    }

    /// Subscribe to the EventBus and receive traffic events
    /// This is the key fix for proper event bus integration
    fn spawn_task(&self) {
        let ev = self.events.clone();
        let run = self.running.clone();
        let bus = self.bus.clone();
        run.store(true, std::sync::atomic::Ordering::SeqCst);
        
        tokio::spawn(async move {
            let mut rx = bus.subscribe(TrafficFilter::default());
            
            eprintln!("[INFO] EventBus subscription successful");
            
            while run.load(std::sync::atomic::Ordering::SeqCst) {
                match rx.blocking_recv() {
                    Ok(event) => {
                        if let Some(row) = TrafficRow::from_event(&event) {
                            if let Ok(mut v) = ev.lock() {
                                v.push(row);
                                // Auto-prune to MAX_EVENTS (oldest first)
                                while v.len() > MAX_EVENTS {
                                    v.remove(0);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            eprintln!("[INFO] Event subscription loop terminated");
        });
    }

    fn stop_task(&self) { 
        self.running.store(false, std::sync::atomic::Ordering::SeqCst); 
    }

    fn flush_events(&mut self) {
        if let Ok(mut events) = self.events.lock() {
            let count = events.len();
            events.clear();
            self.selected = None;
            log_ui_state(&format!("Flushed {} events", count));
        }
    }

    fn apply_filter(&mut self) {
        let nh: Vec<String> = self.filter_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        self.filter.hosts = if nh.is_empty() { None } else { Some(nh) };
        self.show_filter_input = false;
        log_ui_state(&format!("Filter applied: hosts={:?}", self.filter.hosts));
    }

    fn update_display_with_debug(&self) {
        let ev = self.events.lock().unwrap();
        let count = ev.len();
        if self.debug_mode {
            log_ui_state(&format!("Event count: {}", count));
            for (i, row) in ev.iter().take(5).enumerate() {
                log_ui_state(&format!("  [{}] {} {} -> {} {} {} {}",
                    i,
                    row.timestamp,
                    row.event_type,
                    row.src_addr,
                    row.dst_addr,
                    row.method,
                    row.status
                ));
            }
        }
    }

    fn get_selected(&self) -> Option<TrafficRow> {
        self.selected.and_then(|i| self.events.lock().unwrap().get(i).cloned())
    }

    fn stats(&self) -> HashMap<String, usize> {
        let ev = self.events.lock().unwrap();
        let mut m: HashMap<String, usize> = HashMap::new();
        m.insert("total".to_string(), ev.len());
        for r in ev.iter() { *m.entry(r.event_type.clone()).or_insert(0) += 1; }
        m
    }
}

impl Drop for FlowLensGui {
    fn drop(&mut self) { self.stop_task(); }
}

impl App for FlowLensGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = self.theme.clone();
        let repaint = if self.events.lock().unwrap().is_empty() { 100 } else { 16 };
        ctx.request_repaint_after(std::time::Duration::from_millis(repaint));

        // Top bar
        egui::TopBottomPanel::top("top").frame(
            egui::Frame::default().fill(theme.panel).stroke(egui::Stroke::new(1.0_f32, theme.border))
        ).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{25C8} FlowLens").color(theme.accent).size(17.0));
                ui.add_space(20.0);
                let st = self.stats();
                ui.label(format!("Events: {}", st.get("total").unwrap_or(&0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("\u{2715} Clear").clicked() {
                        self.events.lock().unwrap().clear();
                        self.selected = None;
                        log_ui_state("Events cleared via button");
                    }
                    ui.add_space(8.0);
                    if ui.button("d").clicked() {
                        self.debug_mode = !self.debug_mode;
                        log_ui_state(&format!("Debug mode: {}", if self.debug_mode { "ON" } else { "OFF" }));
                    }
                    if ui.button("f").clicked() {
                        self.flush_events();
                    }
                    if ui.button(if theme.bg.g() < 20 { "\u{2600}" } else { "\u{1F319}" }).clicked() {
                        self.dark = !self.dark;
                        self.theme = if self.dark { Theme::dark() } else { Theme::light() };
                        log_ui_state(&format!("Theme toggled to {}", if self.dark { "dark" } else { "light" }));
                    }
                });
            });
            
            // Show keyboard shortcuts in debug mode
            if self.debug_mode {
                ui.label(egui::RichText::new("Shortcuts: r-refresh | /-filter | d-debug | f-flush")
                    .color(theme.text2).size(11.0));
            }
        });

        // Handle keyboard shortcuts for debug mode
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            log_ui_state("Refresh stats triggered via keyboard");
            self.update_display_with_debug();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            log_ui_state("Filter input triggered via keyboard");
            self.show_filter_input = true;
            self.filter_input.clear();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            log_ui_state("Flush events triggered via keyboard");
            self.flush_events();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::D)) {
            self.debug_mode = !self.debug_mode;
            log_ui_state(&format!("Debug mode: {}", if self.debug_mode { "ON" } else { "OFF" }));
        }

        // Left panel
        egui::SidePanel::left("side").default_width(200.0).resizable(false).frame(
            egui::Frame::default().fill(theme.panel).stroke(egui::Stroke::new(1.0_f32, theme.border))
        ).show(ctx, |ui| {
            ui.strong("Filters");
            ui.add_space(6.0);
            ui.label("Host:");
            let mut hs = self.filter.hosts.as_ref().map(|h| h.join(",")).unwrap_or_default();
            egui::TextEdit::singleline(&mut hs).hint_text("example.com").ui(ui);
            if ui.button("Apply").clicked() {
                let nh: Vec<String> = hs.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                self.filter.hosts = if nh.is_empty() { None } else { Some(nh) };
                log_ui_state(&format!("Filter hosts updated: {:?}", self.filter.hosts));
            }
            if ui.button("Reset").clicked() {
                self.filter = TrafficFilter::default();
                log_ui_state("Filter reset to default");
            }
            if ui.button("Flush (f)").clicked() {
                self.flush_events();
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            ui.strong("Stats");
            ui.add_space(6.0);
            let st = self.stats();
            let cols = vec![
                ("connection", theme.success),
                ("http_request", theme.info),
                ("http_response", Color32::from_rgb(70, 220, 140)),
                ("tls", theme.warning),
                ("websocket", Color32::from_rgb(170, 80, 255)),
                ("sse", Color32::from_rgb(0, 220, 210)),
            ];
            for (k, c) in cols {
                if let Some(&n) = st.get(k) {
                    if n > 0 {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("\u{25CF}").color(c).size(9.0));
                            ui.add_space(4.0);
                            ui.label(format!("{}: {}", k, n));
                        });
                    }
                }
            }
            ui.add_space(4.0);
            ui.label(format!("Total: {}", st.get("total").unwrap_or(&0)));
            
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            
            ui.strong("Debug Info");
            ui.label(format!("Subscribers: {}", self.bus.subscriber_count()));
            ui.label(format!("Event count: {}", self.bus.get_event_count()));
            ui.label(format!("Debug Mode: {}", if self.debug_mode { "ON" } else { "OFF" }));
        });

        // Center panel
        egui::CentralPanel::default().frame(
            egui::Frame::default().fill(theme.bg).rounding(2.0)
        ).show(ctx, |ui| {
            let ev = self.events.lock().unwrap();
            if ev.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("\u{25C8}").color(theme.accent).size(56.0));
                        ui.add_space(16.0);
                        ui.label("No events captured yet");
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Start a capture to see network events").color(theme.text2));
                    });
                });
            } else {
                ui.horizontal(|ui| {
                    ui.strong("Time"); ui.add_space(6.0);
                    ui.strong("Type"); ui.add_space(6.0);
                    ui.strong("Source"); ui.add_space(6.0);
                    ui.strong("Dest"); ui.add_space(6.0);
                    ui.strong("Method"); ui.add_space(6.0);
                    ui.strong("Path"); ui.add_space(6.0);
                    ui.strong("Status"); ui.add_space(6.0);
                    ui.strong("Size");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{} events", ev.len()));
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical().id_salt("ev").show(ui, |ui| {
                    for (i, row) in ev.iter().enumerate() {
                        let sel = self.selected == Some(i);
                        let even = i % 2 == 0;
                        let bg = if sel { theme.row_sel } else if even { theme.row_even } else { theme.row_odd };

                        ui.painter().rect(
                            ui.min_rect(),
                            egui::Rounding::same(3.0),
                            bg,
                            egui::Stroke::new(0.5_f32, theme.border),
                        );
                        ui.set_min_size(egui::vec2(0.0, 24.0));

                        let tc = if sel { theme.accent } else { theme.text2 };
                        ui.label(egui::RichText::new(format_ts(row.timestamp)).color(tc).size(11.0));
                        ui.add_space(6.0);

                        let ec = event_color(&row.event_type, &theme);
                        ui.label(egui::RichText::new(&row.event_type).color(ec).size(11.0));
                        ui.add_space(6.0);

                        let ac = if sel { theme.accent } else { theme.text };
                        ui.monospace(egui::RichText::new(trunc(&row.src_addr, 14)).color(ac).size(11.0));
                        ui.add_space(6.0);
                        ui.monospace(egui::RichText::new(trunc(&row.dst_addr, 14)).color(ac).size(11.0));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(&row.method).color(ac).size(11.0));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(trunc(&row.path, 22)).color(theme.text).size(11.0));
                        ui.add_space(6.0);

                        let sc = if row.status.starts_with('2') { theme.success }
                            else if row.status.starts_with('3') { theme.info }
                            else if row.status.starts_with('4') { theme.warning }
                            else { theme.danger };
                        ui.label(egui::RichText::new(&row.status).color(sc).size(11.0));
                        ui.add_space(6.0);
                        let szc = if sel { theme.accent } else { theme.text2 };
                        ui.label(egui::RichText::new(fmt_bytes(row.body_size)).color(szc).size(11.0));

                        if ui.interact(ui.max_rect(), egui::Id::new(i), egui::Sense::click()).clicked() {
                            self.selected = if self.selected == Some(i) { None } else { Some(i) };
                        }
                    }
                });
            }
        });

        // Right panel
        if let Some(_s) = self.selected {
            if let Some(row) = self.get_selected() {
                let ts_str = format_ts(row.timestamp);
                let sz_str = fmt_bytes(row.body_size);
                
                egui::SidePanel::right("details").default_width(360.0).resizable(true).frame(
                    egui::Frame::default().fill(theme.panel).stroke(egui::Stroke::new(1.0_f32, theme.border))
                ).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Details");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("\u{2715}").clicked() { self.selected = None; }
                        });
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    let ec = event_color(&row.event_type, &theme);
                    ui.label(egui::RichText::new(&row.event_type).color(ec).size(15.0));
                    ui.add_space(16.0);

                    let pairs: Vec<(&str, &str)> = vec![
                        ("Timestamp", &ts_str),
                        ("Source", &row.src_addr),
                        ("Destination", &row.dst_addr),
                        ("Method", &row.method),
                        ("Path", &row.path),
                        ("Status", &row.status),
                        ("Size", &sz_str),
                    ];
                    for (label, val) in pairs {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{}:", label)).color(theme.text2).size(12.0));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(val).color(theme.text).size(12.0));
                        });
                    }
                });
            } else {
                self.selected = None;
            }
        }

        // Filter input window (shown when '/' is pressed)
        if self.show_filter_input {
            egui::Window::new("Filter Input").default_width(400.0).show(ctx, |ui| {
                ui.label("Enter host filters (comma-separated):");
                ui.add_space(8.0);
                egui::TextEdit::multiline(&mut self.filter_input)
                    .hint_text("example.com, api.example.org")
                    .desired_width(f32::INFINITY)
                    .show(ui);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        self.apply_filter();
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_filter_input = false;
                        log_ui_state("Filter input canceled");
                    }
                });
            });
        }

        // Debug panel at bottom (when debug mode is on)
        if self.debug_mode {
            egui::TopBottomPanel::bottom("debug").default_height(150.0).show(ctx, |ui| {
                ui.strong("Debug View - Raw Event Details");
                ui.separator();
                let ev = self.events.lock().unwrap();
                for (i, row) in ev.iter().take(20).enumerate() {
                    ui.label(format!(
                        "{}. [{}] {} {} -> {} | {} {} | {} | {}",
                        i, row.timestamp, row.event_type, row.src_addr, row.dst_addr,
                        row.method, row.status, fmt_bytes(row.body_size), trunc(&row.path, 30)
                    ));
                }
            });
        }
    }
}

pub struct GuiFrontend {
    bus: Arc<BroadcastEventBus>,
}

impl GuiFrontend {
    pub fn new(bus: Arc<BroadcastEventBus>) -> Self { Self { bus } }
}

impl crate::UiFrontend for GuiFrontend {
    fn run(&self, _bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        let bc = self.bus.clone();
        let opts = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1600.0, 900.0])
                .with_title("FlowLens - Network Traffic Analyzer"),
            ..Default::default()
        };
        eframe::run_native("FlowLens", opts, Box::new(|_cc| {
            let app = FlowLensGui::new(bc.clone());
            app.spawn_task();
            Ok(Box::new(app))
        })).map_err(|e| anyhow::anyhow!("GUI error: {:?}", e))?;
        Ok(())
    }

    fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("GUI stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_creation() {
        let bus = Arc::new(BroadcastEventBus::new(64));
        let g = FlowLensGui::new(bus);
        assert_eq!(g.events.lock().unwrap().len(), 0);
    }
    #[test]
    fn test_dark_theme() {
        let t = Theme::dark();
        assert!(t.bg.r() < 20 && t.bg.g() < 20 && t.bg.b() < 22);
    }
    #[test]
    fn test_light_theme() {
        let t = Theme::light();
        assert!(t.bg.r() > 245 && t.bg.g() > 245 && t.bg.b() > 248);
    }
    #[test]
    fn test_fmt_bytes() {
        assert_eq!(fmt_bytes(500), "500 B");
        assert_eq!(fmt_bytes(1500), "1.46 KB");
        assert_eq!(fmt_bytes(1500000), "1.43 MB");
    }
    #[test]
    fn test_trunc() {
        assert_eq!(trunc("hello", 10), "hello");
        assert!(trunc("hello world long text", 8).ends_with("..."));
    }
    #[test]
    fn test_format_ts() {
        assert_eq!(format_ts(0), "00:00:00");
        assert_eq!(format_ts(123456789), "10:17:36.7");
    }
}

