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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_empty() {
        let exporter = HarExporterImpl::new();
        let result = exporter.export(&[]).unwrap();
        assert!(result.contains("\"entries\""));
    }
}
