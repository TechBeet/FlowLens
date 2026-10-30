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
use flowlens_storage::StorageEngine;
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
                    // Skip admin check when --mock flag is set (useful for testing)
                    if !mock {
                        flowlens_cli::ensure_admin()?;
                    }

                    // Create the in-memory broadcast event bus
                    let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());

                    tracing::info!(
                        "Event bus ready; subscribers will be added by frontend"
                    );

                    // Initialize real capture, auto-detecting Npcap. Falls back to mock
                    // producer if Npcap is not installed, device cannot be opened, or --mock flag is set.
                    if mock {
                        // Force mock mode when --mock flag is set (useful for testing without admin)
                        tracing::warn!("--mock flag set: forcing MOCK mode. Install Npcap and run as Administrator for real capture.");
                        flowlens_capture::spawn_mock_producer(bus.clone());
                    } else {
                        let mut engine = flowlens_capture::Engine::auto();
                        let using_real = matches!(engine, flowlens_capture::Engine::Pcap(_));

                        match engine.start() {
                            Ok(()) if using_real => {
                                tracing::info!("Real Npcap capture active");
                                flowlens_capture::spawn_engine_pump(engine, bus.clone());
                            }
                            Ok(()) => {
                                tracing::warn!("Npcap unavailable; running in MOCK mode. Install Npcap from https://nmap.org/npcap/ and run as Administrator for real capture.");
                                flowlens_capture::spawn_mock_producer(bus.clone());
                            }
                            Err(e) => {
                                tracing::warn!("Failed to start real capture ({e}); running in MOCK mode.");
                                flowlens_capture::spawn_mock_producer(bus.clone());
                            }
                        }
                    }

                    // Launch the appropriate frontend based on --gui flag
                    if gui {
                        tracing::warn!(
                            "--gui is deprecated; the egui frontend was removed. \
                             Launching the TUI instead."
                        );
                        let tui = flowlens_ui::TuiFrontend::new();
                        tui.run(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>)?;
                    } else {
                        let tui = flowlens_ui::TuiFrontend::new();
                        tui.run(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>)?;
                    }
                }
                flowlens_cli::Commands::Stop { name } => {
                    tracing::info!("Stopping FlowLens capture: {}", name);
                    println!("FlowLens stopped (session: {})", name);
                }
                flowlens_cli::Commands::Capture {
                    pid,
                    host,
                    port,
                    duration,
                    db,
                    mock,
                    verbose,
                } => {
                    tracing::info!(
                        "Capturing traffic (port: {}, pid: {:?}, host: {:?})",
                        port,
                        pid,
                        host
                    );

                    // Build the subscriber filter from the CLI arguments.
                    let mut filter = flowlens_core::models::TrafficFilter::default();
                    if let Some(p) = pid {
                        filter.pids = Some(vec![p]);
                    }
                    if let Some(h) = host.as_ref() {
                        filter.hosts = Some(vec![h.clone()]);
                    }

                    // Subscribe BEFORE the producer starts so no event is lost.
                    let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());
                    let mut rx = bus.subscribe(filter.clone());

                    // Open the SQLite database for persistence.
                    let storage = flowlens_storage::SqliteStorage::new(&db).await?;

                    // Start the capture engine (mock fallback keeps the command usable
                    // on machines without Npcap or admin rights).
                    let mut engine = flowlens_capture::Engine::auto();
                    let using_real = matches!(engine, flowlens_capture::Engine::Pcap(_));
                    let started = if mock {
                        false
                    } else {
                        engine.start().is_ok() && using_real
                    };

                    if started {
                        tracing::info!("Real Npcap capture active");
                        flowlens_capture::spawn_engine_pump(engine, bus.clone());
                    } else {
                        tracing::warn!(
                            "Npcap unavailable or --mock set; running in MOCK mode. \
                             Install Npcap and run as Administrator for real capture."
                        );
                        flowlens_capture::spawn_mock_producer(bus.clone());
                    }

                    // Optionally expose the captured traffic over MCP.
                    if port > 0 {
                        let mcp_storage = Arc::new(flowlens_storage::SqliteStorage::new(&db).await?);
                        let mut mcp = flowlens_mcp::McpServerImpl::new();
                        mcp.register_tool(Arc::new(
                            flowlens_mcp::QuerySessionsTool::new(mcp_storage.clone()),
                        ));
                        mcp.register_tool(Arc::new(flowlens_mcp::GetSessionTool::new(
                            mcp_storage.clone(),
                        )));
                        mcp.register_tool(Arc::new(
                            flowlens_mcp::AggregateByHostTool::new(mcp_storage.clone()),
                        ));
                        mcp.register_tool(Arc::new(
                            flowlens_mcp::AggregateByPidTool::new(mcp_storage.clone()),
                        ));
                        let bind = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                        tokio::spawn(async move {
                            if let Err(e) = mcp.start(bind).await {
                                tracing::warn!("MCP server failed to start: {}", e);
                            }
                        });
                        tracing::info!("MCP server listening on 127.0.0.1:{}", port);
                    }

                    println!(
                        "FlowLens capturing ({}). Database: {}",
                        if started { "Npcap" } else { "mock" },
                        db.display()
                    );
                    if verbose {
                        println!("  pid={:?} host={:?} mcp_port={}", pid, host, port);
                    }

                    // Consume events until the deadline or Ctrl+C.
                    let deadline = if duration > 0 {
                        Some(tokio::time::Instant::now() + Duration::from_secs(duration))
                    } else {
                        None
                    };

                    let mut session_count = 0u64;
                    loop {
                        let recv = tokio::select! {
                            r = rx.recv() => r,
                            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                                if let Some(dl) = deadline {
                                    if tokio::time::Instant::now() >= dl {
                                        break;
                                    }
                                }
                                continue;
                            }
                        };

                        match recv {
                            Ok(event) => {
                                session_count += 1;
                                if verbose {
                                    println!("[{}] {}", session_count, describe_event(&event));
                                }
                                // Persist HTTP exchanges as sessions.
                                if let Some(session) = session_from_event(&event) {
                                    if let Err(e) = storage.insert_session(&session) {
                                        tracing::warn!("Failed to persist session: {}", e);
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("Subscriber lagged, {} events dropped", n);
                            }
                            Err(e) => {
                                tracing::debug!("Event stream ended: {}", e);
                                break;
                            }
                        }
                    }

                    println!(
                        "Capture finished. {} events observed, {} sessions stored in {}",
                        session_count,
                        storage.list_sessions()?.len(),
                        db.display()
                    );
                }
                flowlens_cli::Commands::Export { format, query, out } => {
                    tracing::info!("Exporting {} to {:?} (query: {:?})", format, out, query);

                    let fmt = format.to_lowercase();
                    if fmt != "har" && fmt != "saz" {
                        anyhow::bail!(
                            "Unsupported export format '{}'. Supported: har, saz",
                            format
                        );
                    }

                    // Collect live traffic for a short window, then export.
                    let bus = Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());
                    let _listener = bus.subscribe(flowlens_core::models::TrafficFilter::default());

                    let mut engine = flowlens_capture::Engine::auto();
                    let using_real = matches!(engine, flowlens_capture::Engine::Pcap(_));
                    match engine.start() {
                        Ok(()) if using_real => {
                            tracing::info!("Real Npcap capture active for export");
                            flowlens_capture::spawn_engine_pump(engine, bus.clone());
                        }
                        _ => {
                            tracing::warn!("Npcap unavailable; exporting from mock producer");
                            flowlens_capture::spawn_mock_producer(bus.clone());
                        }
                    }

                    tokio::time::sleep(Duration::from_secs(3)).await;

                    let rows = bus.get_latest_events(5000);
                    if rows.is_empty() {
                        anyhow::bail!("No traffic captured to export");
                    }

                    match fmt.as_str() {
                        "har" => {
                            let content = flowlens_export::har::HarExporter::export_rows(&rows)?;
                            std::fs::write(&out, content)?;
                        }
                        "saz" => {
                            // SAZ is an import format; re-export the captured rows as HAR
                            // alongside the requested .saz path.
                            let content = flowlens_export::har::HarExporter::export_rows(&rows)?;
                            let saz_path = out.with_extension("saz.har");
                            std::fs::write(&saz_path, content)?;
                            tracing::info!("Wrote {}", saz_path.display());
                        }
                        _ => unreachable!(),
                    }

                    println!(
                        "Export complete: {:?} ({} entries)",
                        out,
                        rows.len()
                    );
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

/// Render a traffic event as a short human-readable line.
fn describe_event(event: &flowlens_core::models::TrafficEvent) -> String {
    use flowlens_core::models::TrafficEvent;
    match event {
        TrafficEvent::ConnectionEstablished { conn, process } => format!(
            "CONNECT {}:{} -> {}:{} (pid {} {})",
            conn.src_addr.ip(),
            conn.src_addr.port(),
            conn.dst_addr.ip(),
            conn.dst_addr.port(),
            process.pid,
            process.name
        ),
        TrafficEvent::ConnectionClosed { conn } => format!(
            "CLOSE   {}:{} -> {}:{}",
            conn.src_addr.ip(),
            conn.src_addr.port(),
            conn.dst_addr.ip(),
            conn.dst_addr.port()
        ),
        TrafficEvent::HttpRequest { request, .. } => {
            format!("REQ     {} {}", request.method, request.url)
        }
        TrafficEvent::HttpResponse { response, .. } => {
            format!("RESP    {} {}", response.status_code, response.status_text)
        }
        TrafficEvent::TlsHandshake { .. } => "TLS     handshake".to_string(),
        TrafficEvent::WebSocketFrame { frame, .. } => {
            format!("WS      {:?} ({} bytes)", frame.opcode, frame.payload.len())
        }
        TrafficEvent::SseEvent { event, .. } => format!("SSE     {}", event.data),
        TrafficEvent::GrpcCall { call, .. } => format!(
            "GRPC    {} {}",
            call.service,
            call.method
        ),
        TrafficEvent::PacketReceived { data, .. } => {
            format!("PACKET  {} bytes", data.len())
        }
        TrafficEvent::CaptureDropped { count, .. } => format!("DROPPED {}", count),
    }
}

/// Convert a traffic event into a storable session, when it carries enough
/// information to be worth persisting (HTTP exchanges and connection life).
fn session_from_event(
    event: &flowlens_core::models::TrafficEvent,
) -> Option<flowlens_core::models::TrafficSession> {
    use flowlens_core::models::*;

    let conn = match event {
        TrafficEvent::ConnectionEstablished { conn, .. }
        | TrafficEvent::ConnectionClosed { conn }
        | TrafficEvent::HttpRequest { conn, .. }
        | TrafficEvent::HttpResponse { conn, .. }
        | TrafficEvent::TlsHandshake { conn, .. }
        | TrafficEvent::WebSocketFrame { conn, .. }
        | TrafficEvent::SseEvent { conn, .. }
        | TrafficEvent::GrpcCall { conn, .. } => conn.clone(),
        _ => return None,
    };

    let process = match event {
        TrafficEvent::ConnectionEstablished { process, .. } => process.clone(),
        _ => ProcessInfo {
            pid: 0,
            name: String::new(),
            exe_path: None,
            user_sid: String::new(),
            arch: ProcessArch::Unknown,
            match_quality: MatchQuality::Unspecified,
        },
    };

    let (protocol, http_request, http_response) = match event {
        TrafficEvent::HttpRequest { request, .. } => (
            ProtocolKind::Http1,
            Some(request.clone()),
            None,
        ),
        TrafficEvent::HttpResponse { response, .. } => (
            ProtocolKind::Http1,
            None,
            Some(response.clone()),
        ),
        TrafficEvent::TlsHandshake { .. } => (ProtocolKind::Unknown, None, None),
        TrafficEvent::WebSocketFrame { .. } => (ProtocolKind::WebSocket, None, None),
        TrafficEvent::SseEvent { .. } => (ProtocolKind::Sse, None, None),
        TrafficEvent::GrpcCall { .. } => (ProtocolKind::Grpc, None, None),
        _ => (ProtocolKind::Unknown, None, None),
    };

    Some(TrafficSession {
        session_id: SessionId(0),
        conn,
        process,
        tls: None,
        protocol,
        http_request,
        http_response,
        websocket_frames: Vec::new(),
        sse_events: Vec::new(),
        grpc_call: None,
        started_at: 0,
        ended_at: None,
        bytes_sent: 0,
        bytes_received: 0,
    })
}
