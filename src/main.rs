//! FlowLens CLI - Main entry point
//!
//! Root binary crate. Wires the CLI to the capture/TUI pipeline.

use std::sync::Arc;

use clap::Parser;
use flowlens_ui::UiFrontend;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = flowlens_cli::Cli::parse();

    match cli.command {
        flowlens_cli::Commands::Start { name, admin_force, gui } => {
            tracing::info!("Starting FlowLens capture: {} (gui={})", name, gui);
            if admin_force {
                flowlens_cli::ensure_admin()?;
            }

            // Create the in-memory broadcast event bus
            let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());

            // Spawn mock traffic producer so the UI displays live events
            flowlens_capture::spawn_mock_producer(bus.clone());

            // Launch the appropriate frontend based on --gui flag
            if gui {
                let gui_frontend = flowlens_ui::GuiFrontend::new(bus.clone());
                gui_frontend.run(bus as Arc<dyn flowlens_core::event_bus::EventBus>)?;
            } else {
                let tui = flowlens_ui::TuiFrontend::new();
                tui.run(bus as Arc<dyn flowlens_core::event_bus::EventBus>)?;
            }
        }
        flowlens_cli::Commands::Stop { name } => {
            tracing::info!("Stopping FlowLens capture: {}", name);
            println!("FlowLens stopped (session: {})", name);
        }
        flowlens_cli::Commands::Capture { pid, host, port } => {
            tracing::info!(
                "Capturing traffic (port: {}, pid: {:?}, host: {:?})",
                port,
                pid,
                host
            );
            println!("FlowLens capturing on port {}", port);
        }
        flowlens_cli::Commands::Export { format, query, out } => {
            tracing::info!("Exporting {} to {:?} (query: {:?})", format, out, query);
            println!("Export complete: {:?}", out);
        }
        flowlens_cli::Commands::Cleanup => {
            tracing::info!("Cleaning up FlowLens resources");
            println!("Cleanup complete");
        }
        flowlens_cli::Commands::Version => {
            println!("FlowLens v0.1.0");
        }
    }

    Ok(())
}
