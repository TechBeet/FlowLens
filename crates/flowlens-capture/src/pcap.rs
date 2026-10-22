//! Real Npcap (pcap) capture engine for FlowLens

use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkType {
    #[default]
    Ethernet,
    Raw,
    Loopback,
    LinuxCooked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PacketDirection {
    Inbound,
    #[default]
    Outbound,
}

#[derive(Debug, Clone)]
pub struct RawPacket {
    pub timestamp: i64,
    pub interface_name: String,
    pub link_type: LinkType,
    pub data: Vec<u8>,
    pub direction: PacketDirection,
    pub src_mac: Option<String>,
    pub dst_mac: Option<String>,
}

/// Parse IPv4 packet to extract (src_ip, src_port, dst_ip, dst_port, protocol_byte)
pub fn parse_ipv4(payload: &[u8]) -> Option<(IpAddr, u16, IpAddr, u16, u8)> {
    if payload.len() < 20 {
        return None;
    }
    let ver = payload[0] >> 4;
    if ver != 4 {
        return None;
    }
    let ihl = ((payload[0] & 0x0F) as usize) * 4;
    if ihl < 20 || ihl + 4 > payload.len() {
        return None;
    }
    let proto = payload[9];
    let src = IpAddr::from([payload[12], payload[13], payload[14], payload[15]]);
    let dst = IpAddr::from([payload[16], payload[17], payload[18], payload[19]]);
    match proto {
        6 => {
            let sp = ((payload[ihl] as u16) << 8) | payload[ihl + 1] as u16;
            let dp = ((payload[ihl + 2] as u16) << 8) | payload[ihl + 3] as u16;
            Some((src, sp, dst, dp, proto))
        }
        17 => {
            let sp = ((payload[ihl] as u16) << 8) | payload[ihl + 1] as u16;
            let dp = ((payload[ihl + 2] as u16) << 8) | payload[ihl + 3] as u16;
            Some((src, sp, dst, dp, proto))
        }
        _ => None,
    }
}

/// Convert raw packet bytes (with Ethernet II header) into a RawPacket
pub fn parse_ethernet(data: &[u8]) -> Option<RawPacket> {
    if data.len() < 14 {
        return None;
    }
    let link_type = if data[12] == 0x08 && data[13] == 0x00 {
        // IPv4
        LinkType::Ethernet
    } else if data[12] == 0x86 && data[13] == 0xDD {
        // IPv6
        LinkType::Ethernet
    } else {
        LinkType::Raw
    };
    let ts = now_micros();
    let src_mac = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        data[0], data[1], data[2], data[3], data[4], data[5]
    );
    let dst_mac = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        data[6], data[7], data[8], data[9], data[10], data[11]
    );
    Some(RawPacket {
        timestamp: ts,
        interface_name: String::new(),
        link_type,
        data: data.to_vec(),
        direction: PacketDirection::Outbound,
        src_mac: Some(src_mac),
        dst_mac: Some(dst_mac),
    })
}

fn now_micros() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}
/// Npcap capture engine using the pcap crate
pub struct PcapEngine {
    device_name: String,
    pub(crate) running: Arc<std::sync::atomic::AtomicBool>,
    queue: Arc<Mutex<std::collections::VecDeque<RawPacket>>>,
    bpf_filter: Option<String>,
}

unsafe impl Send for PcapEngine {}
unsafe impl Sync for PcapEngine {}

impl PcapEngine {
    pub fn new(device_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            queue: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            bpf_filter: None,
        }
    }

    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.bpf_filter = Some(filter.into());
        self
    }

    /// Pick a capture device: prefer Npcap interfaces, fall back to first
    pub fn pick_device() -> Option<String> {
        let devices: Vec<String> = pcap::Device::list()
            .ok()?
            .into_iter()
            .map(|d| d.name)
            .collect();
        for name in &devices {
            if name.starts_with('\\') && name.to_lowercase().contains("npcap") {
                return Some(name.clone());
            }
        }
        for name in &devices {
            if name != "127.0.0.1" && name != "::1" {
                return Some(name.clone());
            }
        }
        devices.into_iter().next()
    }

    /// Open and start capturing; spawns a reader thread into the queue
    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        let device = self.device_name.clone();
        let cap = pcap::Capture::from_device(device.as_str())
            .ok()
            .and_then(|d| {
                let mut cap = d.promisc(true).snaplen(65535).timeout(100).open().ok()?;
                if let Some(f) = self.bpf_filter.clone() {
                    cap.filter(&f, true).ok()?;
                }
                Some(cap)
            });

        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        tracing::info!(device = %device, "Npcap capture started");

        if let Some(mut cap) = cap {
            let queue = self.queue.clone();
            let running = self.running.clone();
            std::thread::spawn(move || {
                while running.load(std::sync::atomic::Ordering::SeqCst) {
                    match cap.next_packet() {
                        Ok(pkt) => {
                            if let Some(rp) = parse_ethernet(pkt.data) {
                                if let Ok(mut q) = queue.lock() {
                                    q.push_back(rp);
                                    if q.len() > 4096 {
                                        q.pop_front();
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                    }
                }
                tracing::info!("Npcap reader thread exiting");
            });
        } else {
            tracing::warn!("Failed to open pcap device {} (no Npcap?)", device);
        }
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Npcap capture stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Drain up to `limit` packets
    pub fn drain_packets(&self, limit: usize) -> Vec<RawPacket> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.queue.lock() {
            while let Some(p) = q.pop_front() {
                out.push(p);
                if out.len() >= limit {
                    break;
                }
            }
        }
        out
    }

    /// List all available Npcap devices on the system.
    ///
    /// Returns an empty vector if Npcap is not installed or no devices
    /// can be enumerated. This is safe to call even without Npcap.
    pub fn list_devices() -> Vec<String> {
        match pcap::Device::list() {
            Ok(devices) => devices.into_iter().map(|d| d.name).collect(),
            Err(e) => {
                tracing::warn!("Failed to list Npcap devices: {}", e);
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4_tcp() {
        let mut pkt = vec![0xffu8; 6];
        pkt.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
        pkt.extend_from_slice(&[0x08, 0x00]);
        pkt.extend_from_slice(&[0x45, 0x00, 0x00, 0x28, 0, 1, 0, 0, 64, 6, 0, 0]);
        pkt.extend_from_slice(&[192, 168, 1, 10]);
        pkt.extend_from_slice(&[93, 184, 216, 34]);
        pkt.extend_from_slice(&[0x30, 0x39, 0x01, 0xbb, 0, 0, 0, 0, 80, 0x50, 0, 0]);
        let rp = parse_ethernet(&pkt).unwrap();
        let ip_payload = &rp.data[14..];
        let (_src, sport, dst, dport, proto) = parse_ipv4(ip_payload).unwrap();
        assert_eq!(proto, 6);
        assert_eq!(sport, 12345);
        assert_eq!(dport, 443);
        assert_eq!(dst, IpAddr::from([93, 184, 216, 34]));
    }

    #[test]
    fn test_engine_new_not_running() {
        let mut e = PcapEngine::new("Ethernet");
        assert!(!e.is_running());
        assert_eq!(e.device_name(), "Ethernet");
        e.stop().unwrap();
    }
}
