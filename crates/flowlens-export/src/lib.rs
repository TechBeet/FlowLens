//! HAR export and SAZ import for FlowLens

pub mod har;
pub mod saz;

use flowlens_core::models::TrafficSession;
use std::path::Path;

/// HAR export trait
pub trait HarExporter: Send + Sync {
    fn export(&self, sessions: &[TrafficSession]) -> anyhow::Result<String>;
    fn export_to_file(&self, sessions: &[TrafficSession], path: &Path) -> anyhow::Result<()>;
}

/// SAZ import trait
pub trait SazImporter: Send + Sync {
    fn import(&self, path: &Path) -> anyhow::Result<Vec<TrafficSession>>;
}

/// HAR exporter implementation
pub struct HarExporterImpl;

impl HarExporterImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HarExporterImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl HarExporter for HarExporterImpl {
    fn export(&self, sessions: &[TrafficSession]) -> anyhow::Result<String> {
        Ok(har::HarExporter::export_sessions(sessions)?)
    }

    fn export_to_file(&self, sessions: &[TrafficSession], path: &Path) -> anyhow::Result<()> {
        std::fs::write(path, self.export(sessions)?)?;
        Ok(())
    }
}

/// HAR import trait
pub trait HarImporter: Send + Sync {
    fn import(&self, json: &str) -> anyhow::Result<Vec<TrafficSession>>;
    fn import_from_file(&self, path: &Path) -> anyhow::Result<Vec<TrafficSession>>;
}

/// SAZ export trait
pub trait SazExporter: Send + Sync {
    fn export(&self, sessions: &[TrafficSession], path: &Path) -> anyhow::Result<()>;
}

/// HAR importer implementation.
pub struct HarImporterImpl;

impl HarImporterImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HarImporterImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl HarImporter for HarImporterImpl {
    fn import(&self, json: &str) -> anyhow::Result<Vec<TrafficSession>> {
        Ok(har::HarImporter::import_str(json)?)
    }

    fn import_from_file(&self, path: &Path) -> anyhow::Result<Vec<TrafficSession>> {
        har::HarImporter::import_file(path)
    }
}

/// SAZ exporter implementation.
pub struct SazExporterImpl;

impl SazExporterImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SazExporterImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl SazExporter for SazExporterImpl {
    fn export(&self, sessions: &[TrafficSession], path: &Path) -> anyhow::Result<()> {
        saz::SazExporter::new().export_sessions(sessions, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_empty() {
        let exporter = HarExporterImpl::new();
        let result = exporter.export(&[]).unwrap();
        assert!(result.contains("\"entries\""));
    }

    #[test]
    fn test_har_import_empty() {
        let importer = HarImporterImpl::new();
        let sessions = importer.import("{\"log\":{\"version\":\"1.2\",\"creator\":{\"name\":\"x\",\"version\":\"1\"},\"entries\":[]}}").unwrap();
        assert!(sessions.is_empty());
    }
}
