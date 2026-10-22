//! HAR export and SAZ import for FlowLens

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
        let entries: Vec<serde_json::Value> = sessions.iter().map(|s| {
            serde_json::json!({
                "startedDateTime": format!("{}Z", s.started_at),
                "request": {
                    "method": s.http_request.as_ref().map(|r| &r.method).unwrap_or(&String::new()),
                    "url": s.http_request.as_ref().map(|r| &r.url).unwrap_or(&String::new()),
                },
                "response": {
                    "status": s.http_response.as_ref().map(|r| r.status_code).unwrap_or(0),
                }
            })
        }).collect();

        let har = serde_json::json!({
            "log": {
                "version": "1.2",
                "creator": { "name": "FlowLens", "version": "0.1.0" },
                "entries": entries,
            }
        });
        Ok(serde_json::to_string_pretty(&har)?)
    }

    fn export_to_file(&self, sessions: &[TrafficSession], path: &Path) -> anyhow::Result<()> {
        let content = self.export(sessions)?;
        std::fs::write(path, content)?;
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
