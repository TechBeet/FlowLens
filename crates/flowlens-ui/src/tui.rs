//! TUI frontend using ratatui

use std::sync::Arc;
use std::time::Duration;

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
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

use crate::UiFrontend;

/// Application state for the TUI frontend.
#[derive(Default)]
pub struct AppState {
    events: Arc<std::sync::Mutex<Vec<TrafficRow>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    scroll_offset: usize,
    debug_mode: bool,
    filter_input: String,
    show_filter_input: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            events: Arc::new(std::sync::Mutex::new(Vec::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            scroll_offset: 0,
            debug_mode: false,
            filter_input: String::new(),
            show_filter_input: false,
        }
    }
}

/// TUI frontend for displaying network traffic
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

        // Spawn a thread to receive events from the broadcast bus
        let events_arc = app.events.clone();
        let bus_clone = bus.clone();
        let running = app.running.clone();

        let _event_thread = std::thread::spawn(move || {
            let mut rx = bus_clone.subscribe(TrafficFilter::default());
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                match rx.blocking_recv() {
                    Ok(event) => {
                        if let Some(row) = TrafficRow::from_event(&event) {
                            if let Ok(mut evts) = events_arc.lock() {
                                evts.push(row);
                                let len = evts.len();
                                if len > 1000 {
                                    evts.drain(..len - 500);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        // Setup terminal
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stderr()))?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        enable_raw_mode()?;

        // Main render loop
        loop {
            terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Length(1), Constraint::Fill(1)])
                    .split(frame.size());

                // Update title with debug info
                let debug_hint = if app.debug_mode { " [DEBUG]" } else { "" };
                let title_text = format!(
                    "FlowLens - Network Traffic Monitor (q=quit, n/p=scroll, c=clear, d=debug, /=filter){}",
                    debug_hint
                );
                let title = Paragraph::new(title_text)
                    .style(Style::default().fg(Color::White).bg(Color::Blue));
                frame.render_widget(title, chunks[0]);

                let stats = app.events.lock().unwrap();
                let conn = stats.iter().collect::<Vec<_>>();
                let total_events = conn.len();
                drop(stats);

                // Status bar with debug info
                let status_text = if app.debug_mode {
                    format!(
                        "Events: {} | Debug Mode: ON | Press 'd' to toggle",
                        total_events
                    )
                } else {
                    format!("Events: {}", total_events)
                };
                let status = Paragraph::new(status_text)
                    .style(Style::default().fg(Color::Yellow));
                frame.render_widget(status, chunks[1]);

                let events = app.events.lock().unwrap();
                if events.is_empty() {
                    let placeholder = Paragraph::new(
                        "Waiting for traffic events...\nStart a capture to see events here.",
                    )
                    .style(Style::default().fg(Color::Gray))
                    .block(Block::default().title("Events").borders(Borders::ALL));
                    frame.render_widget(placeholder, chunks[2]);
                    return;
                }

                // Apply scroll offset (kept in sync below)
                let start = app.scroll_offset.min(events.len().saturating_sub(1));
                let visible: Vec<Row> = events
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take(chunks[2].height.saturating_sub(2) as usize)
                    .map(|(i, row)| {
                        let ts_str = format!("{}", row.timestamp);
                        let ts_short = ts_str.chars().take(12).collect::<String>();
                        let info = if row.path.len() > 30 {
                            format!("{}...", &row.path[..30])
                        } else {
                            row.path.clone()
                        };
                        let color = match row.event_type.as_str() {
                            "connection" => Color::Green,
                            "http_request" | "http_response" => Color::Cyan,
                            "tls" => Color::Yellow,
                            "websocket" => Color::Magenta,
                            "sse" => Color::LightCyan,
                            "closed" => Color::DarkGray,
                            "dropped" => Color::Red,
                            _ => Color::White,
                        };
                        let _ = i;
                        Row::new(vec![
                            ts_short,
                            row.event_type.clone(),
                            row.src_addr.clone(),
                            row.dst_addr.clone(),
                            format!("{} {}", row.method, row.status),
                            info,
                        ])
                        .style(Style::default().fg(color))
                    })
                    .collect();

                let table = Table::new(
                    visible,
                    &[
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Length(22),
                        Constraint::Length(22),
                        Constraint::Length(14),
                        Constraint::Fill(1),
                    ],
                )
                .header(
                    Row::new(vec!["Time", "Type", "Source", "Destination", "Status", "Info"])
                        .style(Style::default().bg(Color::Gray).fg(Color::Black)),
                )
                .block(
                    Block::default()
                        .title(format!("Events ({})", events.len()))
                        .borders(Borders::ALL),
                );
                frame.render_widget(table, chunks[2]);
            })?;

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(KeyEvent { code: KeyCode::Char('q'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Char('Q'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => break,
                    Event::Key(KeyEvent { code: KeyCode::Char('c'), .. }) => {
                        app.events.lock().unwrap().clear();
                        app.scroll_offset = 0;
                        println!("Events cleared");
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char('n'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Down, .. }) => {
                        let len = app.events.lock().unwrap().len();
                        if len > 0 {
                            app.scroll_offset = (app.scroll_offset + 1).min(len - 1);
                        }
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char('p'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Up, .. }) => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(1);
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char('d'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Char('D'), .. }) => {
                        app.debug_mode = !app.debug_mode;
                        println!("Debug mode: {}", if app.debug_mode { "ON" } else { "OFF" });
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char('/'), .. }) => {
                        app.show_filter_input = true;
                        app.filter_input.clear();
                        println!("Filter input opened");
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char('r'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Char('R'), .. }) => {
                        println!("Refreshing stats...");
                        let stats = app.events.lock().unwrap();
                        println!("Total events: {}", stats.len());
                        for (i, row) in stats.iter().take(5).enumerate() {
                            println!("  [{}] {} {} -> {} | {} {}",
                                i, row.timestamp, row.event_type, row.src_addr,
                                row.method, row.status
                            );
                        }
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char('f'), .. })
                    | Event::Key(KeyEvent { code: KeyCode::Char('F'), .. }) => {
                        app.events.lock().unwrap().clear();
                        app.scroll_offset = 0;
                        println!("Events flushed");
                    }
                    Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                        if app.show_filter_input {
                            println!("Filter applied: {:?}", app.filter_input);
                            app.show_filter_input = false;
                        }
                    }
                    Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) => {
                        if app.show_filter_input {
                            app.filter_input.push(c);
                        }
                    }
                    Event::Key(KeyEvent { code: KeyCode::Backspace, .. }) => {
                        if app.show_filter_input {
                            app.filter_input.pop();
                        }
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
