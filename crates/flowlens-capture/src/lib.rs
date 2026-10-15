//! Capture engine for FlowLens
//!
//! Provides:
//! - Real Npcap capture via the `pcap` crate (Phase 1+), falling back to mock
//! - MockEngine for tests / systems without Npcap
//! - RawPacket abstraction and packet parsing helpers

pub mod pcap;

pub use pcap::{LinkType, PacketDirection, PcapEngine, RawPacket};

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use flowlens_core::models::{
    ConnectionKey, MatchQuality, ProcessArch, ProcessInfo, TrafficEvent, TransportProtocol,
};
use flowlens_core::event_bus::EventBus;

/// Unified capture engine that auto-selects pcap or mock
pub enum Engine {
    Pcap(PcapEngine),
    Mock(MockEngine),
}

impl Engine {
    /// Auto-detect: use real Npcap if a device is available, else mock
    pub fn auto() -> Self {
        match PcapEngine::pick_device() {
            Some(device) => {
                tracing::info!("Using real Npcap capture on device: {}", device);
                Engine::Pcap(PcapEngine::new(device))
            }
            None => {
                tracing::warn!("No Npcap device found, falling back to mock engine");
                Engine::Mock(MockEngine::new())
            }
        }
    }

    pub fn new_pcap(device: impl Into<String>) -> Self {
        Engine::Pcap(PcapEngine::new(device))
    }

    pub fn new_mock() -> Self {
        Engine::Mock(MockEngine::new())
    }

    fn running_flag(&self) -> &Arc<AtomicBool> {
        match self {
            Engine::Pcap(p) => &p.running,
            Engine::Mock(m) => &m.running,
        }
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        match self {
            Engine::Pcap(p) => p.start(),
            Engine::Mock(m) => m.start(),
        }
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        match self {
            Engine::Pcap(p) => p.stop(),
            Engine::Mock(m) => m.stop(),
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            Engine::Pcap(p) => p.is_running(),
            Engine::Mock(m) => m.is_running(),
        }
    }

    pub fn drain_packets(&self, limit: usize) -> Vec<RawPacket> {
        match self {
            Engine::Pcap(p) => p.drain_packets(limit),
            Engine::Mock(m) => m.drain_packets(limit),
        }
    }
}

/// Test/mock engine used when Npcap is unavailable or in unit tests
pub struct MockEngine {
    pub(crate) running: Arc<AtomicBool>,
    queue: Arc<std::sync::Mutex<std::collections::VecDeque<RawPacket>>>,
    next_id: std::sync::atomic::AtomicU64,
}

unsafe impl Send for MockEngine {}
unsafe impl Sync for MockEngine {}

impl Default for MockEngine {
    fn default() -> Self { Self::new() }
}

impl MockEngine {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            queue: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            next_id: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn take_id(&self) -> u64 {
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub fn synthesize_event(&self, n: u64) -> TrafficEvent {
        let conn = ConnectionKey {
            src_addr: format!("192.168.1.{}:{}", n % 250 + 1, 40000 + n)
                .parse().unwrap_or_else(|_| "127.0.0.1:0".parse().unwrap()),
            dst_addr: format!("93.184.216.{:}:443", n % 20 + 1)
                .parse().unwrap_or_else(|_| "127.0.0.1:443".parse().unwrap()),
            protocol: if n % 3 == 0 { TransportProtocol::Udp } else { TransportProtocol::Tcp },
            connection_id: n,
            timestamp: 0,
            interface_guid: None,
        };
        let kind = n % 6;
        let process = ProcessInfo {
            pid: (n % 3000 + 100) as u32,
            name: format!("process_{}.exe", n % 10),
            exe_path: Some(format!(r"C:\Windows\System32\proc{}.exe", n % 10).into()),
            user_sid: "S-1-5-21-1234567890-1001".to_string(),
            arch: ProcessArch::X64,
            match_quality: MatchQuality::Heuristic,
        };
        match kind {
            0 => TrafficEvent::ConnectionEstablished { conn, process },
            1 => TrafficEvent::ConnectionClosed { conn },
            2 => TrafficEvent::CaptureDropped {
                reason: "mock".to_string(),
                count: 1,
            },
            3 => TrafficEvent::HttpRequest {
                session_id: flowlens_core::models::SessionId(0),
                conn,
                request: flowlens_core::models::HttpRequest {
                    method: "GET".to_string(),
                    url: "/index.html".to_string(),
                    path: "/index.html".to_string(),
                    request_line_version: flowlens_core::models::HttpVersion::Http11,
                    headers: vec![("Host".to_string(), "example.com".to_string())],
                    body: None,
                    body_size: 0,
                    timestamp: 0,
                },
            },
            4 => TrafficEvent::HttpResponse {
                session_id: flowlens_core::models::SessionId(0),
                conn,
                response: flowlens_core::models::HttpResponse {
                    status_code: 200,
                    status_text: "OK".to_string(),
                    version: flowlens_core::models::HttpVersion::Http11,
                    headers: vec![("Content-Type".to_string(), "text/html".to_string())],
                    body: None,
                    body_size: 1024,
                    timestamp: 0,
                },
            },
            _ => TrafficEvent::ConnectionEstablished { conn, process },
        }
    }

    fn now_micros() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as i64)
            .unwrap_or(0)
    }

    pub fn drain_packets(&self, limit: usize) -> Vec<RawPacket> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.queue.lock() {
            while let Some(p) = q.pop_front() {
                out.push(p);
                if out.len() >= limit { break; }
            }
        }
        out
    }
}

/// Capture engine trait (kept for compatibility)
#[allow(async_fn_in_trait)]
pub trait CaptureEngine {
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
    fn is_running(&self) -> bool;
    async fn receive_packet(&mut self) -> Option<RawPacket>;
}

impl CaptureEngine for MockEngine {
    fn start(&mut self) -> anyhow::Result<()> {
        self.running.store(true, std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Mock capture engine started");
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Mock capture engine stopped");
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn receive_packet(&mut self) -> Option<RawPacket> {
        if !self.is_running() { return None; }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let n = self.take_id();
        Some(RawPacket {
            timestamp: MockEngine::now_micros(),
            interface_name: format!("mock-{}", n),
            link_type: LinkType::Loopback,
            data: format!("mock-packet-{}", n).into_bytes(),
            direction: PacketDirection::Outbound,
            src_mac: None,
            dst_mac: None,
        })
    }
}

/// Parse a raw packet to extract connection info
pub fn extract_connection_info_from(pkt: &RawPacket, id: u64) -> Option<ConnectionKey> {
    let ip_payload: &[u8] = if pkt.link_type == LinkType::Ethernet && pkt.data.len() > 14 {
        &pkt.data[14..]
    } else {
        &pkt.data
    };
    let (src_ip, sport, dst_ip, dport, proto_byte) = pcap::parse_ipv4(ip_payload)?;
    let src_addr = std::net::SocketAddr::new(src_ip, sport);
    let dst_addr = std::net::SocketAddr::new(dst_ip, dport);
    let protocol = if proto_byte == 6 { TransportProtocol::Tcp } else { TransportProtocol::Udp };
    Some(ConnectionKey {
        src_addr,
        dst_addr,
        protocol,
        connection_id: id,
        timestamp: pkt.timestamp,
        interface_guid: None,
    })
}


/// Start a capture loop in a background task and publish TrafficEvents onto the bus.
pub fn spawn_capture(mut eng: Engine, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
    eng.start()?;
    let bus_clone = bus.clone();
    let running = eng.running_flag().clone();
    std::thread::spawn(move || {
        let mut id_counter: u64 = 0;
        while running.load(std::sync::atomic::Ordering::SeqCst) {
            let packets = eng.drain_packets(64);
            for pkt in packets {
                if let Some(conn) = extract_connection_info_from(&pkt, id_counter) {
                    let _ = bus_clone.publish(TrafficEvent::ConnectionEstablished {
                        conn,
                        process: ProcessInfo {
                            pid: 0,
                            name: "unknown".to_string(),
                            exe_path: None,
                            user_sid: String::new(),
                            arch: ProcessArch::Unknown,
                            match_quality: MatchQuality::Unspecified,
                        },
                    });
                    id_counter += 1;
                }
                let _ = bus_clone.publish(TrafficEvent::PacketReceived {
                    data: pkt.data.clone(),
                    interface: pkt.interface_name.clone(),
                    direction: format!("{:?}", pkt.direction),
                    timestamp: pkt.timestamp,
                });
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        tracing::info!("Capture loop exited");
    });
    Ok(())
}

/// Background task that fabricates events onto an EventBus (for tests/demo).
pub fn spawn_mock_producer(bus: Arc<dyn EventBus>) {
    tokio::spawn(async move {
        let engine = MockEngine::new();
        let mut counter: u64 = 0;
        tracing::info!("Mock event producer started");
        loop {
            let event = engine.synthesize_event(counter);
            let _ = bus.publish(event);
            counter += 1;
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_start_stop() {
        let mut eng = Engine::new_mock();
        assert!(!eng.is_running());
        eng.start().unwrap();
        assert!(eng.is_running());
        eng.stop().unwrap();
        assert!(!eng.is_running());
    }

    #[test]
    fn test_mock_engine_synthesize() {
        let engine = MockEngine::new();
        let ev = engine.synthesize_event(0);
        assert!(matches!(ev, TrafficEvent::ConnectionEstablished { .. }));
    }

    #[test]
    fn test_extract_connection_info_from_minimal() {
        let pkt = RawPacket {
            timestamp: 12345,
            interface_name: String::new(),
            link_type: LinkType::Raw,
            data: vec![0u8; 10],
            direction: PacketDirection::Outbound,
            src_mac: None,
            dst_mac: None,
        };
        assert!(extract_connection_info_from(&pkt, 0).is_none());
    }
}

