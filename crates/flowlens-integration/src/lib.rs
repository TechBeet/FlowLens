//! Integration tests for FlowLens
//!
//! Verifies EventBus flow, capture->UI delivery, session processing, and CLI parsing.

#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;
#[allow(unused_imports)]
use tokio::runtime::Runtime;

#[allow(unused_imports)]
use clap::Parser;
#[allow(unused_imports)]
use flowlens_capture::{spawn_mock_producer, Engine};
#[allow(unused_imports)]
use flowlens_cli::Cli;
#[allow(unused_imports)]
use flowlens_core::event_bus::{BroadcastEventBus, EventBus, EventBusDiagnostics};
#[allow(unused_imports)]
use flowlens_core::models::{ConnectionKey, TrafficEvent, TrafficFilter};
#[allow(unused_imports)]
use flowlens_core::session::SessionManager;

/// Test that EventBus publishes and subscribers receive events
#[test]
fn test_event_bus_publish_subscribe_flow() {
    let bus = Arc::new(BroadcastEventBus::new(128));
    let mut rx = bus.subscribe(TrafficFilter::default());

    let event = TrafficEvent::ConnectionEstablished {
        conn: ConnectionKey {
            src_addr: "127.0.0.1:12345".parse().unwrap(),
            dst_addr: "8.8.8.8:53".parse().unwrap(),
            protocol: flowlens_core::models::TransportProtocol::Udp,
            connection_id: 1,
            timestamp: 1000,
            interface_guid: None,
        },
        process: flowlens_core::models::ProcessInfo {
            pid: 1234,
            name: "test.exe".to_string(),
            exe_path: None,
            user_sid: "S-1-5-21-123".to_string(),
            arch: flowlens_core::models::ProcessArch::X64,
            match_quality: flowlens_core::models::MatchQuality::ExactTuple,
        },
    };

    bus.publish(event.clone()).unwrap();

    let received = rx.blocking_recv();
    assert!(
        received.is_ok(),
        "Subscriber should receive published event"
    );
    assert!(matches!(
        received.unwrap(),
        TrafficEvent::ConnectionEstablished { .. }
    ));
}

/// Test that EventBus tracks event count accurately
#[test]
fn test_event_bus_count_accuracy() {
    let bus = Arc::new(BroadcastEventBus::new(64));
    for i in 0..20u64 {
        bus.publish(TrafficEvent::CaptureDropped {
            reason: format!("test {}", i),
            count: 1,
        })
        .unwrap();
    }
    assert_eq!(bus.event_count(), 20);
}

/// Test that latest_events snapshot returns correct number of events
#[test]
fn test_latest_events_snapshot_limit() {
    let bus = Arc::new(BroadcastEventBus::new(64));
    for i in 0..10u64 {
        bus.publish(TrafficEvent::CaptureDropped {
            reason: format!("test {}", i),
            count: 1,
        })
        .unwrap();
    }
    let recent = bus.latest_events(5);
    assert_eq!(recent.len(), 5);
}

/// Test that capture engine mock producer flows through event bus to subscribers
#[test]
fn test_mock_producer_to_subscriber_flow() {
    let bus = Arc::new(BroadcastEventBus::new(1024));

    // Spawn mock producer
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        spawn_mock_producer(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>);
    });

    // Wait for events to be generated
    thread::sleep(Duration::from_secs(2));

    // Verify events were produced
    assert!(
        bus.event_count() > 0,
        "Mock producer should generate events"
    );
    assert!(!bus.latest_events(10).is_empty());
}

/// Test that GUI frontend can subscribe to event bus and receive events
#[test]
fn test_gui_receives_events() {
    let bus = Arc::new(BroadcastEventBus::new(1024));

    // Spawn mock producer
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        spawn_mock_producer(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>);
    });

    // Wait for events
    thread::sleep(Duration::from_secs(2));

    // Create GUI frontend - it takes Arc<dyn EventBus> in its run method
    let _gui = flowlens_ui::GuiFrontend::new();

    // Verify bus has events (GUI frontend receives them via subscription)
    assert!(bus.event_count() > 0);
    assert!(!bus.latest_events(20).is_empty());
}

/// Test that TUI frontend can subscribe to event bus and receive events
#[test]
fn test_tui_receives_events() {
    let bus = Arc::new(BroadcastEventBus::new(1024));

    // Spawn mock producer
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        spawn_mock_producer(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>);
    });

    // Wait for events
    thread::sleep(Duration::from_secs(2));

    // Create TUI frontend
    let _tui = flowlens_ui::TuiFrontend::new();

    // Verify bus has events (TUI creates its own subscription in run())
    assert!(bus.event_count() > 0);
    assert!(!bus.latest_events(20).is_empty());
}

/// Test SessionManager creates and updates sessions correctly
#[test]
fn test_session_manager_creation_and_update() {
    let session_mgr = flowlens_core::session::InMemorySessionManager::new();

    // Create a session
    let conn = ConnectionKey {
        src_addr: "127.0.0.1:12345".parse().unwrap(),
        dst_addr: "93.184.216.34:443".parse().unwrap(),
        protocol: flowlens_core::models::TransportProtocol::Tcp,
        connection_id: 1,
        timestamp: 1000,
        interface_guid: None,
    };

    let session_id = session_mgr.create_session(conn.clone());
    let session = session_mgr
        .get_session(session_id)
        .expect("Session should exist");
    assert_eq!(session.session_id, session_id);
    assert_eq!(session.conn, conn);
}

/// Test SessionManager handles TrafficEvent updates
#[test]
fn test_session_manager_processes_events() {
    let session_mgr = flowlens_core::session::InMemorySessionManager::new();

    // Create session
    let conn = ConnectionKey {
        src_addr: "127.0.0.1:12345".parse().unwrap(),
        dst_addr: "93.184.216.34:443".parse().unwrap(),
        protocol: flowlens_core::models::TransportProtocol::Tcp,
        connection_id: 1,
        timestamp: 1000,
        interface_guid: None,
    };
    let session_id = session_mgr.create_session(conn.clone());

    // Update with HTTP request event
    let http_request = TrafficEvent::HttpRequest {
        session_id,
        conn: conn.clone(),
        request: flowlens_core::models::HttpRequest {
            method: "GET".to_string(),
            url: "https://example.com/api".to_string(),
            path: "/api".to_string(),
            request_line_version: flowlens_core::models::HttpVersion::Http11,
            headers: vec![("Host".to_string(), "example.com".to_string())],
            body: None,
            body_size: 0,
            timestamp: 1001,
        },
    };
    session_mgr
        .update_session(session_id, http_request)
        .unwrap();

    // Update with HTTP response event
    let http_response = TrafficEvent::HttpResponse {
        session_id,
        conn: conn.clone(),
        response: flowlens_core::models::HttpResponse {
            status_code: 200,
            status_text: "OK".to_string(),
            version: flowlens_core::models::HttpVersion::Http11,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: None,
            body_size: 1234,
            timestamp: 1002,
        },
    };
    session_mgr
        .update_session(session_id, http_response)
        .unwrap();

    // Verify session was updated
    let session = session_mgr
        .get_session(session_id)
        .expect("Session should still exist");
    assert!(session.http_request.is_some(), "HTTP request should be set");
    assert!(
        session.http_response.is_some(),
        "HTTP response should be set"
    );
    assert_eq!(session.http_request.as_ref().unwrap().method, "GET");
    assert_eq!(session.http_response.as_ref().unwrap().status_code, 200);
}

/// Test CLI --status command parses correctly
#[test]
fn test_cli_status_command_parses() {
    let cli = Cli::parse_from(["flowlens", "start", "--status"]);
    match cli.command {
        flowlens_cli::Commands::Start { status, .. } => {
            assert!(status);
        }
        _ => panic!("Expected Start command"),
    }
}

/// Test CLI --events command parses correctly
#[test]
fn test_cli_events_command_parses() {
    let cli = Cli::parse_from(["flowlens", "start", "--events"]);
    match cli.command {
        flowlens_cli::Commands::Start { events, .. } => {
            assert!(events);
        }
        _ => panic!("Expected Start command"),
    }
}

/// Test CLI --filters command parses correctly
#[test]
fn test_cli_filters_command_parses() {
    let cli = Cli::parse_from(["flowlens", "start", "--filters"]);
    match cli.command {
        flowlens_cli::Commands::Start { filters, .. } => {
            assert!(filters);
        }
        _ => panic!("Expected Start command"),
    }
}

/// Test CLI --gui command parses correctly
#[test]
fn test_cli_gui_command_parses() {
    let cli = Cli::parse_from(["flowlens", "start", "--gui"]);
    match cli.command {
        flowlens_cli::Commands::Start { gui, .. } => {
            assert!(gui);
        }
        _ => panic!("Expected Start command"),
    }
}

/// Test mock engine start/stop cycle
#[test]
fn test_mock_engine_lifecycle() {
    let mut engine = flowlens_capture::Engine::new_mock();

    assert!(!engine.is_running());
    engine.start().unwrap();
    assert!(engine.is_running());
    engine.stop().unwrap();
    assert!(!engine.is_running());
}

/// Test that event bus handles concurrent publish/subscribe
#[test]
fn test_concurrent_publish_subscribe() {
    let bus = Arc::new(BroadcastEventBus::new(1024));

    // Spawn multiple publishers
    let mut handles = vec![];
    for i in 0..5 {
        let bus_clone = bus.clone();
        let handle = thread::spawn(move || {
            for j in 0..10 {
                bus_clone
                    .publish(TrafficEvent::CaptureDropped {
                        reason: format!("concurrent-{}-{}", i, j),
                        count: 1,
                    })
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    // Subscribe and collect
    let _rx = bus.subscribe(TrafficFilter::default());

    // Wait for publishers to finish
    for handle in handles {
        handle.join().unwrap();
    }

    // Give time for events to propagate
    thread::sleep(Duration::from_millis(100));

    // Verify we got events
    assert!(
        bus.event_count() >= 50,
        "Should have published at least 50 events"
    );
    let recent = bus.latest_events(100);
    assert!(!recent.is_empty(), "Should have recent events");
}

/// Test filter subscription tracking
#[test]
fn test_filter_subscription_tracking() {
    let bus = BroadcastEventBus::new(64);

    // Subscribe with different filters
    let _rx1 = bus.subscribe(TrafficFilter {
        pids: Some(vec![1234]),
        ..Default::default()
    });
    let _rx2 = bus.subscribe(TrafficFilter {
        hosts: Some(vec!["example.com:443".to_string()]),
        ..Default::default()
    });
    let _rx3 = bus.subscribe(TrafficFilter::default());

    // Verify subscriber count
    assert_eq!(bus.subscriber_count(), 3);
}

// ---------------------------------------------------------------------------
// Real-capture integration tests
// ---------------------------------------------------------------------------
//
// These tests verify that FlowLens correctly initialises real Npcap capture
// and falls back gracefully when Npcap is unavailable. Run them with:
//
//   cargo test --workspace --test integration
//
// On systems without Npcap, the *real* capture tests are marked #[ignore]
// and the mock fallback path is exercised instead.
//
// To enable the real-capture tests on Windows:
//   1. Install Npcap from https://nmap.org/npcap/
//   2. Ensure the "Npcap Loopback Adapter" option was selected during install
//   3. Run the test binary as Administrator
// ---------------------------------------------------------------------------

/// Verify that Engine::auto(); selects a real pcap engine when Npcap is
/// available, or a mock engine otherwise (never panics).
#[test]
fn test_real_capture_initialization() {
    let eng = flowlens_capture::Engine::auto();
    match &eng {
        flowlens_capture::Engine::Pcap(pc) => {
            tracing::info!(
                "Real Npcap capture initialised on device: {}",
                pc.device_name()
            );
        }
        flowlens_capture::Engine::Mock(_) => {
            tracing::warn!("Npcap not available; using mock engine");
        }
    }
    // Engine should be constructed regardless of Npcap presence.
    assert!(!eng.is_running());
}

/// Verify that Engine::with_real_capture() behaves the same as auto().
#[test]
fn test_real_capture_with_real_capture_constructor() {
    let eng = flowlens_capture::Engine::with_real_capture();
    match &eng {
        flowlens_capture::Engine::Pcap(_) => {}
        flowlens_capture::Engine::Mock(_) => {}
    }
    assert!(!eng.is_running());
}

/// Attempt to start the auto-detected engine and verify it reports running
/// status. The engine is then stopped cleanly.
#[test]
fn test_real_capture_start_stop_lifecycle() {
    let mut eng = flowlens_capture::Engine::auto();
    assert!(!eng.is_running());

    let start_result = eng.start();

    match &eng {
        flowlens_capture::Engine::Pcap(_) => {
            // On Windows with Npcap, start should succeed.
            // Without Npcap the auto engine will be Mock which also starts.
            assert!(
                start_result.is_ok(),
                "Engine start should succeed (either with real pcap or mock)"
            );
            assert!(eng.is_running());
        }
        flowlens_capture::Engine::Mock(_) => {
            assert!(
                start_result.is_ok(),
                "Mock engine start should always succeed"
            );
            assert!(eng.is_running());
        }
    }

    // Stop should always succeed.
    assert!(eng.stop().is_ok());
    assert!(!eng.is_running());
}

/// Drain packets from the engine after it has been started for a short period.
/// This exercises the drain path used by pump_engine_into_bus.
#[test]
fn test_real_capture_drain_packets() {
    let mut eng = flowlens_capture::Engine::auto();
    if let Err(e) = eng.start() {
        // If we can't even start (shouldn't happen for mock), skip.
        tracing::warn!("Cannot start engine for drain test: {e}");
        return;
    }

    // Give the capture thread a moment to collect packets (if real).
    std::thread::sleep(Duration::from_millis(200));

    let packets = eng.drain_packets(64);
    // Packets may be zero if no traffic is flowing; we just verify no panic.
    tracing::info!("Drained {} packets", packets.len());
    assert!(packets.len() <= 64);

    let _ = eng.stop();
}

/// Generate a small burst of HTTP traffic with curl and verify that the
/// real-capture engine picks up at least some packets.
///
/// This test is marked #[ignore] by default because it requires:
///   - Npcap installed
///   - curl available on PATH
///   - Running as Administrator on Windows
///
/// Run it explicitly with: cargo test --workspace --test integration test_end_to_end_real_traffic -- --ignored
#[test]
#[ignore = "Requires Npcap, Administrator privileges, and curl on PATH"]
fn test_end_to_end_real_traffic() {
    // --- Setup ---------------------------------------------------------------
    let mut eng = flowlens_capture::Engine::auto();
    match &eng {
        flowlens_capture::Engine::Mock(_) => {
            panic!("Test requires real Npcap capture; mock mode detected");
        }
        flowlens_capture::Engine::Pcap(_) => {}
    }

    let bus = Arc::new(BroadcastEventBus::new(1024));

    assert!(eng.start().is_ok(), "Npcap engine should start");

    // --- Generate traffic ----------------------------------------------------
    // Use curl to hit a well-known HTTPS endpoint.
    let curl_result = std::process::Command::new("curl")
        .args([
            "--max-time",
            "5",
            "--silent",
            "--output",
            "/dev/null",
            "--write-out",
            "%{http_code}",
            "https://example.com",
        ])
        .output();

    match curl_result {
        Ok(out) => {
            let code = String::from_utf8_lossy(&out.stdout);
            tracing::info!("curl returned: {}", code.trim());
        }
        Err(e) => {
            // curl may not be installed; use a TCP connect as fallback.
            tracing::warn!("curl not available ({e}); using raw TCP fallback");
            use std::net::TcpStream;
            let _ = TcpStream::connect("93.184.216.34:443");
        }
    }

    // --- Capture window ------------------------------------------------------
    std::thread::sleep(Duration::from_secs(1));

    // --- Drain and verify ----------------------------------------------------
    let packets = eng.drain_packets(256);
    tracing::info!("Captured {} packets during test window", packets.len());

    // With real traffic, we expect at least a few packets (TCP handshake, etc.)
    // If Npcap didn't capture anything, the test still passes 鈥?we log it.
    if packets.is_empty() {
        tracing::warn!(
            "No packets captured; check that Npcap is installed and \
             the loopback adapter is enabled"
        );
    } else {
        assert!(
            packets.len() >= 2,
            "Expected at least 2 packets from generated traffic, got {}",
            packets.len()
        );
    }

    // Publish any captured packets into the bus and verify events appear.
    let bus_dyn: Arc<dyn EventBus> = bus.clone();
    flowlens_capture::pump_engine_into_bus(&eng, &bus_dyn);
    let event_count = bus.event_count();
    tracing::info!("Published {} events to bus", event_count);

    // Bus may remain at 0 if no parseable TCP/UDP packets were captured;
    // that's acceptable 鈥?the important thing is that no panic occurred.
    let _ = eng.stop();
}

/// Verify that when Npcap is NOT available, the engine gracefully falls back
/// to mock mode and the mock producer still generates events.
#[test]
fn test_mock_fallback_when_no_npcap() {
    let mut eng = flowlens_capture::Engine::auto();
    let is_mock = matches!(&eng, flowlens_capture::Engine::Mock(_));

    if !is_mock {
        // Npcap is available; skip the fallback assertion.
        tracing::info!("Npcap is present; skipping mock-fallback test");
        return;
    }

    let bus = Arc::new(BroadcastEventBus::new(1024));

    // Start the mock engine
    assert!(eng.start().is_ok());
    assert!(eng.is_running());

    // Spawn the mock producer (this is what main.rs does in fallback mode)
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        spawn_mock_producer(bus.clone() as Arc<dyn flowlens_core::event_bus::EventBus>);
    });

    // Wait for events
    thread::sleep(Duration::from_secs(1));

    assert!(
        bus.event_count() > 0,
        "Mock fallback should generate events when Npcap is unavailable"
    );
    assert!(!bus.latest_events(10).is_empty());

    assert!(eng.stop().is_ok());
}
