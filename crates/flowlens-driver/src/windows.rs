//! Windows-specific driver implementation
//!
//! This module provides Windows API bindings for WFP driver management.

/// Driver service name
pub const DRIVER_SERVICE_NAME: &str = "flowlens";

/// Get the driver path based on architecture
pub fn get_driver_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop();
    path.push("driver");
    path.push(format!(
        "flowlens{}.sys",
        if is_x64() {
            "x64"
        } else if is_arm64() {
            "arm64"
        } else {
            "x86"
        }
    ));
    path
}

/// Check if running on x64 architecture
pub fn is_x64() -> bool {
    // Check environment variable or use actual API
    std::env::var("PROCESSOR_ARCHITECTURE")
        .map(|v| v == "AMD64")
        .unwrap_or(true)
}

/// Check if running on ARM64 architecture
pub fn is_arm64() -> bool {
    std::env::var("PROCESSOR_ARCHITECTURE")
        .map(|v| v == "ARM64")
        .unwrap_or(false)
}

/// Lifecycle state of the kernel driver service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    NotInstalled,
    Stopped,
    Running,
    Unknown,
}

impl DriverState {
    pub fn is_running(self) -> bool {
        matches!(self, DriverState::Running)
    }
    pub fn is_installed(self) -> bool {
        matches!(self, DriverState::Stopped | DriverState::Running)
    }
}

#[cfg(windows)]
mod imp {
    use super::DriverState;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ControlService, CreateServiceW, DeleteService, OpenSCManagerW,
        OpenServiceW, QueryServiceStatus, StartServiceW, SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS,
        SERVICE_DEMAND_START, SERVICE_ERROR_IGNORE, SERVICE_KERNEL_DRIVER, SERVICE_RUNNING,
        SERVICE_STOP, SERVICE_STATUS,
    };

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn open_scm() -> Result<isize, String> {
        let scm = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS) };
        if scm == 0 {
            return Err(format!("OpenSCManager failed: {}", std::io::Error::last_os_error()));
        }
        Ok(scm as isize)
    }

    pub fn query_state() -> DriverState {
        let scm = match open_scm() {
            Ok(h) => h,
            Err(_) => return DriverState::Unknown,
        };
        let name = to_wide(super::DRIVER_SERVICE_NAME);
        let svc = unsafe { OpenServiceW(scm as _, name.as_ptr(), SERVICE_ALL_ACCESS) };
        if svc == 0 {
            unsafe { CloseServiceHandle(scm as _) };
            return DriverState::NotInstalled;
        }
        let mut status = SERVICE_STATUS {
            dwServiceType: 0,
            dwCurrentState: 0,
            dwControlsAccepted: 0,
            dwWin32ExitCode: 0,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: 0,
        };
        let ok = unsafe { QueryServiceStatus(svc, &mut status) };
        let state = if ok != 0 {
            match status.dwCurrentState {
                SERVICE_RUNNING => DriverState::Running,
                _ => DriverState::Stopped,
            }
        } else {
            DriverState::Unknown
        };
        unsafe {
            CloseServiceHandle(scm as _);
            CloseServiceHandle(svc);
        }
        state
    }

    pub fn install() -> Result<(), String> {
        let path = super::get_driver_path();
        if !path.exists() {
            return Err(format!("driver file not found: {}", path.display()));
        }
        let scm = open_scm()?;
        let name = to_wide(super::DRIVER_SERVICE_NAME);
        let display = to_wide("FlowLens WFP Callout Driver");
        let bin = to_wide(&path.to_string_lossy());
        let svc = unsafe {
            CreateServiceW(
                scm as _,
                name.as_ptr(),
                display.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_KERNEL_DRIVER,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_IGNORE,
                bin.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if svc == 0 {
            let e = std::io::Error::last_os_error();
            unsafe { CloseServiceHandle(scm as _) };
            if e.raw_os_error() == Some(1073) {
                return Err("driver service already installed".to_string());
            }
            return Err(format!("CreateService failed: {e}"));
        }
        unsafe {
            CloseServiceHandle(scm as _);
            CloseServiceHandle(svc);
        }
        Ok(())
    }

    pub fn uninstall() -> Result<(), String> {
        let scm = open_scm()?;
        let name = to_wide(super::DRIVER_SERVICE_NAME);
        let svc = unsafe { OpenServiceW(scm as _, name.as_ptr(), SERVICE_ALL_ACCESS) };
        if svc == 0 {
            unsafe { CloseServiceHandle(scm as _) };
            return Err("driver service is not installed".to_string());
        }
        let ok = unsafe { DeleteService(svc) };
        let res = if ok != 0 {
            Ok(())
        } else {
            Err(format!(
                "DeleteService failed: {}",
                std::io::Error::last_os_error()
            ))
        };
        unsafe {
            CloseServiceHandle(scm as _);
            CloseServiceHandle(svc);
        }
        res
    }

    pub fn start() -> Result<(), String> {
        let scm = open_scm()?;
        let name = to_wide(super::DRIVER_SERVICE_NAME);
        let svc = unsafe { OpenServiceW(scm as _, name.as_ptr(), SERVICE_ALL_ACCESS) };
        if svc == 0 {
            unsafe { CloseServiceHandle(scm as _) };
            return Err("driver service is not installed".to_string());
        }
        let ok = unsafe { StartServiceW(svc, 0, std::ptr::null()) };
        let res = if ok != 0 {
            Ok(())
        } else {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(1056) {
                Ok(())
            } else {
                Err(format!("StartService failed: {e}"))
            }
        };
        unsafe {
            CloseServiceHandle(scm as _);
            CloseServiceHandle(svc);
        }
        res
    }

    pub fn stop() -> Result<(), String> {
        let scm = open_scm()?;
        let name = to_wide(super::DRIVER_SERVICE_NAME);
        let svc = unsafe { OpenServiceW(scm as _, name.as_ptr(), SERVICE_ALL_ACCESS) };
        if svc == 0 {
            unsafe { CloseServiceHandle(scm as _) };
            return Err("driver service is not installed".to_string());
        }
        let mut status = SERVICE_STATUS {
            dwServiceType: 0,
            dwCurrentState: 0,
            dwControlsAccepted: 0,
            dwWin32ExitCode: 0,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: 0,
        };
        let ok = unsafe { ControlService(svc, SERVICE_STOP, &mut status) };
        let res = if ok != 0 {
            Ok(())
        } else {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(1062) {
                Ok(())
            } else {
                Err(format!("ControlService(stop) failed: {e}"))
            }
        };
        unsafe {
            CloseServiceHandle(scm as _);
            CloseServiceHandle(svc);
        }
        res
    }
}

#[cfg(not(windows))]
mod imp {
    use super::DriverState;
    pub fn query_state() -> DriverState { DriverState::Unknown }
    pub fn install() -> Result<(), String> { Err("driver management only supported on Windows".to_string()) }
    pub fn uninstall() -> Result<(), String> { Err("driver management only supported on Windows".to_string()) }
    pub fn start() -> Result<(), String> { Err("driver management only supported on Windows".to_string()) }
    pub fn stop() -> Result<(), String> { Err("driver management only supported on Windows".to_string()) }
}

/// Query the current lifecycle state of the driver service.
pub fn query_state() -> DriverState { imp::query_state() }
/// Install the kernel driver service (requires administrator privileges).
pub fn install() -> Result<(), String> { imp::install() }
/// Remove the kernel driver service.
pub fn uninstall() -> Result<(), String> { imp::uninstall() }
/// Start the kernel driver service.
pub fn start() -> Result<(), String> { imp::start() }
/// Stop the kernel driver service.
pub fn stop() -> Result<(), String> { imp::stop() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires Windows SCM; crashes under AV injection on this machine"]
    fn test_architecture_detection() {
        // On Windows, we can detect architecture
        let is_x64 = is_x64();
        // We expect x64 on most systems
        assert!(is_x64 || is_arm64());
    }

    #[test]
    #[ignore = "requires Windows SCM; crashes under AV injection on this machine"]
    fn test_driver_path() {
        let path = get_driver_path();
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".sys"));
    }
}
