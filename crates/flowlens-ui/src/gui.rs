//! FlowLens GUI 图形界面 - 网络流量分析器

use std::sync::{Arc, Mutex};
use eframe::App;
use egui::{Color32, Layout, FontData, FontDefinitions, FontFamily, Align};
use flowlens_core::{event_bus::EventBus, models::{TrafficFilter, TrafficRow}};

const BASE_FONT_SIZE: f32 = 24.0;
const HEADING_FONT_SIZE: f32 = 30.0;
const STATUS_FONT_SIZE: f32 = 20.0;
const CELL_FONT_SIZE: f32 = 22.0;
const MAX_EVENTS: usize = 5000;
const AUTO_PRUNE_THRESHOLD: usize = 500;

#[allow(dead_code)]
fn log_ui(msg: &str) { eprintln!("[UI] {}", msg); }

fn setup_chinese_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates: Vec<&str> = if cfg!(target_os = "windows") {
        vec![r"C:\Windows\Fonts\msyh.ttc", r"C:\Windows\Fonts\msyhbd.ttc", r"C:\Windows\Fonts\Deng.ttf", r"C:\Windows\Fonts\simsun.ttc", r"C:\Windows\Fonts\simhei.ttf"]
    } else if cfg!(target_os = "macos") {
        vec!["/System/Library/Fonts/Supplemental/SF Pro Text.otf", "/System/Library/Fonts/STHeiti Light.ttc", "/System/Library/Fonts/PingFang.ttc"]
    } else {
        vec!["/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.otf", "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf"]
    };
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fd = FontData::from_owned(bytes);
            fd.index = 0;
            let base = path.rsplit(|c| c == '\\' || c == '/').next().unwrap_or("font");
            let name = base.trim_end_matches('.').trim_end_matches("ttc").trim_end_matches("ttf").trim_end_matches("otf");
            let name = if name.is_empty() { "chinese" } else { name };
            let name: String = name.to_string();
            let mut prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
            prop.retain(|n| n != &name); prop.insert(0, name.clone());
            let mut mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
            mono.retain(|n| n != &name); mono.insert(0, name.clone());
            fonts.font_data.insert(name.clone(), Arc::new(fd));
            fonts.families.insert(FontFamily::Proportional, prop);
            fonts.families.insert(FontFamily::Monospace, mono);
            log_ui(&format!("Loaded Chinese font: {} ({}px)", path, BASE_FONT_SIZE as u32));
            break;
        }
    }
    ctx.set_fonts(fonts);
    ctx.set_pixels_per_point(1.25);
}

// ─── Helper functions ───
fn event_type_cn(e: &str) -> String {
    match e { "connection" => "连接".to_string(), "http_request" => "HTTP 请求".to_string(),
        "http_response" => "HTTP 响应".to_string(), "tls" => "TLS".to_string(),
        "websocket" => "WebSocket".to_string(), "sse" => "SSE".to_string(), _ => e.to_string() }
}
fn event_color(e: &str, dark: bool) -> Color32 {
    let c = |r,g,b| Color32::from_rgb(r,g,b);
    match e { "connection" => c(70,220,120), "http_request" => c(80,160,255),
        "http_response" => c(80,230,130), "tls" => c(255,170,60),
        "websocket" => c(180,90,255), "sse" => c(0,230,220),
        _ => if dark { c(170,170,200) } else { c(90,90,120) } }
}
fn format_ts(ts: i64) -> String {
    if ts <= 0 { return "00:00:00".to_string(); }
    let s = ts / 1000; let ms = (ts % 1000) / 100;
    format!("{:02}:{:02}:{:02}.{}", (s/3600)%24, (s/60)%60, s%60, ms)
}
fn fmt_bytes(size: usize) -> String {
    if size >= 1024*1024 { format!("{:.2} MB", size as f64/(1024.0*1024.0)) }
    else if size >= 1024 { format!("{:.2} KB", size as f64/1024.0) }
    else { format!("{} B", size) }
}
fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}

// ─── GUI Frontend struct ───
pub struct GuiFrontend {
    events: Arc<Mutex<Vec<TrafficRow>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    debug_mode: bool,
    dark_mode: bool,
    filter_input: String,
    show_filter_input: bool,
    status_msg: String,
    selected_row: Option<usize>,
    bus: Option<Arc<dyn EventBus>>,
}
impl Default for GuiFrontend {
    fn default() -> Self {
        Self { events: Arc::new(Mutex::new(Vec::new())), running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            debug_mode: false, dark_mode: true, filter_input: String::new(), show_filter_input: false,
            status_msg: "就绪".to_string(), selected_row: None, bus: None }
    }
}
impl GuiFrontend {
    pub fn new() -> Self { Self::default() }
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>) -> Self { self.bus = Some(bus); self }
    fn start_capture(&mut self) {
        if let Some(bus) = &self.bus {
            self.start_capture_inner(bus.clone());
        }
    }
    fn start_capture_inner(&self, bus: Arc<dyn EventBus>) {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) { return; }
        self.running.store(true, std::sync::atomic::Ordering::SeqCst);
        let events_arc = self.events.clone();
        let running = self.running.clone();
        std::thread::spawn(move || {
            let mut rx = bus.subscribe(TrafficFilter::default());
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                match rx.blocking_recv() {
                    Ok(event) => {
                        if let Ok(mut evts) = events_arc.lock() {
                            if let Some(row) = TrafficRow::from_event(&event) {
                                evts.push(row);
                                let len = evts.len();
                                if len > MAX_EVENTS {
                                    evts.drain(..len - AUTO_PRUNE_THRESHOLD);
                                }
                            }
                        }
                    } Err(_) => break,
                }
            }
        });
    }
    fn clear_events(&mut self) { self.events.lock().unwrap().clear(); self.selected_row = None; self.status_msg = "已清空".to_string(); log_ui("事件已清空"); }
    fn toggle_debug(&mut self) { self.debug_mode = !self.debug_mode; self.status_msg = if self.debug_mode { "调试模式已开启" } else { "调试模式已关闭" }.to_string(); }
}

// ─── egui App 实现 ───
impl App for GuiFrontend {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dark = self.dark_mode;
        let evt_count = self.events.lock().map(|e| e.len()).unwrap_or(0);

        ctx.style_mut(|style| {
            style.visuals.extreme_bg_color = if dark { Color32::from_rgb(18, 18, 28) } else { Color32::from_rgb(238, 240, 245) };
            style.visuals.panel_fill = if dark { Color32::from_rgb(24, 24, 36) } else { Color32::from_rgb(246, 247, 250) };
            style.visuals.override_text_color = if dark { Some(Color32::from_rgb(230, 235, 245)) } else { Some(Color32::from_rgb(30, 30, 50)) };
        });

        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(egui::RichText::new("FlowLens - Network Traffic Analyzer")
                    .color(if dark { Color32::from_rgb(0, 210, 255) } else { Color32::from_rgb(0, 100, 200) })
                    .size(HEADING_FONT_SIZE));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Exit").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
            });
        });

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Start Capture").clicked() { self.start_capture(); self.status_msg = "Capturing".to_string(); log_ui("Start capture"); }
                if ui.button("Stop").clicked() { self.running.store(false, std::sync::atomic::Ordering::SeqCst); self.status_msg = "Stopped".to_string(); log_ui("Stop capture"); }
                if ui.button("Clear").clicked() { self.clear_events(); }
                if ui.button("Debug Mode").clicked() { self.toggle_debug(); }
                if ui.button("Theme").clicked() { self.dark_mode = !self.dark_mode; log_ui("Toggle theme"); }
                if ui.button("Filter").clicked() { self.show_filter_input = !self.show_filter_input; if self.show_filter_input { self.filter_input.clear(); } }
                ui.add_space(12.0); ui.separator();
                ui.label("Hotkeys: D-Debug F-Clear T-Theme /-Filter");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let c = if self.debug_mode { Color32::YELLOW } else { Color32::GREEN };
                ui.label(egui::RichText::new(format!("Events: {} | Status: {} | Theme: {}", evt_count, self.status_msg, if dark { "Dark" } else { "Light" }))
                    .size(STATUS_FONT_SIZE).color(c));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(egui::RichText::new("FlowLens v0.1.0").size(STATUS_FONT_SIZE - 4.0));
                });
            });

            if self.show_filter_input {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Filter keyword:").size(STATUS_FONT_SIZE));
                    ui.text_edit_singleline(&mut self.filter_input);
                    if ui.button("Apply").clicked() { self.show_filter_input = false; log_ui("Filter applied"); }
                    if ui.button("Cancel").clicked() { self.show_filter_input = false; self.filter_input.clear(); }
                });
                ui.separator();
            }

            if evt_count == 0 {
                ui.allocate_space(egui::vec2(ui.available_width(), 200.0));
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("No events - Click 'Start Capture'").size(STATUS_FONT_SIZE));
                });
            } else {
                ui.horizontal(|ui| {
                    for h in ["Time", "Type", "Source", "Destination", "Size", "Method/Path"] {
                        ui.label(egui::RichText::new(h).size(CELL_FONT_SIZE).strong());
                    }
                });
                ui.separator();
                let start = evt_count.saturating_sub(500);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for row in &self.events.lock().unwrap()[start..] {
                        let c = event_color(&row.event_type, dark);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format_ts(row.timestamp)).color(c).size(CELL_FONT_SIZE));
                            ui.label(egui::RichText::new(event_type_cn(&row.event_type)).color(c).size(CELL_FONT_SIZE));
                            ui.label(egui::RichText::new(row.src_addr.to_string()).size(CELL_FONT_SIZE - 2.0));
                            ui.label(egui::RichText::new(row.dst_addr.to_string()).size(CELL_FONT_SIZE - 2.0));
                            ui.label(egui::RichText::new(fmt_bytes(row.body_size)).size(CELL_FONT_SIZE));
                            ui.label(egui::RichText::new(format!("{} {}", trunc(&row.method, 10), trunc(&row.path, 20))).size(CELL_FONT_SIZE - 2.0));
                        });
                    }
                });
            }
        });

        egui::TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let c = if self.debug_mode { Color32::YELLOW } else { Color32::GREEN };
                ui.label(egui::RichText::new(&self.status_msg).size(STATUS_FONT_SIZE - 2.0).color(c));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(egui::RichText::new(if dark { "Dark Theme" } else { "Light Theme" }).size(STATUS_FONT_SIZE - 4.0));
                });
            });
        });

        if self.debug_mode {
            egui::SidePanel::right("debug_panel").show(ctx, |ui| {
                ui.strong(egui::RichText::new("Event Details").size(CELL_FONT_SIZE));
                ui.separator();
                let events = self.events.lock().unwrap();
                if events.is_empty() { ui.label("No events to display"); }
                else {
                    for (i, row) in events.iter().rev().take(80).enumerate() {
                        ui.collapsing(format!("#{} {} {}", i+1, event_type_cn(&row.event_type), format_ts(row.timestamp)), |ui| {
                            ui.label(format!("Source: {}", row.src_addr));
                            ui.label(format!("Destination: {}", row.dst_addr));
                            ui.label(format!("Method: {}", row.method));
                            ui.label(format!("Path: {}", row.path));
                            ui.label(format!("Status: {}", row.status));
                            ui.label(format!("Size: {} bytes", row.body_size));
                        });
                    }
                }
            });
        }

        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::ALT, egui::Key::D) { self.toggle_debug(); }
            if i.consume_key(egui::Modifiers::ALT, egui::Key::F) { self.clear_events(); }
            if i.consume_key(egui::Modifiers::ALT, egui::Key::T) { self.dark_mode = !self.dark_mode; log_ui("Toggle theme"); }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::F) { self.clear_events(); }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::D) { self.toggle_debug(); }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::T) { self.dark_mode = !self.dark_mode; log_ui("Toggle theme"); }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Slash) { self.show_filter_input = !self.show_filter_input; if self.show_filter_input { self.filter_input.clear(); } }
        });
    }
}

use crate::UiFrontend;
impl UiFrontend for GuiFrontend {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        let app = Self {
            events: self.events.clone(),
            running: self.running.clone(),
            debug_mode: self.debug_mode,
            dark_mode: self.dark_mode,
            filter_input: self.filter_input.clone(),
            show_filter_input: self.show_filter_input,
            status_msg: self.status_msg.clone(),
            selected_row: self.selected_row,
            bus: Some(bus),
        };
        eframe::run_native(
            "FlowLens - Network Traffic Analyzer",
            eframe::NativeOptions::default(),
            Box::new(|_cc| Ok(Box::new(app))),
        ).map_err(|e| anyhow::anyhow!(e.to_string()))
    }
    fn stop(&self) -> anyhow::Result<()> {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_gui_creation() { let _gui = GuiFrontend::new(); }
    #[test] fn test_event_type_cn() { assert_eq!(event_type_cn("connection"), "连接"); }
    #[test] fn test_format_ts() { assert_eq!(format_ts(0), "00:00:00"); }
    #[test] fn test_fmt_bytes() { assert_eq!(fmt_bytes(0), "0 B"); assert_eq!(fmt_bytes(1024), "1.00 KB"); }
    #[test] fn test_trunc() { assert_eq!(trunc("hello", 10), "hello"); assert_eq!(trunc("hello world", 5), "hello…"); }
}
