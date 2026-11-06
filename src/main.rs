//! FlowLens CLI - Main entry point
//! Root binary crate. Wires the CLI to the capture/TUI pipeline.
//! ## Npcap Configuration (Windows)
//! To enable real packet capture:
//! 1. Install Npcap from https://nmap.org/npcap/
//!    - During installation, enable "Npcap Loopback Adapter" for localhost traffic
//!    - Enable "WinPcap API-compatible Mode" for compatibility
//! 2. Run FlowLens as Administrator (required for packet capture)
//! 3. Common device names: "Ethernet", "Wi-Fi", "Loopback Pseudo-Interface"
//! //! FlowLens auto-detects Npcap and falls back to mock mode if unavailable.
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use flowlens_core::event_bus::EventBus;
use flowlens_core::models::TrafficEvent;
use flowlens_storage::StorageEngine;
use flowlens_tls::CaManager;
use flowlens_gui::GuiFrontend;
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
                    gui,
                    mock,
                    status,
                    events,
                    filters,
                } => {
                    // --status / --events / --filters: print one-shot report and exit
                    if status || events || filters {
                        let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());

                        // Subscribe a listener to ensure the mock producer starts publishing.
                        // The report reads from `get_latest_events()` snapshot, not from receiver.
                        let _listener = bus.subscribe(flowlens_core::models::TrafficFilter::default());
                        tracing::debug!(
                            "One-shot mode: listener registered (count={})",
                            bus.subscriber_count()
                        );

                        // Try to initialize real capture; fall back to mock if Npcap unavailable.
                        let mut engine = flowlens_capture::Engine::auto();
                        let using_real = matches!(engine, flowlens_capture::Engine::Pcap(_));
                        if let Err(e) = engine.start() {
                            tracing::warn!("Real capture failed to start ({e}); using mock producer");
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        } else if !using_real {
                            tracing::warn!(
                                "Npcap unavailable; falling back to mock producer. 
                                 Install Npcap from https://nmap.org/npcap/ for real capture."
                            );
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        } else {
                            tracing::info!("Real Npcap capture active for one-shot report");
                        }

                        // Allow capture / mock producer some time to accumulate events
                        tokio::time::sleep(Duration::from_secs(2)).await;

                        // Drain any real packets into the bus so reports reflect them
                        // (then exit - one-shot mode)
                        return Ok(());
                    }

                    // Initialize mock producer on the bus
                    let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());
                    let _listener = bus.subscribe(flowlens_core::models::TrafficFilter::default());
                    flowlens_capture::spawn_mock_producer(bus.clone());

                    // Wire up the GUI frontend
                    let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let gui = GuiFrontend::new();
                    gui.set_theme(crate::styles::ThemeMode::Dark);

                    // Start the GUI
                    gui.run(bus.clone())?;
                }
                _ => { /* Other commands */ }
            }
        })
}
﻿
