//! FlowLens Example - Basic Traffic Capture
//!
//! This example demonstrates how to capture and analyze HTTP traffic.

use flowlens_capture::Engine;
use flowlens_protocol::Http1Parser;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Create capture engine - auto-detects Npcap or falls back to mock
    let _bus = std::sync::Arc::new(flowlens_core::event_bus::BroadcastEventBus::default());
    let mut engine = Engine::auto();

    tracing::info!("Starting capture engine...");
    engine.start()?;

    // Parse HTTP requests (example data)
    let parser = Http1Parser::new();
    let sample_request = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";

    if let Some(request) = parser.parse_http1_request(sample_request) {
        tracing::info!("Captured request: {} {}", request.method, request.path);
    }

    // Simulate packet capture loop
    for i in 0..5 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        tracing::info!("Processing packet {}", i);
    }

    engine.stop()?;
    tracing::info!("Capture completed");

    Ok(())
}
