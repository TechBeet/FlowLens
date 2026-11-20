//! WinDivert network layer capture engine for FlowLens
//!
//! Provides:
//! - Real WinDivert network layer capture (Phase 2: no-proxy HTTPS decryption)
//! - Packet filtering via BPF-like filter expressions
//! - Blocking receive with timeout for graceful shutdown
//! - Graceful shutdown via shutdown handle
//!
//! WinDivert intercepts packets at the Windows Filtering Platform (WFP) layer,
//! so HTTPS traffic can be redirected into the local MITM engine without
//! configuring a system-wide proxy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use flowlens_core::event_bus::EventBus;
use flowlens_core::models::{
    ConnectionKey, MatchQuality, ProcessArch, ProcessInfo, TrafficEvent, TransportProtocol,
};
use windivert::prelude::*;

use crate::pcap::parse_ipv4;

/// Default filter: outbound TCP traffic destined for HTTPS.
pub const DEFAULT_HTTPS_FILTER: &str = "outbound and tcp.DstPort == 443";

/// WinDivert network layer capture engine.
///
/// Owns a WinDivert handle opened on the network layer. Packets matching the
/// filter are received by [`WinDivertEngine::recv_packet`] and can be inspected
/// or re-injected with [`WinDivertEngine::send_packet`].
pub struct WinDivertEngine {
    handle: WinDivert<NetworkLayer>,
    filter: String,
    running: Arc<AtomicBool>,
    shutdown_handle: windivert::ShutdownHandle,
}

impl WinDivertEngine {
    /// Open a WinDivert handle on the network layer.
    ///
    /// `filter` uses WinDivert's BPF-like syntax, e.g.
    /// `"outbound and tcp.DstPort == 443"`.
    pub fn new(filter: &str, priority: i16, flags: WinDivertFlags) -> anyhow::Result<Self> {
        let handle = WinDivert::network(filter, priority, flags)?;
        let shutdown_handle = handle.shutdown_handle();
        Ok(Self {
            handle,
            filter: filter.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            shutdown_handle,
        })
    }

    /// Mark the engine as running.
    pub fn start(&self) -> anyhow::Result<()> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Stop the engine and shut down the WinDivert handle.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.shutdown_handle.shutdown();
    }

    /// Whether the engine is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// The filter expression this engine was opened with.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Receive one packet, waiting at most `timeout_ms` milliseconds.
    ///
    /// Returns `Ok(None)` when the timeout elapses without a packet.
    pub fn recv_packet<'a>(
        &self,
        buffer: &'a mut [u8],
        timeout_ms: u32,
    ) -> anyhow::Result<Option<WinDivertPacket<'a, NetworkLayer>>> {
        Ok(self.handle.recv_wait(buffer, timeout_ms)?)
    }

    /// Receive up to `max_packets` packets in a single syscall, waiting at most
    /// `timeout_ms` milliseconds.
    ///
    /// Returns an empty vector on timeout. This amortises the kernel/user
    /// transition cost under high packet rates.
    pub fn recv_batch<'a>(
        &self,
        buffer: &'a mut [u8],
        max_packets: u8,
        timeout_ms: u32,
    ) -> anyhow::Result<Vec<WinDivertPacket<'a, NetworkLayer>>> {
        Ok(self.handle.recv_wait_ex(buffer, max_packets, timeout_ms)?)
    }

    /// Re-inject a previously received packet.
    pub fn send_packet(&self, packet: &WinDivertPacket<NetworkLayer>) -> anyhow::Result<u32> {
        Ok(self.handle.send(packet)?)
    }
}

/// Extract a [`ConnectionKey`] from a captured WinDivert packet.
///
/// WinDivert's network layer delivers bare IP packets (no Ethernet header), so
/// the payload is parsed directly as IPv4.
pub fn extract_connection_info_from(
    pkt: &WinDivertPacket<NetworkLayer>,
    id: u64,
) -> Option<ConnectionKey> {
    let (src_ip, sport, dst_ip, dport, proto_byte) = parse_ipv4(&pkt.data)?;
    let protocol = if proto_byte == 6 {
        TransportProtocol::Tcp
    } else {
        TransportProtocol::Udp
    };
    Some(ConnectionKey {
        src_addr: std::net::SocketAddr::new(src_ip, sport),
        dst_addr: std::net::SocketAddr::new(dst_ip, dport),
        protocol,
        connection_id: id,
        timestamp: pkt.address.event_timestamp(),
        interface_guid: None,
    })
}

/// Spawn a background thread that receives packets and publishes
/// `ConnectionEstablished` events onto the bus.
///
/// The thread exits when the engine is stopped or the handle is shut down.
///
/// Packets are received in batches via `recv_wait_ex` (up to 64 per syscall)
/// to amortise the kernel/user transition cost under high load.
pub fn spawn_capture(eng: WinDivertEngine, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
    let running = eng.running.clone();
    std::thread::spawn(move || {
        // 64 packets × ~1500 bytes, plus headroom for jumbo frames.
        let mut buffer = [0u8; 64 * 2048];
        let mut id_counter: u64 = 0;

        while running.load(Ordering::SeqCst) {
            match eng.recv_batch(&mut buffer, 64, 100) {
                Ok(packets) => {
                    for packet in packets {
                        if let Some(conn) = extract_connection_info_from(&packet, id_counter) {
                            let _ = bus.publish(TrafficEvent::ConnectionEstablished {
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
                        // Re-inject so the packet still reaches its destination.
                        if let Err(e) = eng.send_packet(&packet) {
                            tracing::warn!(err = ?e, "WinDivert re-inject failed");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(err = ?e, "WinDivert recv error");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
        tracing::info!("WinDivert capture loop exited");
    });
    Ok(())
}

/// Create a started engine that captures outbound HTTPS traffic.
pub fn create_https_engine() -> anyhow::Result<WinDivertEngine> {
    let engine = WinDivertEngine::new(DEFAULT_HTTPS_FILTER, 0, WinDivertFlags::default())?;
    engine.start()?;
    Ok(engine)
}

/// Create a started engine that captures all IP traffic.
pub fn create_all_ip_engine() -> anyhow::Result<WinDivertEngine> {
    let engine = WinDivertEngine::new("ip", 0, WinDivertFlags::default())?;
    engine.start()?;
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default HTTPS filter must target outbound TCP port 443.
    #[test]
    fn test_default_https_filter_targets_port_443() {
        assert!(DEFAULT_HTTPS_FILTER.contains("tcp.DstPort == 443"));
        assert!(DEFAULT_HTTPS_FILTER.contains("outbound"));
    }

    /// A short/zeroed buffer is not a valid IPv4 header, so parsing must fail.
    ///
    /// This exercises the pure parsing path without opening a WinDivert handle
    /// (opening a handle requires administrator rights and the driver).
    #[test]
    fn test_parse_ipv4_rejects_invalid_payload() {
        assert!(parse_ipv4(&[0u8; 16]).is_none());
        assert!(parse_ipv4(&[]).is_none());
    }

    /// A well-formed IPv4/TCP header must yield the expected connection tuple.
    #[test]
    fn test_parse_ipv4_extracts_tcp_tuple() {
        let mut pkt = vec![0x45, 0x00, 0x00, 0x28, 0, 1, 0, 0, 64, 6, 0, 0];
        pkt.extend_from_slice(&[192, 168, 1, 10]);
        pkt.extend_from_slice(&[93, 184, 216, 34]);
        pkt.extend_from_slice(&[0x30, 0x39, 0x01, 0xbb, 0, 0, 0, 0, 80, 0x50, 0, 0]);

        let (src, sport, dst, dport, proto) = parse_ipv4(&pkt).expect("valid IPv4/TCP header");
        assert_eq!(proto, 6);
        assert_eq!(sport, 12345);
        assert_eq!(dport, 443);
        assert_eq!(src.to_string(), "192.168.1.10");
        assert_eq!(dst.to_string(), "93.184.216.34");
    }
}
