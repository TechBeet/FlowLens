//! Theme system with multiple color palettes and smooth transitions.

use egui::Color32;

/// Theme modes available
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
    HighContrast,
}

/// Global color palette for the application
#[derive(Clone)]
pub struct ColorPalette {
    pub bg: Color32,
    pub bg_secondary: Color32,
    pub bg_tertiary: Color32,
    pub surface: Color32,
    pub surface_elevated: Color32,
    pub border: Color32,
    pub border_light: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_inverse: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_active: Color32,
    pub success: Color32,
    pub success_hover: Color32,
    pub warning: Color32,
    pub warning_hover: Color32,
    pub error: Color32,
    pub error_hover: Color32,
    pub http: Color32,
    pub https: Color32,
    pub websocket: Color32,
    pub tls: Color32,
    pub grpc: Color32,
}

impl ColorPalette {
    /// Get the palette for the given theme mode
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => ColorPalette {
                bg: Color32::from_rgb(255, 255, 255),
                bg_secondary: Color32::from_rgb(245, 245, 245),
                bg_tertiary: Color32::from_rgb(230, 230, 230),
                surface: Color32::from_rgb(245, 245, 245),
                surface_elevated: Color32::from_rgb(255, 255, 255),
                border: Color32::from_rgb(200, 200, 200),
                border_light: Color32::from_rgb(220, 220, 220),
                text: Color32::from_rgb(26, 26, 26),
                text_muted: Color32::from_rgb(120, 120, 120),
                text_inverse: Color32::from_rgb(26, 26, 26),
                primary: Color32::from_rgb(100, 150, 255),
                primary_hover: Color32::from_rgb(120, 170, 255),
                primary_active: Color32::from_rgb(80, 130, 235),
                success: Color32::from_rgb(100, 200, 100),
                success_hover: Color32::from_rgb(120, 220, 120),
                warning: Color32::from_rgb(255, 200, 100),
                warning_hover: Color32::from_rgb(255, 220, 120),
                error: Color32::from_rgb(255, 100, 100),
                error_hover: Color32::from_rgb(255, 120, 120),
                http: Color32::from_rgb(100, 150, 255),
                https: Color32::from_rgb(100, 200, 100),
                websocket: Color32::from_rgb(255, 200, 100),
                tls: Color32::from_rgb(255, 100, 100),
                grpc: Color32::from_rgb(200, 100, 255),
            },
            ThemeMode::HighContrast => ColorPalette {
                bg: Color32::from_rgb(0, 0, 0),
                bg_secondary: Color32::from_rgb(0, 0, 0),
                bg_tertiary: Color32::from_rgb(0, 0, 0),
                surface: Color32::from_rgb(0, 0, 0),
                surface_elevated: Color32::from_rgb(0, 0, 0),
                border: Color32::from_rgb(255, 255, 255),
                border_light: Color32::from_rgb(255, 255, 255),
                text: Color32::from_rgb(255, 255, 255),
                text_muted: Color32::from_rgb(200, 200, 200),
                text_inverse: Color32::from_rgb(255, 255, 255),
                primary: Color32::from_rgb(255, 255, 0),
                primary_hover: Color32::from_rgb(255, 255, 150),
                primary_active: Color32::from_rgb(200, 200, 0),
                success: Color32::from_rgb(100, 200, 100),
                success_hover: Color32::from_rgb(120, 220, 120),
                warning: Color32::from_rgb(255, 200, 100),
                warning_hover: Color32::from_rgb(255, 220, 120),
                error: Color32::from_rgb(255, 100, 100),
                error_hover: Color32::from_rgb(255, 120, 120),
                http: Color32::from_rgb(255, 255, 0),
                https: Color32::from_rgb(100, 200, 100),
                websocket: Color32::from_rgb(255, 200, 100),
                tls: Color32::from_rgb(255, 100, 100),
                grpc: Color32::from_rgb(200, 100, 255),
            },
            ThemeMode::Dark => ColorPalette {
                bg: Color32::from_rgb(30, 30, 30),
                bg_secondary: Color32::from_rgb(40, 40, 40),
                bg_tertiary: Color32::from_rgb(50, 50, 50),
                surface: Color32::from_rgb(35, 35, 35),
                surface_elevated: Color32::from_rgb(45, 45, 45),
                border: Color32::from_rgb(60, 60, 60),
                border_light: Color32::from_rgb(70, 70, 70),
                text: Color32::from_rgb(220, 220, 220),
                text_muted: Color32::from_rgb(150, 150, 150),
                text_inverse: Color32::from_rgb(30, 30, 30),
                primary: Color32::from_rgb(100, 150, 255),
                primary_hover: Color32::from_rgb(120, 170, 255),
                primary_active: Color32::from_rgb(80, 130, 235),
                success: Color32::from_rgb(100, 200, 100),
                success_hover: Color32::from_rgb(120, 220, 120),
                warning: Color32::from_rgb(255, 200, 100),
                warning_hover: Color32::from_rgb(255, 220, 120),
                error: Color32::from_rgb(255, 100, 100),
                error_hover: Color32::from_rgb(255, 120, 120),
                http: Color32::from_rgb(100, 150, 255),
                https: Color32::from_rgb(100, 200, 100),
                websocket: Color32::from_rgb(255, 200, 100),
                tls: Color32::from_rgb(255, 100, 100),
                grpc: Color32::from_rgb(200, 100, 255),
            },
        }
    }
}

/// Theme manager with color palettes and smooth transitions
pub struct ThemeManager {
    pub mode: ThemeMode,
    pub palette: ColorPalette,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            palette: ColorPalette::for_mode(ThemeMode::Dark),
        }
    }
}

impl ThemeManager {
    pub fn new(mode: ThemeMode) -> Self {
        let mut tm = Self::default();
        tm.mode = mode;
        tm
    }

    pub fn current_palette(&self) -> &ColorPalette {
        &self.palette
    }

    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.mode = mode;
        self.palette = ColorPalette::for_mode(mode);
    }

    pub fn interpolate(&mut self, _dt: f32) {
        // Interpolation is now handled in theme_manager.rs apply()
    }
}
