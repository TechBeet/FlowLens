//! FlowLens WFP Callout Driver Interface

pub mod windows;

use std::time::Duration;

use flowlens_capture::{CaptureEngine, RawPacket};

/// Windows WFP driver interface
#[derive(Debug)]
pub struct WfpDriver {
    driver_available: bool,
    interfaces: Vec<String>,
}

impl WfpDriver {
    /// Create a new WFP driver instance
    pub fn new() -> Self {
        Self {
            driver_available: false,
            interfaces: vec![
                "Wi-Fi".to_string(),
                "Ethernet".to_string(),
                "Loopback".to_string(),
            ],
        }
    }

    /// Check if WFP driver is available
    pub fn is_available(&self) -> bool {
        self.driver_available
    }

    /// Get available network interfaces
    pub fn get_interfaces(&self) -> &[String] {
        &self.interfaces
    }
}

impl Default for WfpDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureEngine for WfpDriver {
    fn start(&mut self) -> anyhow::Result<()> {
        tracing::info!("Starting WFP driver");
        self.driver_available = true;
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("Stopping WFP driver");
        self.driver_available = false;
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.driver_available
    }

    async fn receive_packet(&mut self) -> Option<RawPacket> {
        // In production, this would receive packets from the WFP driver
        // For now, return None as this is a placeholder
        tokio::time::sleep(Duration::from_millis(100)).await;
        None
    }
}

/// WFP driver error type
#[derive(Debug, thiserror::Error)]
pub enum WfpDriverError {
    #[error("WFP driver not available")]
    NotAvailable,
    #[error("Failed to initialize driver: {0}")]
    InitFailure(String),
    #[error("Driver communication error: {0}")]
    CommError(String),
}

/// Driver metrics
#[derive(Debug, Clone, Default)]
pub struct DriverMetrics {
    pub packets_received: u64,
    pub packets_processed: u64,
    pub errors: u64,
    pub start_time: i64,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_creation() {
        let driver = WfpDriver::new();
        assert!(!driver.is_available()); // Starts as not available
        assert!(!driver.get_interfaces().is_empty());
        assert_eq!(driver.get_interfaces().len(), 3);
    }

    #[test]
    fn test_driver_start_stop() {
        let mut driver = WfpDriver::new();
        assert!(!driver.is_running());

        driver.start().unwrap();
        assert!(driver.is_running());

        driver.stop().unwrap();
        assert!(!driver.is_running());
    }

    #[test]
    fn test_driver_interfaces() {
        let driver = WfpDriver::new();
        let interfaces = driver.get_interfaces();
        assert!(interfaces.contains(&"Wi-Fi".to_string()));
        assert!(interfaces.contains(&"Ethernet".to_string()));
        assert!(interfaces.contains(&"Loopback".to_string()));
    }
}
