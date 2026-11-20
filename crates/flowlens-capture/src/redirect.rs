//! Transparent TCP redirect for WinDivert (no-proxy HTTPS interception).
//!
//! This module implements the packet-level NAT that makes HTTPS decryption work
//! **without** configuring a system proxy:
//!
//! 1. Outbound TCP packets destined for port 443 are rewritten so their
//!    destination becomes the local MITM listener (`127.0.0.1:<mitm_port>`).
//!    The original destination is remembered per client source port.
//! 2. The MITM listener terminates TLS with a per-host leaf certificate and
//!    forwards the plaintext to the real upstream server.
//! 3. Response packets emitted by the MITM listener (source port == mitm_port)
//!    are rewritten so their source becomes the original server address, which
//!    keeps the client's TCP state machine consistent.
//!
//! Packets that do not match either rule are re-injected unchanged, so all other
//! traffic is unaffected.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windivert::prelude::*;
use windivert_sys::ChecksumFlags;

/// Loopback address used as the redirect target.
const LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

/// Offset of the IPv4 destination address inside a bare IP packet.
const IPV4_DST_OFFSET: usize = 16;
/// Offset of the IPv4 source address inside a bare IP packet.
const IPV4_SRC_OFFSET: usize = 12;
/// Offset of the IPv4 protocol byte.
const IPV4_PROTO_OFFSET: usize = 9;

/// Per-flow redirect table: client source port -> original server address.
pub type FlowTable = HashMap<u16, SocketAddrV4>;

/// Rewrite a bare IPv4/TCP packet in place according to the redirect rules.
///
/// This is the pure, handle-free core of the redirect engine so it can be unit
/// tested without opening a WinDivert driver handle.
///
/// Returns `true` when the packet was modified.
pub fn rewrite_packet(data: &mut [u8], mitm_port: u16, flows: &mut FlowTable) -> bool {
    if data.len() < 20 || (data[0] >> 4) != 4 || data[IPV4_PROTO_OFFSET] != 6 {
        return false;
    }
    let ihl = ((data[0] & 0x0f) as usize) * 4;
    if data.len() < ihl + 4 {
        return false;
    }

    let src_ip = Ipv4Addr::new(data[12], data[13], data[14], data[15]);
    let dst_ip = Ipv4Addr::new(data[16], data[17], data[18], data[19]);
    let src_port = u16::from_be_bytes([data[ihl], data[ihl + 1]]);
    let dst_port = u16::from_be_bytes([data[ihl + 2], data[ihl + 3]]);

    // ── Outbound client → server: redirect to the MITM listener ──────────
    if dst_port == 443 && src_ip != LOOPBACK {
        flows.insert(src_port, SocketAddrV4::new(dst_ip, dst_port));
        data[IPV4_DST_OFFSET..IPV4_DST_OFFSET + 4].copy_from_slice(&LOOPBACK.octets());
        data[ihl + 2..ihl + 4].copy_from_slice(&mitm_port.to_be_bytes());
        return true;
    }

    // ── MITM listener → client: restore the original server address ──────
    if src_port == mitm_port && dst_ip != LOOPBACK {
        if let Some(server) = flows.get(&dst_port).copied() {
            data[IPV4_SRC_OFFSET..IPV4_SRC_OFFSET + 4].copy_from_slice(&server.ip().octets());
            data[ihl..ihl + 2].copy_from_slice(&server.port().to_be_bytes());
            return true;
        }
    }

    false
}

/// A transparent redirect engine that funnels outbound HTTPS into a local MITM
/// listener and rewrites the responses back to the original server address.
pub struct RedirectEngine {
    handle: WinDivert<NetworkLayer>,
    mitm_port: u16,
    flows: Arc<Mutex<FlowTable>>,
    running: Arc<AtomicBool>,
    shutdown_handle: windivert::ShutdownHandle,
}

impl RedirectEngine {
    /// Open a redirect engine that sends outbound port-443 traffic to
    /// `127.0.0.1:<mitm_port>`.
    pub fn new(mitm_port: u16) -> anyhow::Result<Self> {
        // Capture outbound HTTPS plus the MITM listener's own responses.
        let filter = format!("tcp and (tcp.DstPort == 443 or tcp.SrcPort == {mitm_port})");
        let handle = WinDivert::network(&filter, 0, WinDivertFlags::default())?;
        let shutdown_handle = handle.shutdown_handle();
        Ok(Self {
            handle,
            mitm_port,
            flows: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            shutdown_handle,
        })
    }

    /// The local port the MITM listener must bind to.
    pub fn mitm_port(&self) -> u16 {
        self.mitm_port
    }

    /// Number of tracked client flows (diagnostics / tests).
    pub fn tracked_flows(&self) -> usize {
        self.flows.lock().map(|f| f.len()).unwrap_or(0)
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

    /// Whether the engine is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Receive one packet with a timeout.
    pub fn recv_packet<'a>(
        &self,
        buffer: &'a mut [u8],
        timeout_ms: u32,
    ) -> anyhow::Result<Option<WinDivertPacket<'a, NetworkLayer>>> {
        Ok(self.handle.recv_wait(buffer, timeout_ms)?)
    }

    /// Re-inject a (possibly rewritten) packet.
    pub fn send_packet(&self, packet: &WinDivertPacket<NetworkLayer>) -> anyhow::Result<u32> {
        Ok(self.handle.send(packet)?)
    }

    /// Rewrite a packet in place according to the redirect rules.
    ///
    /// Returns `true` when the packet was modified. The caller must re-inject the
    /// packet afterwards either way.
    pub fn rewrite(&self, packet: &mut WinDivertPacket<NetworkLayer>) -> bool {
        let data = match packet.data {
            std::borrow::Cow::Owned(ref mut d) => d,
            std::borrow::Cow::Borrowed(_) => return false,
        };
        let mut flows = match self.flows.lock() {
            Ok(f) => f,
            Err(_) => return false,
        };
        let changed = rewrite_packet(data, self.mitm_port, &mut flows);
        drop(flows);
        if changed {
            let _ = packet.recalculate_checksums(ChecksumFlags::default());
        }
        changed
    }
}

/// Spawn a background thread that redirects outbound HTTPS to the MITM listener.
///
/// Every received packet is rewritten (when applicable) and re-injected so the
/// network stack keeps working. The thread exits when the engine is stopped.
pub fn spawn_redirect(eng: RedirectEngine) -> anyhow::Result<()> {
    let running = eng.running.clone();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 65535];
        while running.load(Ordering::SeqCst) {
            match eng.recv_packet(&mut buffer, 100) {
                Ok(Some(packet)) => {
                    // `recv_wait` yields a borrowed packet; take ownership so the
                    // rewrite can mutate the bytes and recompute checksums.
                    let mut owned = packet.into_owned();
                    eng.rewrite(&mut owned);
                    if let Err(e) = eng.send_packet(&owned) {
                        tracing::warn!(err = ?e, "WinDivert redirect re-inject failed");
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(err = ?e, "WinDivert redirect recv error");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
        tracing::info!("WinDivert redirect loop exited");
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal IPv4/TCP packet for rewrite tests.
    fn tcp_packet(src: Ipv4Addr, sport: u16, dst: Ipv4Addr, dport: u16) -> Vec<u8> {
        let mut p = vec![0x45, 0x00, 0x00, 0x28, 0, 1, 0, 0, 64, 6, 0, 0];
        p.extend_from_slice(&src.octets());
        p.extend_from_slice(&dst.octets());
        p.extend_from_slice(&sport.to_be_bytes());
        p.extend_from_slice(&dport.to_be_bytes());
        p.extend_from_slice(&[0, 0, 0, 0, 80, 0x50, 0, 0]);
        p
    }

    #[test]
    fn rewrite_redirects_outbound_https_to_mitm() {
        let mut flows = FlowTable::new();
        let mut pkt = tcp_packet(
            Ipv4Addr::new(192, 168, 1, 10),
            50000,
            Ipv4Addr::new(93, 184, 216, 34),
            443,
        );
        assert!(rewrite_packet(&mut pkt, 18080, &mut flows));
        assert_eq!(&pkt[16..20], &LOOPBACK.octets());
        assert_eq!(u16::from_be_bytes([pkt[22], pkt[23]]), 18080);
        assert_eq!(flows.len(), 1);
        assert_eq!(
            flows.get(&50000).copied(),
            Some(SocketAddrV4::new(Ipv4Addr::new(93, 184, 216, 34), 443))
        );
    }

    #[test]
    fn rewrite_restores_server_address_on_response() {
        let mut flows = FlowTable::new();
        // Register the flow via an outbound packet.
        let mut out = tcp_packet(
            Ipv4Addr::new(192, 168, 1, 10),
            50000,
            Ipv4Addr::new(93, 184, 216, 34),
            443,
        );
        rewrite_packet(&mut out, 18080, &mut flows);

        // Response from the MITM listener back to the client.
        let mut resp = tcp_packet(LOOPBACK, 18080, Ipv4Addr::new(192, 168, 1, 10), 50000);
        assert!(rewrite_packet(&mut resp, 18080, &mut flows));
        assert_eq!(&resp[12..16], &[93, 184, 216, 34]);
        assert_eq!(u16::from_be_bytes([resp[20], resp[21]]), 443);
    }

    #[test]
    fn rewrite_ignores_non_https_traffic() {
        let mut flows = FlowTable::new();
        let mut pkt = tcp_packet(
            Ipv4Addr::new(192, 168, 1, 10),
            50000,
            Ipv4Addr::new(93, 184, 216, 34),
            80,
        );
        assert!(!rewrite_packet(&mut pkt, 18080, &mut flows));
        assert!(flows.is_empty());
    }

    #[test]
    fn rewrite_ignores_non_ipv4_and_short_packets() {
        let mut flows = FlowTable::new();
        let mut short = vec![0u8; 10];
        assert!(!rewrite_packet(&mut short, 18080, &mut flows));

        // IPv6 version nibble (6) must be rejected.
        let mut v6 = vec![0x60u8; 40];
        assert!(!rewrite_packet(&mut v6, 18080, &mut flows));
    }

    #[test]
    fn rewrite_ignores_udp() {
        let mut flows = FlowTable::new();
        let mut pkt = tcp_packet(
            Ipv4Addr::new(192, 168, 1, 10),
            50000,
            Ipv4Addr::new(93, 184, 216, 34),
            443,
        );
        pkt[IPV4_PROTO_OFFSET] = 17; // UDP
        assert!(!rewrite_packet(&mut pkt, 18080, &mut flows));
    }
}
