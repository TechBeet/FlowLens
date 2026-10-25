//! FlowLens GUI 图形界面 —— 网络流量分析器
//!
//! 全中文界面，暗色/亮色主题，实时事件表格，支持过滤与行选中。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::App;
use egui::{Color32, RichText, ScrollArea};
use flowlens_core::{
    event_bus::EventBus,
    models::{TrafficFilter, TrafficRow},
};

use crate::UiFrontend;

/// 查找系统中文字体路径
fn find_cjk_font() -> Option<std::path::PathBuf> {
    // 优先使用 Noto Sans SC（开源高质量中文字体）
    let candidates = [
        "C:\\Windows\\Fonts\\NotoSansSC-VF.ttf",   // Noto Sans SC 可变字体
        "C:\\Windows\\Fonts\\msyh.ttc",           // 微软雅黑
        "C:\\Windows\\Fonts\\simsun.ttc",         // 宋体
        "C:\\Windows\\Fonts\\simhei.ttf",         // 黑体
        "C:\\Windows\\Fonts\\NotoSerifSC-VF.ttf", // Noto Serif SC
    ];
    
    for path in &candidates {
        let p = std::path::Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// 初始化 egui 中文字体支持
fn init_cjk_fonts(ctx: &egui::Context) {
    let mut font_def = egui::FontDefinitions::default();
    
    if let Some(font_path) = find_cjk_font() {
        // 读取字体文件
        let font_data = match std::fs::read(&font_path) {
            Ok(bytes) => egui::FontData::from_owned(bytes),
            Err(e) => {
                tracing::warn!("读取中文字体失败 {}: {}", font_path.display(), e);
                return;
            }
        };
        
        // 获取字体文件名用于调试
        let font_name = font_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("CJK");
        
        tracing::info!("加载中文字体: {}", font_path.display());
        
        // 将 CJK 字体插入到字体集合中
        // 使用自定义名称 "CJK"
        font_def.font_data.insert(
            "CJK".to_string(),
            Arc::new(font_data)
        );
        
        // 将 CJK 字体设置为默认字体（替换 Monospace 和 Proportional）
        // egui 0.30 的 FontDefinitions 结构：
        // - fonts: HashMap<String, FontData> 存储字体数据
        // - families: HashMap<FontFamily, Vec<String>> 定义字体族使用的字体列表
        
        // 替换 Monospace 字体的默认字体
        if let Some(monospace_fonts) = font_def.families.get_mut(&egui::FontFamily::Monospace) {
            // 将 CJK 字体插入到列表最前面（优先级最高）
            monospace_fonts.insert(0, "CJK".to_string());
        }
        
        // 替换 Proportional 字体的默认字体  
        if let Some(proportional_fonts) = font_def.families.get_mut(&egui::FontFamily::Proportional) {
            // 将 CJK 字体插入到列表最前面（优先级最高）
            proportional_fonts.insert(0, "CJK".to_string());
        }
        
        ctx.set_fonts(font_def);
        tracing::info!("中文字体初始化成功: {}", font_name);
    } else {
        tracing::warn!("未找到系统中文字体，中文可能无法正确显示");
    }
}

/// 最多保留的事件条数
const MAX_EVENTS: usize = 5000;
/// 超出上限后裁剪到的目标条数
const AUTO_PRUNE_THRESHOLD: usize = 4000;

/// 事件类型英文标识 -> 中文名称
fn event_type_cn(kind: &str) -> &'static str {
    match kind {
        "connection" => "连接",
        "http_request" => "HTTP 请求",
        "http_response" => "HTTP 响应",
        "tls" => "TLS",
        "websocket" => "WebSocket",
        "sse" => "SSE",
        "dns" => "DNS",
        _ => "其他",
    }
}

/// 事件类型对应的强调色
fn event_color(kind: &str, dark: bool) -> Color32 {
    match kind {
        "connection" => Color32::from_rgb(76, 217, 123),
        "http_request" => Color32::from_rgb(88, 166, 255),
        "http_response" => Color32::from_rgb(86, 224, 141),
        "tls" => Color32::from_rgb(247, 178, 63),
        "websocket" => Color32::from_rgb(178, 120, 255),
        "sse" => Color32::from_rgb(45, 226, 216),
        "dns" => Color32::from_rgb(240, 130, 210),
        _ => {
            if dark {
                Color32::from_rgb(170, 176, 186)
            } else {
                Color32::from_rgb(96, 104, 116)
            }
        }
    }
}

/// 毫秒时间戳 -> HH:MM:SS.d
fn format_ts(ts: i64) -> String {
    if ts <= 0 {
        return "--:--:--.-".to_string();
    }
    let total_secs = ts / 1000;
    let tenths = (ts % 1000) / 100;
    let h = (total_secs / 3600) % 24;
    let m = (total_secs / 60) % 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}.{tenths}")
}

/// 字节数 -> 人类可读
fn fmt_bytes(size: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let n = size as f64;
    if n >= MB {
        format!("{:.2} MB", n / MB)
    } else if n >= KB {
        format!("{:.2} KB", n / KB)
    } else {
        format!("{size} B")
    }
}

/// 按字符数截断，超出部分用省略号代替
fn trunc(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let head: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// 状态码着色
fn status_color(status: u16, dark: bool) -> Color32 {
    match status {
        0 => {
            if dark {
                Color32::from_rgb(150, 156, 166)
            } else {
                Color32::from_rgb(120, 126, 136)
            }
        }
        100..=199 => Color32::from_rgb(45, 212, 191),
        200..=299 => Color32::from_rgb(76, 201, 120),
        300..=399 => Color32::from_rgb(238, 205, 90),
        400..=499 => Color32::from_rgb(240, 150, 70),
        _ => Color32::from_rgb(235, 90, 90),
    }
}

// __PART2__

/// GUI 前端状态。被 eframe 单线程调用，不对外共享。
pub struct GuiFrontend {
    events: Arc<Mutex<Vec<TrafficRow>>>,
    running: Arc<AtomicBool>,
    dark_mode: bool,
    filter_input: String,
    selected_row: Option<usize>,
    bus: Option<Arc<dyn EventBus>>,
}

impl Default for GuiFrontend {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            dark_mode: true,
            filter_input: String::new(),
            selected_row: None,
            bus: None,
        }
    }
}

impl GuiFrontend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bus(mut self, bus: Arc<dyn EventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// 在第一次调用时启动后台线程监听事件总线，
    /// 持续将新事件追加到 `events` 缓存中。
    fn start_capture(&mut self) {
        if self.bus.is_none() {
            return;
        }
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        let events = self.events.clone();
        let running = self.running.clone();
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let bus = match bus {
                Some(b) => b,
                None => return,
            };
            let mut rx = bus.subscribe(TrafficFilter::default());
            while running.load(Ordering::SeqCst) {
                match rx.blocking_recv() {
                    Ok(event) => {
                        if let Ok(mut evts) = events.lock() {
                            if let Some(row) = TrafficRow::from_event(&event) {
                                evts.push(row);
                                if evts.len() > MAX_EVENTS {
                                    let excess = evts.len() - AUTO_PRUNE_THRESHOLD;
                                    evts.drain(..excess);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    fn clear_events(&mut self) {
        if let Ok(mut evts) = self.events.lock() {
            evts.clear();
        }
        self.selected_row = None;
    }
}

/// 包装 eframe App trait
struct FlowLensApp {
    frontend: GuiFrontend,
    /// 字体是否已初始化（避免每帧重复加载 17MB 字体文件）
    fonts_initialized: bool,
}

// __PART3__

impl App for FlowLensApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 初始化中文字体（仅首次执行，避免每帧重复读取字体文件造成卡顿）
        if !self.fonts_initialized {
            init_cjk_fonts(ctx);
            self.fonts_initialized = true;
        }

        self.frontend.start_capture();

        // 浅色/深色主题参数
        let dark = self.frontend.dark_mode;
        let pal = if dark {
            ui::Palette::dark()
        } else {
            ui::Palette::light()
        };

        // 字体大小：标题 16，表头 13，正文 12，状态栏 11
        let mut style = (*ctx.style()).clone();
        *style.text_styles.get_mut(&egui::TextStyle::Heading).unwrap() =
            egui::FontId::proportional(16.0);
        *style.text_styles.get_mut(&egui::TextStyle::Body).unwrap() =
            egui::FontId::proportional(12.0);
        *style.text_styles.get_mut(&egui::TextStyle::Monospace).unwrap() =
            egui::FontId::proportional(12.0);
        *style.text_styles.get_mut(&egui::TextStyle::Small).unwrap() =
            egui::FontId::proportional(11.0);
        
        // 根据主题设置视觉样式
        style.visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.hyperlink_color = Color32::from_rgb(76, 175, 255);
        
        // 优化按钮和输入框视觉层次
        style.visuals.widgets.noninteractive.bg_stroke.color = Color32::from_rgb(45, 45, 45);
        style.visuals.widgets.hovered.bg_stroke.color = Color32::from_rgb(65, 65, 65);
        style.visuals.widgets.active.bg_stroke.color = Color32::from_rgb(85, 85, 85);
        style.visuals.widgets.open.bg_stroke.color = Color32::from_rgb(65, 65, 65);
        
        // 更深的面板背景
        style.visuals.panel_fill = Color32::from_rgb(25, 25, 25);
        style.visuals.extreme_bg_color = Color32::from_rgb(25, 25, 25);
        
        // 更清晰的选中效果
        style.visuals.selection.bg_fill = Color32::from_rgb(51, 51, 51);
        
        // 更清晰的边框
        style.visuals.window_fill = Color32::from_rgb(30, 30, 30);
        style.visuals.window_stroke.color = Color32::from_rgb(51, 51, 51);
        
        // 设置行 hover 效果
        style.visuals.widgets.hovered.bg_fill = pal.hover;
        
        ctx.set_style(style);

        // ---- 顶部标题栏 ----
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(Color32::from_rgb(30, 30, 30)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.heading(RichText::new("流光 — 网络流量分析器").size(18.0));
                ui.separator();

                let evt_count = self.frontend.events.lock().map(|e| e.len()).unwrap_or(0);
                let running = self.frontend.running.load(Ordering::SeqCst);

                // 状态指示器 - 使用更大的尺寸和更鲜明的颜色
                if running {
                    ui.label(
                        RichText::new("● 运行中")
                            .color(Color32::from_rgb(0, 255, 100)) // GREEN
                            .size(16.0), // 加大尺寸
                    );
                } else {
                    ui.label(
                        RichText::new("○ 已停止")
                            .color(Color32::from_rgb(128, 128, 128)) // GRAY
                            .size(16.0), // 加大尺寸
                    );
                }

                ui.separator();
                // 事件计数 - 使用 RichText 并加粗
                ui.label(RichText::new(format!("事件 {}", evt_count)).size(14.0).strong());

                // 搜索框样式优化
                ui.add_space(20.0);

                // 使用 egui::TextEdit 的默认样式，但通过 UI 设置背景和边框
                let text_edit = egui::TextEdit::singleline(&mut self.frontend.filter_input)
                    .hint_text("过滤：主机 / PID / 方法 / 路径")
                    .desired_width(300.0);

                let response = ui.add(text_edit);
                
                // 手动绘制背景和边框
                let rect = response.rect;
                let rounded_rect = egui::Rect::from_two_pos(rect.min, rect.max);
                ui.painter().rect_filled(rounded_rect, 6.0_f32, Color32::from_rgb(45, 45, 45));
                ui.painter().add(egui::Shape::rect_stroke(rounded_rect, 6.0_f32, egui::Stroke::new(1.0_f32, Color32::from_rgb(68, 68, 68))));

                ui.add_space(10.0);

                // 按钮样式优化 - 使用圆角和 hover 效果
                if ui.add(
                    egui::Button::new("清空")
                        .rounding(egui::Rounding::from(6.0))
                        .fill(Color32::from_rgb(60, 60, 60))
                ).clicked() {
                    self.frontend.clear_events();
                }
                if ui.add(
                    egui::Button::new(if dark { "浅色" } else { "深色" })
                        .rounding(egui::Rounding::from(6.0))
                        .fill(if dark {
                            Color32::from_rgb(40, 40, 40)
                        } else {
                            Color32::from_rgb(80, 80, 80)
                        })
                ).clicked() {
                    self.frontend.dark_mode = !self.frontend.dark_mode;
                }
                ui.add_space(14.0);
            });
        });

        // ---- 主表格区域 ----
        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(pal.panel).inner_margin(egui::Margin::same(0.0))
        ).show(ctx, |ui| {
            ScrollArea::both()
                .id_salt("event_table")
                .show(ui, |ui| {
                    ui.visuals_mut().selection.bg_fill = pal.selection;
                    draw_table(ui, &mut self.frontend, &pal);
                });
        });

        // ---- 底部状态栏 ----
        egui::TopBottomPanel::bottom("status")
            .resizable(false)
            .frame(egui::Frame::none().fill(Color32::from_rgb(21, 21, 21)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // 快捷键提示 - 使用 RichText 并加粗
                    ui.label(
                        RichText::new("ESC 关闭 | F5 刷新 | 点击行查看详情")
                            .size(11.0)
                            .color(Color32::from_rgb(170, 170, 170))
                            .strong(),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("FlowLens v0.1.0")
                            .size(11.0)
                            .color(Color32::from_rgb(170, 170, 170)),
                    );
                    if let Some(idx) = self.frontend.selected_row {
                        if let Ok(events) = self.frontend.events.lock() {
                            if let Some(row) = events.get(idx) {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!(
                                        "已选: {}  {}",
                                        row.src_addr, row.path
                                    ))
                                    .size(11.0)
                                    .color(pal.secondary),
                                );
                            }
                        }
                    }
                });
            });
    }
}

// __PART4__

/// 主题色板（暗色/亮色）
mod ui {
    use egui::{Color32, Visuals};

    #[derive(Clone)]
    pub struct Palette {
        pub panel: Color32,
        pub muted: Color32,
        pub secondary: Color32,
        pub row_even: Color32,
        pub row_odd: Color32,
        pub header: Color32,
        pub header_bg: Color32,
        pub selection: Color32,
        pub hover: Color32,
        pub separator: Color32,
        #[allow(dead_code)]
        pub visuals: Visuals,
    }

    impl Palette {
        pub fn dark() -> Self {
            Self {
                panel: Color32::from_rgb(30, 30, 30), // #1E1E1E - 更深的暗色
                muted: Color32::from_rgb(170, 170, 170), // #AAAAAA
                secondary: Color32::from_rgb(190, 190, 190), // #BEBEBE
                row_even: Color32::from_rgb(32, 32, 32), // #202020 - 偶数行
                row_odd: Color32::from_rgb(37, 37, 37), // #252525 - 奇数行
                header: Color32::from_rgb(204, 204, 204), // #CCCCCC - 表头颜色
                header_bg: Color32::from_rgb(30, 30, 30), // #1E1E1E - 表头背景色
                selection: Color32::from_rgb(51, 51, 51), // #333333
                hover: Color32::from_rgb(50, 50, 50), // #323232
                separator: Color32::from_rgb(51, 51, 51), // #333333 - 行分隔线
                visuals: Visuals::dark(),
            }
        }

        pub fn light() -> Self {
            Self {
                panel: Color32::from_rgb(245, 246, 250),
                muted: Color32::from_rgb(140, 148, 160),
                secondary: Color32::from_rgb(80, 90, 106),
                row_even: Color32::from_rgb(255, 255, 255),
                row_odd: Color32::from_rgb(248, 249, 252),
                header: Color32::from_rgb(36, 42, 56),
                header_bg: Color32::from_rgb(245, 246, 250),
                selection: Color32::from_rgb(190, 215, 255),
                hover: Color32::from_rgb(230, 230, 230),
                separator: Color32::from_rgb(220, 222, 228),
                visuals: Visuals::light(),
            }
        }
    }
}

use ui::Palette;

/// 绘制事件主表格
fn draw_table(ui: &mut egui::Ui, frontend: &mut GuiFrontend, pal: &Palette) {
    // 表头样式优化
    ui.horizontal(|ui| {
        ui.scope(|ui| {
            ui.set_width(ui.available_width());
            ui.set_height(30.0_f32);
            ui.painter().rect_filled(ui.max_rect(), 0.0_f32, pal.header_bg);
            ui.horizontal(|ui| {
                ui.label(RichText::new("时间").size(12.0).color(pal.header).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("源地址").size(12.0).color(pal.header).strong());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("目标地址").size(12.0).color(pal.header).strong());
                });
                ui.label(RichText::new("类型").size(12.0).color(pal.header).strong());
                ui.label(RichText::new("方法").size(12.0).color(pal.header).strong());
                ui.label(RichText::new("状态").size(12.0).color(pal.header).strong());
                ui.label(RichText::new("大小").size(12.0).color(pal.header).strong());
                ui.label(RichText::new("路径").size(12.0).color(pal.header).strong());
            });
        });
    });
    
    // 表头分隔线 - 使用 painter 绘制
    ui.painter().add(egui::Shape::line_segment([
        egui::pos2(0.0, 30.0),
        egui::pos2(ui.max_rect().max.x, 30.0)
    ], egui::Stroke::new(1.0_f32, pal.separator)));

    let filter = frontend.filter_input.to_lowercase();
    let (filtered, total_len): (Vec<TrafficRow>, usize) = {
        let events = match frontend.events.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let picked: Vec<TrafficRow> = if filter.is_empty() {
            events.clone()
        } else {
            events
                .iter()
                .filter(|row| {
                    row.src_addr.to_lowercase().contains(&filter)
                        || row.dst_addr.to_lowercase().contains(&filter)
                        || row.method.to_lowercase().contains(&filter)
                        || row.path.to_lowercase().contains(&filter)
                })
                .cloned()
                .collect()
        };
        (picked, events.len())
    };

    for (i, row) in filtered.iter().rev().enumerate() {
        let idx = total_len.saturating_sub(1).saturating_sub(i);
        let is_selected = frontend.selected_row == Some(idx);

        let response = ui.horizontal(|ui| {
            ui.set_min_height(24.0);
            
            // 使用 hover 效果
            let bg = if is_selected {
                pal.selection
            } else if idx % 2 == 0 {
                pal.row_even
            } else {
                pal.row_odd
            };

            let _bg_rect = ui.painter().rect_filled(
                ui.max_rect(),
                0.0_f32,
                bg,
            );

            ui.label(RichText::new(format_ts(row.timestamp)).size(12.0).color(pal.secondary));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(trunc(&row.src_addr, 14)).size(12.0).color(pal.muted));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(trunc(&row.dst_addr, 14)).size(12.0).color(pal.muted));
            });

            let type_col = event_color(&row.event_type, true);
            ui.label(RichText::new(event_type_cn(&row.event_type)).size(12.0).color(type_col));
            ui.label(RichText::new(trunc(&row.method, 8)).size(12.0).color(pal.secondary));
            let status_num: u16 = row.status.parse().unwrap_or(0);
            let sc = status_color(status_num, true);
            ui.label(RichText::new(row.status.to_string()).size(12.0).color(sc));
            ui.label(RichText::new(fmt_bytes(row.body_size)).size(12.0).color(pal.secondary));
            ui.label(RichText::new(trunc(&row.path, 24)).size(12.0).color(pal.secondary));
        });

        if response.response.clicked() {
            frontend.selected_row = Some(idx);
        }

        // 绘制行分隔线（除了最后一行）
        if i < filtered.len() - 1 {
            let y = 24.0_f32;
            ui.painter().add(egui::Shape::line_segment([
                egui::pos2(0.0, y),
                egui::pos2(ui.max_rect().max.x, y)
            ], egui::Stroke::new(1.0_f32, pal.separator)));
        }
    }

    if filtered.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(if filter.is_empty() { "暂无事件" } else { "无匹配事件" }).size(13.0).color(pal.muted));
        });
    }
}

// __PART5__

impl UiFrontend for GuiFrontend {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        let frontend = Self {
            events: self.events.clone(),
            running: self.running.clone(),
            dark_mode: self.dark_mode,
            filter_input: self.filter_input.clone(),
            selected_row: self.selected_row,
            bus: Some(bus),
        };

        eframe::run_native(
            "流光 - 网络流量分析器",
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size((1280.0, 820.0))
                    .with_min_inner_size((900.0, 500.0)),
                ..Default::default()
            },
            Box::new(|_cc| {
                Ok(Box::new(FlowLensApp {
                    frontend,
                    fonts_initialized: false,
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
    fn test_gui_creation() {
        let _gui = GuiFrontend::new();
    }

    #[test]
    fn test_event_type_cn() {
        assert_eq!(event_type_cn("connection"), "连接");
        assert_eq!(event_type_cn("http_request"), "HTTP 请求");
        assert_eq!(event_type_cn("http_response"), "HTTP 响应");
        assert_eq!(event_type_cn("tls"), "TLS");
        assert_eq!(event_type_cn("websocket"), "WebSocket");
        assert_eq!(event_type_cn("sse"), "SSE");
        assert_eq!(event_type_cn("dns"), "DNS");
        assert_eq!(event_type_cn("unknown"), "其他");
    }

    #[test]
    fn test_format_ts() {
        assert_eq!(format_ts(0), "--:--:--.-");
        assert_eq!(format_ts(123456789), "10:17:36.7");
        assert_eq!(format_ts(1000), "00:00:01.0");
    }

    #[test]
    fn test_fmt_bytes() {
        assert_eq!(fmt_bytes(0), "0 B");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1024), "1.00 KB");
        assert_eq!(fmt_bytes(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn test_trunc() {
        assert_eq!(trunc("hello", 10), "hello");
        assert_eq!(trunc("hello world", 5), "hell…");
        assert_eq!(trunc("你好世界", 2), "你…");
        assert_eq!(trunc("", 5), "");
    }

    #[test]
    fn test_status_color() {
        let dark = true;
        assert_eq!(status_color(200, dark), Color32::from_rgb(76, 201, 120));
        assert_eq!(status_color(404, dark), Color32::from_rgb(240, 150, 70));
        assert_eq!(status_color(500, dark), Color32::from_rgb(235, 90, 90));
        assert_eq!(status_color(0, dark), Color32::from_rgb(150, 156, 166));
    }

    #[test]
    fn test_palette() {
        let _dark = Palette::dark();
        let _light = Palette::light();
    }
}
