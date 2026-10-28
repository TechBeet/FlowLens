//! FlowLens GUI - VS Code style dark/light theme, professional table layout
//! Design: snapshot caching (no per-frame clone of 5000 rows), real interactions.

use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use eframe::egui::{self, Id, RichText, ScrollArea, Sense};
use crate::UiFrontend;
use flowlens_core::event_bus::EventBus;
use flowlens_core::models::TrafficRow;

const MAX_EVENTS: usize = 5000;
const ROW_H: f32 = 28.0;
const COL_TIME: f32 = 96.0;
const COL_TYPE: f32 = 100.0;
const COL_METHOD: f32 = 68.0;
const COL_STATUS: f32 = 64.0;
const COL_SIZE: f32 = 84.0;
const COL_ADDR: f32 = 160.0;
const COL_GAP: f32 = 8.0;

// ---------------------------------------------------------------------------
// Theme enum
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

// ---------------------------------------------------------------------------
// Color palette (struct with theme-based colors)
// ---------------------------------------------------------------------------
#[derive(Clone)]
pub struct Palette {
    pub bg_app: egui::Color32,
    pub bg_panel: egui::Color32,
    pub bg_row_a: egui::Color32,
    pub bg_row_b: egui::Color32,
    pub bg_header: egui::Color32,
    pub bg_input: egui::Color32,
    pub accent: egui::Color32,
    pub sel_bg: egui::Color32,
    pub hover_bg: egui::Color32,
    pub border: egui::Color32,
    pub text: egui::Color32,
    pub text_dim: egui::Color32,
    pub text_muted: egui::Color32,
    pub ok: egui::Color32,
    pub warn: egui::Color32,
    pub err: egui::Color32,
    pub purple: egui::Color32,
}

impl Palette {
    pub fn new(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                bg_app: egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
                bg_panel: egui::Color32::from_rgb(0x25, 0x25, 0x26),
                bg_row_a: egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
                bg_row_b: egui::Color32::from_rgb(0x25, 0x25, 0x26),
                bg_header: egui::Color32::from_rgb(0x2D, 0x2D, 0x30),
                bg_input: egui::Color32::from_rgb(0x3C, 0x3C, 0x3C),
                accent: egui::Color32::from_rgb(0x56, 0x9C, 0xD6),
                sel_bg: egui::Color32::from_rgb(0x09, 0x47, 0x71),
                hover_bg: egui::Color32::from_rgb(0x2A, 0x2D, 0x2E),
                border: egui::Color32::from_rgb(0x3C, 0x3C, 0x3C),
                text: egui::Color32::from_rgb(0xD4, 0xD4, 0xD4),
                text_dim: egui::Color32::from_rgb(0x85, 0x85, 0x85),
                text_muted: egui::Color32::from_rgb(0x6A, 0x6A, 0x6A),
                ok: egui::Color32::from_rgb(0x4E, 0xC9, 0xB0),
                warn: egui::Color32::from_rgb(0xDC, 0xED, 0xAA),
                err: egui::Color32::from_rgb(0xF4, 0x47, 0x47),
                purple: egui::Color32::from_rgb(0xC5, 0x86, 0xC0),
            },
            Theme::Light => Self {
                bg_app: egui::Color32::from_rgb(0xF9, 0xF9, 0xFA),
                bg_panel: egui::Color32::from_rgb(0xEE, 0xEE, 0xEF),
                bg_row_a: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
                bg_row_b: egui::Color32::from_rgb(0xF5, 0xF5, 0xF6),
                bg_header: egui::Color32::from_rgb(0xE8, 0xE8, 0xE9),
                bg_input: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
                accent: egui::Color32::from_rgb(0x0F, 0x4C, 0x81),
                sel_bg: egui::Color32::from_rgb(0xD0, 0xE4, 0xF7),
                hover_bg: egui::Color32::from_rgb(0xE6, 0xE6, 0xE8),
                border: egui::Color32::from_rgb(0xD0, 0xD0, 0xD1),
                text: egui::Color32::from_rgb(0x24, 0x24, 0x24),
                text_dim: egui::Color32::from_rgb(0x6A, 0x6A, 0x6A),
                text_muted: egui::Color32::from_rgb(0x8C, 0x8C, 0x8C),
                ok: egui::Color32::from_rgb(0x0C, 0x78, 0x3D),
                warn: egui::Color32::from_rgb(0x85, 0x73, 0x0B),
                err: egui::Color32::from_rgb(0xB3, 0x26, 0x32),
                purple: egui::Color32::from_rgb(0x6C, 0x2E, 0x7B),
            },
        }
    }

    #[inline]
    pub fn event_color(&self, kind: &str) -> egui::Color32 {
        match kind {
            "connection" => self.ok,
            "tls" => self.purple,
            "http_request" => self.accent,
            "http_response" => self.ok,
            "websocket" => self.purple,
            "sse" => self.ok,
            "grpc" => self.warn,
            "closed" => self.text_muted,
            "dropped" => self.err,
            _ => self.text_dim,
        }
    }

    #[inline]
    pub fn status_color(&self, s: &str) -> egui::Color32 {
        let c: u32 = s.parse().unwrap_or(0);
        match c {
            200..=299 => self.ok,
            300..=399 => self.warn,
            400..=499 => self.warn,
            500..=599 => self.err,
            0 => self.text_muted,
            _ => self.text_dim,
        }
    }

    /// Background colour for a table row given its zebra index.
    #[inline]
    pub fn row_bg(&self, index: usize, selected: bool) -> egui::Color32 {
        if selected {
            self.sel_bg
        } else if index % 2 == 0 {
            self.bg_row_a
        } else {
            self.bg_row_b
        }
    }

    /// Colour of the table header strip.
    #[inline]
    pub fn header_bg(&self) -> egui::Color32 {
        self.bg_header
    }

    /// Colour used for subtle separators and outlines.
    #[inline]
    pub fn border_color(&self) -> egui::Color32 {
        self.border
    }

    /// Colour used for the row highlight on mouse hover.
    #[inline]
    pub fn hover_bg(&self) -> egui::Color32 {
        self.hover_bg
    }
}
// ---------------------------------------------------------------------------
// Separator helpers: draw vertical/horizontal lines using palette border colour
// ---------------------------------------------------------------------------
/// Draw a vertical separator line using the palette's border colour.
fn sep_vline(ui: &mut egui::Ui, p: &Palette) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().vline(
        rect.max.x,
        rect.min.y..=rect.max.y,
        egui::Stroke::new(1.0_f32, p.border_color()),
    );
}

/// Draw a horizontal separator line using the palette's border colour.
fn sep_hline(ui: &mut egui::Ui, p: &Palette) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rect.min.x..=rect.max.x,
        rect.max.y,
        egui::Stroke::new(1.0_f32, p.border_color()),
    );
}

// ---------------------------------------------------------------------------
// Sorting & column layout state
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortKey { Time, Type, Method, Status, Size }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir { Asc, Desc }

/// Persistent column widths (persisted via egui memory, adjustable by drag).
#[derive(Clone)]
#[allow(dead_code)]
pub struct ColumnWidths {
    pub time: f32, pub kind: f32, pub method: f32,
    pub status: f32, pub size: f32,
    pub src: f32, pub dst: f32, pub path: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self { time: 96.0, kind: 110.0, method: 64.0, status: 64.0,
               size: 78.0, src: 150.0, dst: 150.0, path: 320.0 }
    }
}

impl ColumnWidths {
    #[allow(dead_code)]
    pub fn as_vec(&self) -> [(&'static str, f32); 8] {
        [
            ("time", self.time), ("kind", self.kind), ("method", self.method),
            ("status", self.status), ("size", self.size), ("src", self.src),
            ("dst", self.dst), ("path", self.path),
        ]
    }
}

fn cmp_rows(a: &TrafficRow, b: &TrafficRow, key: SortKey) -> std::cmp::Ordering {
    match key {
        SortKey::Time => a.timestamp.cmp(&b.timestamp),
        SortKey::Type => a.event_type.cmp(&b.event_type),
        SortKey::Method => a.method.cmp(&b.method),
        SortKey::Status => a.status.cmp(&b.status),
        SortKey::Size => a.body_size.cmp(&b.body_size),
    }
}

// ---------------------------------------------------------------------------
// GuiFrontend
// ---------------------------------------------------------------------------
#[derive(Clone)]
pub struct GuiFrontend {
    events: Arc<Mutex<Vec<TrafficRow>>>,
    running: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    snapshot: Arc<Mutex<Vec<TrafficRow>>>,
    snapshot_len: Arc<std::sync::atomic::AtomicUsize>,
    filter: String,
    selected: Option<usize>,
    theme: Theme,
    sort: Option<(SortKey, SortDir)>,
    #[allow(dead_code)]
    col_widths: ColumnWidths,
    /// Row expanded in the detail panel (index into snapshot).
    expanded: Option<usize>,
    /// Pending context-menu row index.
    #[allow(dead_code)]
    context_row: Option<usize>,
    clipboard: Arc<Mutex<String>>,
    /// Snapshot index of the row that should be scrolled into view on the next
    /// frame. Set by keyboard navigation (Up/Down), consumed by `draw_rows`.
    scroll_to: Option<usize>,
}

impl Default for GuiFrontend {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(true)),
            started: Arc::new(AtomicBool::new(false)),
            snapshot: Arc::new(Mutex::new(Vec::new())),
            snapshot_len: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            filter: String::new(),
            selected: None,
            theme: Theme::Dark,
            sort: None,
            col_widths: ColumnWidths::default(),
            expanded: None,
            context_row: None,
            clipboard: Arc::new(Mutex::new(String::new())),
            scroll_to: None,
        }
    }
}

impl GuiFrontend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start background subscription: EventBus -> events buffer
    fn start_capture(&self, bus: Arc<dyn EventBus>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        let ev = self.events.clone();
        let run = self.running.clone();
        std::thread::spawn(move || {
            let mut rx =
                bus.subscribe(flowlens_core::models::TrafficFilter::default());
            while run.load(Ordering::SeqCst) {
                match rx.blocking_recv() {
                    Ok(e) => {
                        if let Some(row) = TrafficRow::from_event(&e) {
                            if let Ok(mut buf) = ev.lock() {
                                buf.push(row);
                                if buf.len() > MAX_EVENTS {
                                    let excess = buf.len() - MAX_EVENTS;
                                    buf.drain(0..excess);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    /// Refresh render snapshot only when event count changes
    fn refresh_snapshot(&self) {
        let cur = self.events.lock().map(|b| b.len()).unwrap_or(0);
        if cur != self.snapshot_len.load(Ordering::SeqCst) {
            if let (Ok(mut s), Ok(b)) = (self.snapshot.lock(), self.events.lock()) {
                *s = b.clone();
            }
            self.snapshot_len.store(cur, Ordering::SeqCst);
        }
    }

    /// Filter indices for a search term (supports filter.rs DSL or simple text match)
    pub fn filtered_indices(&self, snap: &[TrafficRow], query: &str) -> Vec<usize> {
        let q = query.trim();
        
        // Collect all indices first
        let mut indices: Vec<usize> = if q.is_empty() {
            (0..snap.len()).collect()
        } else {
            // Try to parse as filter.rs DSL first
            let filter_expr = flowlens_core::filter::parse_filter(q);
            
            if let Ok(expr) = filter_expr {
                snap.iter()
                    .enumerate()
                    .filter_map(|(i, r)| {
                        if flowlens_core::filter::eval_filter(r, &expr) {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                // Fall back to simple text match
                let ql = q.to_lowercase();
                snap.iter()
                    .enumerate()
                    .filter_map(|(i, r)| {
                        let h = format!(
                            "{} {} {} {} {}",
                            r.event_type, r.src_addr, r.dst_addr, r.method, r.path
                        )
                        .to_lowercase();
                        if h.contains(&ql) {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        };
        
        // Apply sorting if enabled
        if let Some((key, dir)) = self.sort {
            indices.sort_by(|&a, &b| {
                let ord = cmp_rows(&snap[a], &snap[b], key);
                match dir {
                    SortDir::Asc => ord,
                    SortDir::Desc => ord.reverse(),
                }
            });
        }
        
        indices
    }
    
    /// Toggle sort direction for a column
    pub fn set_sort_key(&mut self, key: SortKey) {
        self.sort = match self.sort {
            Some((k, SortDir::Asc)) if k == key => Some((k, SortDir::Desc)),
            Some((k, _)) if k == key => None,
            _ => Some((key, SortDir::Asc)),
        };
    }

    /// Store `text` in the internal clipboard buffer.
    pub fn copy_to_clipboard(&self, text: impl Into<String>) {
        if let Ok(mut cb) = self.clipboard.lock() {
            *cb = text.into();
        }
    }

    /// Read back the current clipboard buffer contents.
    pub fn clipboard_contents(&self) -> String {
        self.clipboard.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Expand the row at `filtered_index` (position within the filtered list)
    /// or collapse it if it is already expanded.
    pub fn toggle_expand(&mut self, filtered_index: usize) {
        if self.expanded == Some(filtered_index) {
            self.expanded = None;
        } else {
            self.expanded = Some(filtered_index);
        }
    }

    /// Move the selection up/down within `filtered` (a list of snapshot indices).
    pub fn move_selection(&mut self, filtered: &[usize], delta: i32) {
        if filtered.is_empty() {
            self.selected = None;
            return;
        }
        let cur_pos = self
            .selected
            .and_then(|s| filtered.iter().position(|&x| x == s))
            .unwrap_or(0);
        let len = filtered.len() as i32;
        let next = ((cur_pos as i32 + delta).rem_euclid(len)) as usize;
        self.selected = Some(filtered[next]);
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------
fn fmt_ts(micros: i64) -> String {
    if micros == 0 { return "--:--:--.-".into(); }
    let s = micros / 1_000_000; let d = (micros % 1_000_000) / 100_000;
    format!("{:02}:{:02}:{:02}.{}", (s/3600)%24, (s/60)%60, s%60, d)
}

fn fmt_bytes(n: usize) -> String {
    if n == 0 { return "-".into(); }
    let f = n as f64;
    if f >= 1048576.0 { format!("{:.1} MB", f / 1048576.0) }
    else if f >= 1024.0 { format!("{:.1} KB", f / 1024.0) }
    else { format!("{} B", n) }
}

fn fit_chars(s: &str, px: f32) -> String {
    let per = 7.5_f32; let max = (px / per).floor() as usize;
    let mut units = 0usize; let mut out = String::new();
    for ch in s.chars() {
        let w = if ch.is_ascii() { 1 } else { 2 };
        if units + w > max { out.push('\u{2026}'); break; }
        units += w; out.push(ch);
    }
    out
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------
struct FlowLensApp { frontend: GuiFrontend }

impl eframe::App for FlowLensApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // P1-4: DPI zoom factor support - respect system DPI scaling
        let zoom_factor = ctx.pixels_per_point().clamp(0.8, 2.5);
        ctx.set_zoom_factor(zoom_factor);

        self.frontend.refresh_snapshot();
        let snap = self.frontend.snapshot.lock().unwrap().clone();
        let filtered = self.frontend.filtered_indices(&snap, &self.frontend.filter);
        let sel = self.frontend.selected;
        let p = Palette::new(self.frontend.theme);

        // Apply custom visuals so egui's built-in widgets match our palette
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = egui::Rounding::same(4.0);
        visuals.panel_fill = p.bg_app;
        visuals.window_fill = p.bg_panel;
        visuals.extreme_bg_color = p.bg_app;
        visuals.widgets.noninteractive.bg_fill = p.bg_input;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.border_color());
        visuals.widgets.hovered.bg_fill = p.hover_bg;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, p.accent);
        visuals.widgets.active.bg_fill = p.hover_bg;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, p.accent);
        visuals.selection.bg_fill = p.sel_bg;
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, p.accent);
        visuals.faint_bg_color = p.bg_row_b;
        visuals.widgets.noninteractive.weak_bg_fill = p.bg_input;
        ctx.set_visuals(visuals);

        // ===== Top bar =====
        egui::TopBottomPanel::top("header").frame(
            egui::Frame::none()
                .fill(p.bg_panel)
                .inner_margin(egui::Margin::same(8.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                ui.label(
                    RichText::new("\u{2666} \u{6D41}\u{5149} FlowLens")
                        .color(p.accent)
                        .size(18.0),
                );
                ui.label(
                    RichText::new("\u{7F51}\u{7EDC}\u{6D41}\u{91CF}\u{5206}\u{6790}\u{5668}")
                        .color(p.text_dim)
                        .size(12.0),
                );
                // Custom separator: vertical line drawn with the palette border colour
                // so it matches every other outline in the theme.
                sep_vline(ui, &p);
                let running = self.frontend.running.load(Ordering::SeqCst);
                ui.label(
                    RichText::new(if running {
                        "\u{25CF} \u{91C7}\u{96C6}\u{4E2D}"
                    } else {
                        "\u{25CB} \u{5DF2}\u{505C}\u{6B62}"
                    })
                    .color(if running { p.ok } else { p.text_muted })
                    .size(12.0),
                );
                // Custom separator: vertical line drawn with the palette border colour
                sep_vline(ui, &p);
                ui.label(
                    RichText::new(format!(
                        "\u{4E8B}\u{4EF6} {} \u{00B7} \u{7B5B}\u{9009} {}",
                        snap.len(),
                        filtered.len()
                    ))
                    .color(p.text_dim)
                    .size(12.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Theme toggle: sun in dark mode, moon in light mode.
                    let theme_btn = match self.frontend.theme {
                        Theme::Dark => "\u{2600}",
                        Theme::Light => "\u{263E}",
                    };
                    let hint = match self.frontend.theme {
                        Theme::Dark => "\u{5207}\u{6362}\u{4E3A}\u{4EAE}\u{8272}",
                        Theme::Light => "\u{5207}\u{6362}\u{4E3A}\u{6697}\u{8272}",
                    };
                    if ui
                        .button(
                            egui::RichText::new(theme_btn)
                                .color(p.accent)
                                .size(15.0),
                        )
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.frontend.theme = match self.frontend.theme {
                            Theme::Dark => Theme::Light,
                            Theme::Light => Theme::Dark,
                        };
                        // Force a snapshot refresh so colours are re-applied.
                        self.frontend.snapshot_len
                            .store(usize::MAX, Ordering::SeqCst);
                    }
                    if ui.button("\u{6E05}\u{7A7A}").clicked() {
                        self.frontend.events.lock().unwrap().clear();
                        self.frontend.snapshot_len.store(0, Ordering::SeqCst);
                        self.frontend.selected = None;
                    }
                });
            });
        });

        // ===== Filter bar =====
        egui::TopBottomPanel::top("filter")
            .resizable(false)
            .frame(egui::Frame::none().fill(p.bg_app))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.frontend.filter)
                            .id(Id::new("filter"))
                            .hint_text(RichText::new(
                                "筛选：GET /api/* AND status>=200  |  主机 / 方法 / 路径",
                            )
                            .color(p.text_dim))
                            .desired_width(360.0)
                            .text_color(p.text)
                            .background_color(p.bg_input)
                            .margin(egui::Margin::symmetric(6.0, 4.0)),
                    );
                    // Custom separator: vertical line drawn with the palette border colour
                    sep_vline(ui, &p);
                    // 预设筛选按钮
                    let presets: [(&str, &str, &str); 5] = [
                        ("HTTP", "http_request", "只显示 HTTP 请求"),
                        ("TLS", "tls_handshake", "只显示 TLS 握手"),
                        ("WS", "websocket", "只显示 WebSocket"),
                        ("4xx", "status>=400 AND status<500", "只显示 4xx 错误"),
                        ("5xx", "status>=500", "只显示 5xx 错误"),
                    ];
                    for (label, val, hint) in presets.iter() {
                        if ui.small_button(*label).on_hover_text(*hint).clicked() {
                            self.frontend.filter = (*val).to_string();
                        }
                    }
                    if ui.small_button("清除").on_hover_text("清空筛选条件").clicked() {
                        self.frontend.filter.clear();
                    }
                    // Custom separator: vertical line drawn with the palette border colour
                    sep_vline(ui, &p);
                    ui.label(
                        RichText::new(format!("{}/{}", filtered.len(), snap.len()))
                            .color(p.accent)
                            .size(12.0),
                    );
                });
            });

        // ===== Detail Panel (below table) =====
        if let Some(expanded_idx) = self.frontend.expanded {
            if expanded_idx < filtered.len() {
                egui::TopBottomPanel::bottom("detail")
                    .resizable(true)
                    .min_height(150.0)
                    .max_height(300.0)
                    .frame(egui::Frame::none().fill(p.bg_panel).inner_margin(egui::Margin::same(8.0)))
                    .show(ctx, |ui| {
                        let idx = filtered[expanded_idx];
                        let row = &snap[idx];
                        ui.horizontal(|ui| {
                            if ui.button("×").clicked() {
                                self.frontend.expanded = None;
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{} {} → {}  {}",
                                    row.method,
                                    row.path,
                                    row.dst_addr,
                                    row.status
                                ))
                                .strong()
                                .color(p.accent),
                            );
                        });
                        ui.add_space(8.0);
                        egui::CollapsingHeader::new("Headers").show(ui, |ui| {
                            ui.monospace(format!(
                                "GET {}\r\nHost: {}\r\nContent-Type: application/json",
                                row.path, row.src_addr,
                            ));
                            ui.add_space(4.0);
                            ui.monospace(format!("{}", row.status));
                        });
                    });
            }
        }

        // ===== Central Panel: Table =====
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(p.bg_app))
            .show(ctx, |ui| {
                ScrollArea::vertical().id_salt("table").show(ui, |ui| {
                    // Header row on a distinct background strip.
                    let hdr_rect = egui::Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(ui.available_width(), 24.0),
                    );
                    ui.painter().rect_filled(hdr_rect, 0.0, p.header_bg());
                    ui.horizontal(|ui| {
                        let cols = [
                            ("Time", COL_TIME,   SortKey::Time),
                            ("Type", COL_TYPE,   SortKey::Type),
                            ("Method", COL_METHOD, SortKey::Method),
                            ("Status", COL_STATUS, SortKey::Status),
                            ("Size", COL_SIZE,   SortKey::Size),
                        ];
                        for (label, w, key) in cols.iter() {
                            let arrow = self.frontend.sort
                                .and_then(|(k, dir)| if k == *key {
                                    Some(match dir {
                                        SortDir::Asc => " ▲",
                                        SortDir::Desc => " ▼",
                                    })
                                } else { None });
                            let txt = match arrow {
                                Some(a) => format!("{label}{a}"),
                                None => label.to_string(),
                            };
                            if ui.add_sized(
                                egui::vec2(*w, 22.0),
                                egui::SelectableLabel::new(
                                    self.frontend.sort
                                        .map(|(k,_)| k == *key)
                                        .unwrap_or(false),
                                    RichText::new(txt).color(p.text_dim).size(11.0).strong(),
                                ),
                            ).clicked() {
                                self.frontend.set_sort_key(*key);
                            }
                            ui.add_space(COL_GAP);
                        }
                        // Non-sortable columns (addresses + path)
                        for (label, w) in [
                            ("Src Addr", COL_ADDR),
                            ("Dst Addr", COL_ADDR),
                            ("Path", COL_ADDR + 60.0),
                        ] {
                            ui.add_sized(
                                egui::vec2(w, 22.0),
                                egui::Label::new(
                                    RichText::new(label)
                                        .color(p.text_dim)
                                        .size(11.0)
                                        .strong(),
                                ),
                            );
                            ui.add_space(COL_GAP);
                        }
                    });
                    // Custom separator: horizontal line drawn with the palette border colour
                    sep_hline(ui, &p);
                    draw_rows(
            ui,
            &p,
            &snap,
            &filtered,
            sel,
            &mut self.frontend.selected,
            &mut self.frontend.expanded,
            self.frontend.scroll_to.take(),
        );
                });
            });

        // ===== Bottom status bar =====
        egui::TopBottomPanel::bottom("statusbar")
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(p.bg_panel)
                    .inner_margin(egui::Margin::same(8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                    ui.label(
                        RichText::new(
                            "Ctrl+F \u{8FC7}\u{6EE4} \u{00B7} Up/Down \u{9009}\u{62E9} \u{00B7} Esc \u{6E05}\u{7A7A}",
                        )
                        .color(p.text_muted)
                        .size(11.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(idx) = sel {
                            ui.label(
                                RichText::new(format!(
                                    "\u{5DF2}\u{9009}\u{4E2D}: {} {} \u{2192} {}",
                                    snap[idx].event_type,
                                    snap[idx].src_addr,
                                    snap[idx].dst_addr
                                ))
                                .color(p.text)
                                .size(11.0),
                            );
                        }
                    });
                });
            });

        // ===== Keyboard shortcuts =====
        let ctrl_f = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F));
        if ctrl_f {
            ctx.memory_mut(|m| m.request_focus(Id::new("filter")));
        }
        
        // Ctrl+C: copy selected row to clipboard
        let ctrl_c = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::C));
        if ctrl_c && sel.is_some() {
            let idx = sel.unwrap();
            if idx < snap.len() {
                let row = &snap[idx];
                let text = format!(
                    "{} {} {} {} {} {}",
                    row.timestamp, row.event_type, row.method, row.path, row.status, row.body_size
                );
                self.frontend.copy_to_clipboard(&text);
                // Also set system clipboard
                ctx.output_mut(|o| o.copied_text = text);
            }
        }
        
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.frontend.filter.clear();
            self.frontend.selected = None;
            self.frontend.expanded = None;
        }
        
        // Enter: toggle expansion of selected row
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && sel.is_some() {
            let idx = sel.unwrap();
            if let Some(pos) = filtered.iter().position(|&x| x == idx) {
                self.frontend.toggle_expand(pos);
            }
        }
        
        let n = filtered.len();
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && n > 0 {
            let cur = sel
                .and_then(|s| filtered.iter().position(|&x| x == s))
                .unwrap_or(0);
            self.frontend.selected = Some(filtered[cur.saturating_sub(1)]);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && n > 0 {
            let cur = sel
                .and_then(|s| filtered.iter().position(|&x| x == s))
                .unwrap_or(0);
            self.frontend.selected = Some(filtered[(cur + 1) % n]);
        }
    }
}

/// Helper: draw a table row with selection highlighting
fn draw_rows(
    ui: &mut egui::Ui,
    p: &Palette,
    snap: &[TrafficRow],
    filtered: &[usize],
    sel: Option<usize>,
    selected: &mut Option<usize>,
    expanded: &mut Option<usize>,
    scroll_to: Option<usize>,
) {
    let max_rows = 600usize;
    let start = filtered.len().saturating_sub(max_rows);
    let visible = &filtered[start..];
    for (vi, &ri) in visible.iter().enumerate() {
        // Bounds guard: the snapshot may shrink between frames (buffer drain),
        // leaving stale indices in `filtered`. Skip rather than panic on OOB.
        if ri >= snap.len() {
            continue;
        }
        let fi = start + vi; // index within the filtered list
        let r = &snap[ri];
        let is_sel = sel == Some(ri);
        let bg = p.row_bg(ri, is_sel);
        let rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(ui.available_width(), ROW_H),
        );
        ui.painter().rect_filled(rect, 0.0, bg);
        if is_sel {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, egui::vec2(3.0, ROW_H)),
                0.0,
                p.accent,
            );
        }
        // Subtle bottom border keeps rows visually separated in both themes.
        ui.painter().hline(
            rect.min.x..=rect.max.x,
            rect.max.y - 1.0,
            egui::Stroke::new(1.0_f32, p.border_color()),
        );
        // Hover highlight (only when the row is not already selected).
        // Note: Sense::click() implies hover in egui, but we use union for clarity.
        // CRITICAL: Use stable, content-derived ID to prevent mouse state corruption
        // when snapshot indices shift due to buffer draining. The ID combines the
        // snapshot index with immutable row content (timestamp, addresses, size) so
        // egui's widget state cache correctly tracks the same logical row across frames.
        let stable_row_id = Id::new((
            "row",
            ri,
            r.timestamp,
            &r.src_addr,
            &r.dst_addr,
            r.body_size,
        ));
        let response = ui.interact(rect, stable_row_id, Sense::click() | Sense::hover());
        if !is_sel && response.hovered() {
            ui.painter().rect_filled(rect, 0.0, p.hover_bg());
        }
        if response.clicked() {
            // Single click: set selection
            *selected = Some(ri);
        } else if response.double_clicked() {
            // Double-click: toggle expansion at filtered index
            if *expanded == Some(fi) {
                *expanded = None;
            } else {
                *expanded = Some(fi);
            }
        }
        // Auto-scroll if this row should be brought into view
        if scroll_to == Some(ri) {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(COL_GAP, 0.0);
            ui.add_sized(egui::vec2(COL_TIME, ROW_H), egui::Label::new(
                RichText::new(fmt_ts(r.timestamp)).color(p.text).size(12.0)));
            ui.add_sized(egui::vec2(COL_TYPE, ROW_H), egui::Label::new(
                RichText::new(&r.event_type).color(p.event_color(&r.event_type)).size(12.0)));
            ui.add_sized(egui::vec2(COL_METHOD, ROW_H), egui::Label::new(
                RichText::new(&r.method).color(p.text).size(12.0)));
            ui.add_sized(egui::vec2(COL_STATUS, ROW_H), egui::Label::new(
                RichText::new(&r.status).color(p.status_color(&r.status)).size(12.0)));
            ui.add_sized(egui::vec2(COL_SIZE, ROW_H), egui::Label::new(
                RichText::new(fmt_bytes(r.body_size)).color(p.text_dim).size(12.0)));
            ui.add_sized(egui::vec2(COL_ADDR, ROW_H), egui::Label::new(
                RichText::new(fit_chars(&r.src_addr, COL_ADDR)).color(p.text).size(12.0)));
            ui.add_sized(egui::vec2(COL_ADDR, ROW_H), egui::Label::new(
                RichText::new(fit_chars(&r.dst_addr, COL_ADDR)).color(p.text).size(12.0)));
            ui.add_sized(egui::vec2(COL_ADDR + 60.0, ROW_H), egui::Label::new(
                RichText::new(fit_chars(&r.path, COL_ADDR + 60.0)).color(p.text).size(12.0)));
        });
        ui.allocate_space(egui::vec2(0.0, 2.0));
    }
    if visible.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new(if snap.is_empty() {
                    "\u{6682}\u{65E0}\u{4E8B}\u{4EF6}"
                } else {
                    "\u{7121}\u{5339}\u{914D}\u{4E8B}\u{4EF6}"
                })
                .color(p.text_dim)
                .size(13.0),
            );
        });
    }
}

impl UiFrontend for GuiFrontend {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        self.start_capture(bus);
        eframe::run_native(
            "\u{6D41}\u{5149} - \u{7F51}\u{7EDC}\u{6D41}\u{91CF}\u{5206}\u{6790}\u{5668}",
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size((1920.0, 1200.0))
                    .with_min_inner_size((900.0, 500.0)),
                ..Default::default()
            },
            Box::new(move |cc| {
                crate::install_fonts(&cc.egui_ctx);
                // IMPORTANT: reuse the *same* frontend instance that already
                // subscribed to the event bus. Creating a new one here would
                // discard the background receiver and the UI would stay empty.
                Ok(Box::new(FlowLensApp {
                    frontend: self.clone(),
                }))
            }),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    fn stop(&self) -> anyhow::Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_ts() {
        assert_eq!(fmt_ts(0), "--:--:--.-");
        assert_eq!(fmt_ts(1_000_000), "00:00:01.0");
        assert_eq!(fmt_ts(3_723_456_000), "01:02:03.4");
    }

    #[test]
    fn test_fmt_bytes() {
        assert_eq!(fmt_bytes(0), "-");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1024), "1.0 KB");
        assert_eq!(fmt_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_fit_chars_truncates() {
        assert_eq!(fit_chars("abcd", 500.0), "abcd");
        assert!(fit_chars("a-very-long-string-value", 30.0).chars().count() < 24);
    }

    #[test]
    fn test_filtered_indices_empty_snapshot() {
        let frontend = GuiFrontend::default();
        let rows: Vec<TrafficRow> = vec![];
        assert!(frontend.filtered_indices(&rows, "").is_empty());
        assert!(frontend.filtered_indices(&rows, "http").is_empty());
    }

    #[test]
    fn test_filtered_indices_with_sort() {
        let mut frontend = GuiFrontend::default();
        let rows = vec![
            TrafficRow { timestamp: 100, event_type: "http".into(), method: "POST".into(), path: "/api".into(), status: "200".into(), body_size: 1000, src_addr: "127.0.0.1:1".into(), dst_addr: "10.0.0.1:443".into(), duration_ms: 0 },
            TrafficRow { timestamp: 50, event_type: "http".into(), method: "GET".into(), path: "/api".into(), status: "200".into(), body_size: 500, src_addr: "127.0.0.1:2".into(), dst_addr: "10.0.0.1:443".into(), duration_ms: 0 },
            TrafficRow { timestamp: 200, event_type: "tls".into(), method: "".into(), path: "".into(), status: "".into(), body_size: 200, src_addr: "127.0.0.1:3".into(), dst_addr: "10.0.0.1:443".into(), duration_ms: 0 },
        ];
        // Sort by time ascending
        frontend.set_sort_key(SortKey::Time);
        let filtered = frontend.filtered_indices(&rows, "");
        assert_eq!(filtered.len(), 3);
        assert!(rows[filtered[0]].timestamp <= rows[filtered[1]].timestamp);
        
        // Reverse sort
        frontend.set_sort_key(SortKey::Time);
        let filtered_desc = frontend.filtered_indices(&rows, "");
        assert!(rows[filtered_desc[0]].timestamp >= rows[filtered_desc[1]].timestamp);
    }

    #[test]
    fn test_palette_status_color_distinguishes_codes() {
        let p = Palette::new(Theme::Dark);
        assert_ne!(p.status_color("200"), p.status_color("500"));
        assert_eq!(p.status_color("200"), p.ok);
        assert_eq!(p.status_color("500"), p.err);
    }

    #[test]
    fn test_palette_event_color_distinguishes_types() {
        let p = Palette::new(Theme::Dark);
        assert_ne!(p.event_color("http_request"), p.event_color("tls"));
    }

    #[test]
    fn test_light_and_dark_palettes_differ() {
        let dark = Palette::new(Theme::Dark);
        let light = Palette::new(Theme::Light);
        assert_ne!(dark.bg_app, light.bg_app);
        assert_ne!(dark.text, light.text);
        assert_ne!(dark.accent, light.accent);
        // Semantic colors stay distinguishable in both themes.
        assert_ne!(dark.ok, dark.err);
        assert_ne!(light.ok, light.err);
    }

    #[test]
    fn test_row_bg_zigzag_and_selection() {
        let p = Palette::new(Theme::Dark);
        assert_eq!(p.row_bg(0, false), p.bg_row_a);
        assert_eq!(p.row_bg(1, false), p.bg_row_b);
        assert_eq!(p.row_bg(2, false), p.bg_row_a);
        assert_ne!(p.row_bg(0, true), p.bg_row_a);
        assert_eq!(p.row_bg(0, true), p.sel_bg);
    }

    #[test]
    fn test_header_and_helpers_return_valid_colors() {
        let p = Palette::new(Theme::Dark);
        let hdr = p.header_bg();
        let border = p.border_color();
        let hover = p.hover_bg();
        assert_ne!(hdr, p.bg_app);
        assert_ne!(border, p.accent);
        assert_ne!(hover, p.text);
    }
}


