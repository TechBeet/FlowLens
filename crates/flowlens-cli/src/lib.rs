//! FlowLens CLI - Command line interface

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "flowlens")]
#[command(about = "A Windows native proxyless HTTP/HTTPS packet capture tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start capture with UI
    Start {
        /// Name of the capture session
        #[arg(short, long, default_value = "FlowLens")]
        name: String,
        /// Use GUI mode instead of TUI
        #[arg(short, long)]
        gui: bool,
        /// Force mock mode (skip admin check, useful for testing GUI without Npcap/admin)
        #[arg(long)]
        mock: bool,
        /// Print status report and exit
        #[arg(long, default_value = "false")]
        status: bool,
        /// Print last 20 events and exit
        #[arg(long, default_value = "false")]
        events: bool,
        /// Print active filters and exit
        #[arg(long, default_value = "false")]
        filters: bool,
    },
    /// Stop a running capture
    Stop {
        #[arg(short, long, default_value = "FlowLens")]
        name: String,
    },
    /// Capture traffic with filters
    Capture {
        /// Filter by PID
        #[arg(long)]
        pid: Option<u32>,
        /// Filter by hostname
        #[arg(long)]
        host: Option<String>,
        /// MCP server port
        #[arg(short, long, default_value = "5000")]
        port: u16,
    },
    /// Export captured sessions
    Export {
        /// Export format: har or saz
        #[arg(long, default_value = "har")]
        format: String,
        /// SQL-like query filter
        #[arg(long)]
        query: Option<String>,
        /// Output file path
        #[arg(long, default_value = "output.har")]
        out: PathBuf,
    },
    /// Cleanup system (uninstall CA, driver)
    Cleanup,
    /// Show version
    Version,
}

/// Check if running as administrator using Windows TokenElevation API
pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Security::TOKEN_ELEVATION;
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token_handle: HANDLE = std::mem::zeroed();
            if OpenProcessToken(
                GetCurrentProcess(),
                0x0008, // TOKEN_QUERY
                &mut token_handle,
            ) == 0
            {
                return false;
            }

            let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
            let mut return_len: u32 = 0;

            let success = windows_sys::Win32::Security::GetTokenInformation(
                token_handle,
                20, // TokenElevation
                &mut elevation as *mut _ as *mut _,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut return_len,
            );

            // Close the token handle
            windows_sys::Win32::Foundation::CloseHandle(token_handle);

            success != 0 && elevation.TokenIsElevated != 0
        }
    }
    #[cfg(not(windows))]
    {
        // On non-Windows, check uid == 0 (root)
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                unsafe { libc::getuid() == 0 }
            } else {
                false
            }
        }
    }
}

/// Ensure we have admin privileges
pub fn ensure_admin() -> anyhow::Result<()> {
    if !is_admin() {
        anyhow::bail!(
            "FlowLens requires administrator privileges.\n\
             Please run as Administrator or use the portable self-elevating launcher."
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_help() {
        let cli = Cli::try_parse_from(["flowlens", "--help"]);
        assert!(cli.is_err()); // --help causes exit
    }

    #[test]
    fn test_is_admin_detection() {
        let is_admin = is_admin();
        println!("Running as admin: {}", is_admin);
        // Just verify it returns a boolean, don't assert specific value
        // since it depends on how the test is run
        let _ = is_admin;
    }
}
