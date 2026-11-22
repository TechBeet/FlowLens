//! Persistent user settings for FlowLens.
//!
//! Stored as JSON in the platform app-data directory so that theme, capture
//! backend, highlight rules and responder rules survive restarts.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    pub backend: String,
    pub filter: String,
    pub auto_start: bool,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            backend: "mock".to_string(),
            filter: "all".to_string(),
            auto_start: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub capture: CaptureSettings,
    pub highlight_rules: Vec<crate::responder::HighlightingRule>,
    pub responder_rules: Vec<crate::responder::ResponderRule>,
    pub filter_profiles: Vec<crate::responder::FilterProfile>,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture: CaptureSettings::default(),
            highlight_rules: Vec::new(),
            responder_rules: Vec::new(),
            filter_profiles: Vec::new(),
            theme: "dark".to_string(),
        }
    }
}

impl Settings {
    pub fn path_in(app_data_dir: &Path) -> PathBuf {
        app_data_dir.join("flowlens").join("settings.json")
    }

    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = Settings::path_in(dir.path());
        let mut s = Settings::default();
        s.capture.backend = "driver".to_string();
        s.theme = "light".to_string();
        s.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.capture.backend, "driver");
        assert_eq!(loaded.theme, "light");
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = Settings::path_in(dir.path());
        let s = Settings::load(&path);
        assert_eq!(s.capture.backend, "mock");
    }
}