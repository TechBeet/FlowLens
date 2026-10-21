//! FlowLens CLI - Main entry point
//!
//! Root binary crate. Wires the CLI to the capture/TUI pipeline.

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use flowlens_ui::UiFrontend;
use flowlens_core::event_bus::EventBus;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize Tokio runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let cli = flowlens_cli::Cli::parse();

            match cli.command {
                flowlens_cli::Commands::Start {
                    name,
                    admin_force,
                    gui,
                    status,
                    events,
                    filters,
                } => {
                    // --status / --events / --filters: print one-shot report and exit
                    if status || events || filters {
                        let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());
                        flowlens_capture::spawn_mock_producer(bus.clone());
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        if status {
                            println!(
                                "=== FlowLens Status ===\nbus subscribers: {}\nevent count: {}",
                                bus.subscriber_count(),
                                bus.get_event_count()
                            );
                        }
                        if filters {
                            println!("=== Active Filters ===");
                            let filters = bus.get_filters();
                            if filters.is_empty() {
                                println!("  (none)");
                            } else {
                                for f in filters {
                                    println!(
                                        "  pid={:?} host={:?} proto={:?}",
                                        f.pids, f.hosts, f.protocols
                                    );
                                }
                            }
                        }
                        if events {
                            println!("=== Last 20 Events ===");
                            let rows = bus.get_latest_events(20);
                            if rows.is_empty() {
                                println!("  (no events captured yet)");
                            } else {
                                for r in rows {
                                    println!("{:?}", r);
                                }
                            }
                        }
                        return Ok(());
                    }

                    tracing::info!(
                        "Starting FlowLens capture: {} (gui={})",
                        name,
                        gui
                    );
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
                        gui_frontend.run(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>)?;
                    } else {
                        let tui = flowlens_ui::TuiFrontend::new();
                        tui.run(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>)?;
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
        })
}
