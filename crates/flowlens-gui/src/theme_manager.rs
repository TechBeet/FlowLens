//! Runtime theme switching with smooth transitions

use egui::{Context, Color32, Visuals};

use crate::styles::{ThemeManager as BaseThemeManager, ThemeMode, ColorPalette};

/// Enhanced theme manager with UI integration and smooth transitions
pub struct ThemeManager {
    pub theme: BaseThemeManager,
    /// Whether the user can toggle theme via UI
    pub allow_switch: bool,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            theme: BaseThemeManager::default(),
            allow_switch: true,
        }
    }
}

impl ThemeManager {
    pub fn new(mode: ThemeMode) -> Self {
        Self {
            theme: BaseThemeManager::new(mode),
            allow_switch: true,
        }
    }

    /// Apply theme to egui Context with smooth transitions
    pub fn apply(&self, ctx: &Context) {
        let palette = &self.theme.current_palette();
        
        // Create smooth transition visuals
        let mut visuals = Visuals::default();
        
        // Background colors with opacity
        visuals.panel_fill = palette.bg;
        visuals.widgets.noninteractive.bg_fill = palette.bg_secondary;
        visuals.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(
            palette.primary.r(),
            palette.primary.g(),
            palette.primary.b(),
            20, // 20% opacity
        );
        visuals.widgets.active.bg_fill = Color32::from_rgba_premultiplied(
            palette.primary.r(),
            palette.primary.g(),
            palette.primary.b(),
            40, // 40% opacity
        );
        
        // Text colors
        visuals.override_text_color = Some(palette.text);
        
        // Selection background
        visuals.selection.bg_fill = palette.primary;
        visuals.selection.stroke.color = Color32::WHITE;
        
        // Window styling
        visuals.window_fill = palette.surface;
        visuals.window_stroke.color = palette.border;
        visuals.window_stroke.width = 1.0;
        
        // Slider and other control colors
        visuals.widgets.noninteractive.bg_stroke.color = palette.border;
        visuals.widgets.hovered.bg_stroke.color = palette.primary;
        visuals.widgets.active.bg_stroke.color = palette.primary;
        
        // Hyperlink color
        visuals.hyperlink_color = palette.primary;
        
        // Additional styling for round corners
        visuals.clip_rect_margin = 8.0;
        
        ctx.set_visuals(visuals);
    }

    /// Switch to a new theme with smooth animation
    pub fn set_theme(&mut self, mode: ThemeMode) {
        if self.allow_switch {
            self.theme.set_mode(mode);
        }
    }

    /// Cycle to the next theme
    pub fn cycle_theme(&mut self) {
        let next = match self.theme.mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::HighContrast,
            ThemeMode::HighContrast => ThemeMode::Light,
        };
        self.set_theme(next);
    }

    /// Interpolate theme over time for smooth transitions
    pub fn update(&mut self, dt: f32) {
        self.theme.interpolate(dt);
    }

    /// Get the current palette
    pub fn palette(&self) -> &ColorPalette {
        &self.theme.current_palette()
    }

    /// Get the current mode
    pub fn mode(&self) -> ThemeMode {
        self.theme.mode
    }
}

/// Helper function to create gradient background
pub fn gradient_background(ui: &mut egui::Ui, colors: &[Color32]) {
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();
    
    if colors.len() == 1 {
        painter.rect_filled(rect, 0.0, colors[0]);
    } else {
        // Create smooth gradient
        let width = rect.width();
        let height = rect.height();
        
        for (i, color) in colors.iter().enumerate() {
            let x = (i as f32 / (colors.len() as f32 - 1. * width)) / 2.0;
            let gradient_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + x, rect.top()),
                egui::vec2(width / (colors.len() as f32 - 1.), height),
            );
            painter.rect_filled(gradient_rect, 0.0, *color);
        }
    }
}

/// Create glassmorphism effect
pub fn glassmorphism(ui: &mut egui::Ui, color: Color32, opacity: f32, blur: f32) {
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();
    
    // Draw frosted glass effect
    let alpha = (255.0 * opacity) as u8;
    let glass_color = Color32::from_rgba_premultiplied(
        color.r(),
        color.g(),
        color.b(),
        alpha,
    );
    
    painter.rect_filled(rect, blur, glass_color);
}