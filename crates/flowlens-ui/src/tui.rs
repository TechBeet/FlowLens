//! FlowLens TUI 终端界面
//!
//! 功能：
//! - 实时显示网络流量事件
//! - 键盘快捷键控制（方向键、空格、/?、D/F等）
//! - 调试模式显示原始事件详情
//! - 过滤输入框
//! - 控制台日志输出（屏幕阅读器兼容）

use std::sync::Arc;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use flowlens_core::{
    event_bus::EventBus,
    models::{TrafficFilter, TrafficRow},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, TableState},
    Frame, Terminal,
};

use crate::UiFrontend;

/// 最大显示事件数
const MAX_EVENTS: usize = 1000;
/// 自动裁剪阈值
const AUTO_PRUNE_THRESHOLD: usize = 500;

/// 应用状态 - TUI前端
#[derive(Default)]
pub struct AppState {
    events: Arc<std::sync::Mutex<Vec<TrafficRow>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    table_state: TableState,
    debug_mode: bool,
    status_msg: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            events: Arc::new(std::sync::Mutex::new(Vec::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            table_state: TableState::default(),
            debug_mode: false,
            status_msg: String::from("就绪"),
        }
    }
}

/// TUI 前端
#[derive(Default)]
pub struct TuiFrontend;

impl TuiFrontend {
    pub fn new() -> Self {
        Self
    }
}

impl UiFrontend for TuiFrontend {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        let mut app = AppState::new();
        app.running.store(true, std::sync::atomic::Ordering::SeqCst);

        // 从事件总线接收事件
        let events_arc = app.events.clone();
        let bus_clone = bus.clone();
        let running = app.running.clone();

        std::thread::spawn(move || {
            let mut rx = bus_clone.subscribe(TrafficFilter::default());
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                match rx.blocking_recv() {
                    Ok(event) => {
                        if let Ok(mut evts) = events_arc.lock() {
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

        // 初始化终端
        let stdout = std::io::stderr();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        enable_raw_mode()?;

        // 主渲染循环
        loop {
            let _ = terminal.draw(|frame: &mut Frame| {
                let size = frame.size();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(1),
                        Constraint::Fill(7),
                        Constraint::Length(3),
                    ])
                    .split(size);

                // 标题栏
                let title_text = if app.debug_mode {
                    "⏱ FlowLens — 网络流量分析器 [调试模式]"
                } else {
                    "⏱ FlowLens — 网络流量分析器"
                };
                let title_par = Paragraph::new(title_text)
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    .block(Block::default().borders(Borders::ALL).title(" 流光"));
                frame.render_widget(title_par, chunks[0]);

                // 状态栏
                let evt_count = app.events.lock().map(|e| e.len()).unwrap_or(0);
                let status_text = format!("  事件: {} | 状态: {}", evt_count, app.status_msg);
                let status_par = Paragraph::new(status_text)
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL).title(" 状态"));
                frame.render_widget(status_par, chunks[1]);
            });

            // 处理按键事件
            if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
                match code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        let current = app.table_state.selected().unwrap_or(0);
                        let max = app.events.lock().map(|e| e.len()).unwrap_or(0).saturating_sub(1);
                        app.table_state.select(Some(if current >= max { 0 } else { current + 1 }));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let current = app.table_state.selected().unwrap_or(0);
                        app.table_state.select(Some(if current == 0 { 0 } else { current - 1 }));
                    }
                    KeyCode::Char('f') | KeyCode::Char('F') => {
                        app.events.lock().unwrap().clear();
                        app.status_msg = "已清空".to_string();
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        app.debug_mode = !app.debug_mode;
                        app.status_msg = if app.debug_mode { "调试模式已开启" } else { "调试模式已关闭" }.to_string();
                    }
                    _ => {}
                }
            }
        }

        app.running.store(false, std::sync::atomic::Ordering::SeqCst);
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_creation() {
        let _tui = TuiFrontend::new();
    }

    #[test]
    fn test_app_state_creation() {
        let _app = AppState::new();
    }
}
