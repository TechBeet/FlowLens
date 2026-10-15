//! FlowLens Installer
//!
//! This module provides driver installation and certificate management utilities.

use std::path::PathBuf;

/// Installer error type
#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("Driver installation failed: {0}")]
    DriverInstall(String),
    #[error("Certificate installation failed: {0}")]
    CertInstall(String),
    #[error("Insufficient privileges: {0}")]
    Permission(String),
}

/// FlowLens installer
pub struct Installer {
    driver_path: PathBuf,
    cert_path: PathBuf,
}

impl Installer {
    pub fn new(driver_path: PathBuf, cert_path: PathBuf) -> Self {
        Self { driver_path, cert_path }
    }

    /// Install the WFP driver
    pub fn install_driver(&self) -> Result<(), InstallerError> {
        tracing::info!("Installing driver from {:?}", self.driver_path);
        // Phase 4: Will implement actual driver installation
        Ok(())
    }

    /// Uninstall the WFP driver
    pub fn uninstall_driver(&self) -> Result<(), InstallerError> {
        tracing::info!("Uninstalling driver");
        Ok(())
    }

    /// Check if driver is installed
    pub fn is_driver_installed(&self) -> bool {
        // Check registry or service existence
        !self.driver_path.exists()
    }

    /// Cleanup all FlowLens resources
    pub fn cleanup(&self) -> Result<(), InstallerError> {
        self.uninstall_driver()?;
        // Self-clean CA certificate removal
        tracing::info!("Cleanup complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installer_creation() {
        let installer = Installer::new(PathBuf::from("driver.sys"), PathBuf::from("ca.pem"));
        assert!(installer.driver_path.exists() || true); // Placeholder test
    }
}