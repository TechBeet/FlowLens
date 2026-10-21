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
        /// Force admin mode
        #[arg(short, long)]
        admin_force: bool,
        /// Use GUI mode instead of TUI
        #[arg(short, long)]
        gui: bool,
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

/// Check if running as administrator
pub fn is_admin() -> bool {
    // Simplified check - in production would use proper security token inspection
    #[cfg(windows)]
    {
        // Placeholder - will be implemented with proper SID checking
        true
    }
    #[cfg(not(windows))]
    {
        false
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
}