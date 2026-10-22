//! FlowLens CLI - Main entry point
//!
//! Root binary crate. Wires the CLI to the capture/TUI pipeline.
//!
//! ## Npcap Configuration (Windows)
//!
//! To enable real packet capture:
//! 1. Install Npcap from https://nmap.org/npcap/
//!    - During installation, enable "Npcap Loopback Adapter" for localhost traffic
//!    - Enable "WinPcap API-compatible Mode" for compatibility
//! 2. Run FlowLens as Administrator (required for packet capture)
//! 3. Common device names: "Ethernet", "Wi-Fi", "Loopback Pseudo-Interface"
//!
//! FlowLens auto-detects Npcap and falls back to mock mode if unavailable.

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use flowlens_core::event_bus::EventBus;
use flowlens_ui::UiFrontend;

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

                        // Try to initialize real capture; fall back to mock if Npcap unavailable.
                        let mut engine = flowlens_capture::Engine::auto();
                        let using_real = matches!(engine, flowlens_capture::Engine::Pcap(_));
                        if let Err(e) = engine.start() {
                            tracing::warn!("Real capture failed to start ({e}); using mock producer");
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        } else if !using_real {
                            tracing::warn!(
                                "Npcap unavailable; falling back to mock producer. \
                                 Install Npcap from https://nmap.org/npcap/ for real capture."
                            );
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        } else {
                            tracing::info!("Real Npcap capture active for one-shot report");
                        }

                        // Allow capture / mock producer some time to accumulate events
                        tokio::time::sleep(Duration::from_secs(2)).await;

                        // Drain any real packets into the bus so reports reflect them
                        let bus_ref: Arc<dyn flowlens_core::event_bus::EventBus> = bus.clone();
                        flowlens_capture::pump_engine_into_bus(&engine, &bus_ref);

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

                    // Initialize real capture, auto-detecting Npcap. Falls back to mock
                    // producer if Npcap is not installed or the device cannot be opened.
                    let mut engine = flowlens_capture::Engine::auto();
                    let using_real = matches!(engine, flowlens_capture::Engine::Pcap(_));

                    match engine.start() {
                        Ok(()) if using_real => {
                            tracing::info!("Real Npcap capture active");
                            // Continuously pump captured packets into the EventBus in the background.
                            flowlens_capture::spawn_engine_pump(engine, bus.clone());
                        }
                        Ok(()) => {
                            tracing::warn!(
                                "Npcap unavailable; running in MOCK mode. \
                                 Install Npcap from https://nmap.org/npcap/ and run as Administrator \
                                 for real capture."
                            );
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to start real capture ({e}); running in MOCK mode."
                            );
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        }
                    }

                    // Launch the appropriate frontend based on --gui flag
                    if gui {
                        let gui_frontend = flowlens_ui::GuiFrontend::new();
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
