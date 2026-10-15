//! Windows-specific driver implementation
//!
//! This module provides Windows API bindings for WFP driver management.

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Networking::WinSock::*;
use windows_sys::Win32::System::Services::*;

/// Driver service name
pub const DRIVER_SERVICE_NAME: &str = "flowlens";

/// Get the driver path based on architecture
pub fn get_driver_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop();
    path.push("driver");
    path.push(format!("flowlens{}.sys", if is_x64() { "x64" } else if is_arm64() { "arm64" } else { "x86" }));
    path
}

/// Check if running on x64 architecture
pub fn is_x64() -> bool {
    // Check environment variable or use actual API
    std::env::var("PROCESSOR_ARCHITECTURE").map(|v| v == "AMD64").unwrap_or(true)
}

/// Check if running on ARM64 architecture
pub fn is_arm64() -> bool {
    std::env::var("PROCESSOR_ARCHITECTURE").map(|v| v == "ARM64").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_detection() {
        // On Windows, we can detect architecture
        let is_x64 = is_x64();
        // We expect x64 on most systems
        assert!(is_x64 || is_arm64());
    }

    #[test]
    fn test_driver_path() {
        let path = get_driver_path();
        assert_eq!(path.file_name().unwrap().to_str().unwrap().ends_with(".sys"), true);
    }
}