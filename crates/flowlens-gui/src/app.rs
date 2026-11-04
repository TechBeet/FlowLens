//! egui App implementation for FlowLens GUI.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use flowlens_core::event_bus::EventBus;
use flowlens_core::models::TrafficEvent;

use crate::theme_manager::ThemeManager;
use crate::widgets;
use crate::styles::{ThemeMode};

/// Shared GUI application state.
pub struct GuiApp {
    #[allow(dead_code)]
    bus: Arc<dyn EventBus>,
    running: Arc<AtomicBool>,
    events: Vec<TrafficEvent>,
    filter: String,
    debug_mode: bool,
    selected: Option<usize>,
    theme: ThemeManager,
}

impl GuiApp {
    pub fn new(bus: Arc<dyn EventBus>, running: Arc<AtomicBool>) -> Self {
        Self {
            bus,
            running,
            events: Vec::new(),
            filter: String::new(),
            debug_mode: false,
            selected: None,
            theme: ThemeManager::new(ThemeMode::Dark),
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        self.theme.apply(ctx);
        self.theme.update(1.0 / 60.0);

        // Top header bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            widgets::header_bar(ui, self.theme.palette());
        });

        // Filter input
        egui::TopBottomPanel::top("filter_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut self.filter);
                if ui.button("Clear").clicked() {
                    self.filter.clear();
                }
                if ui.button("Debug").clicked() {
                    self.debug_mode = !self.debug_mode;
                }
            });
        });

        // Central event list
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, ev) in self.events.iter().enumerate() {
                    let selected = self.selected == Some(i);
                    widgets::event_card(ui, self.theme.palette(), "HTTP", format!("{:?}", ev), "src".to_string(), "dst".to_string(), 100);
                }
            });
        });

        // Exit on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.running.store(false, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}