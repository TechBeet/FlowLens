//! Process attribution engine for FlowLens
//!
//! This module provides:
//! - ETW (Event Tracing for Windows) process attribution
//! - IP Helper API fallback for PID lookup
//! - ProcessInfo generation with match quality

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use flowlens_core::models::{ConnectionKey, MatchQuality, ProcessArch, ProcessInfo};

/// Attribution engine trait
pub trait AttributionEngine: Send + Sync {
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
    fn lookup_connection(&self, conn: &ConnectionKey) -> Option<ProcessInfo>;
    fn active_connections(&self) -> Vec<(ConnectionKey, ProcessInfo)>;
}

/// IP Helper attribution (fallback)
pub trait IpHelperLookup: Send + Sync {
    fn get_tcp_table(&self, family: AddressFamily) -> Vec<TcpConnection>;
    fn get_pid_from_tcp_socket(&self, src: SocketAddr, dst: SocketAddr) -> Option<u32>;
    fn get_pid_from_udp_socket(&self, socket: SocketAddr) -> Option<u32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone)]
pub struct TcpConnection {
    pub src_addr: SocketAddr,
    pub dst_addr: SocketAddr,
    pub state: TcpState,
    pub pid: u32,
    pub interface: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Established,
    Listen,
    SynSent,
    FinWait1,
    TimeWait,
    Closed,
}

#[derive(Debug, Clone)]
pub struct UdpConnection {
    pub socket_addr: SocketAddr,
    pub pid: u32,
    pub interface: String,
}

/// ETW event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ETWEventType {
    TcpConnect,
    TcpDisconnect,
    UdpConnect,
    UdpDisconnect,
    ProcessCreate,
    ProcessExit,
    SocketOpened,
}

#[derive(Debug, Clone)]
pub struct ETWEvent {
    pub event_type: ETWEventType,
    pub pid: u32,
    pub timestamp: i64,
    pub payload: Vec<u8>,
    pub thread_id: u32,
}

/// In-memory attribution engine for Phase 1 MVP
pub struct InMemoryAttributionEngine {
    active_connections: Arc<RwLock<HashMap<ConnectionKey, ProcessInfo>>>,
    ip_helper: Arc<dyn IpHelperLookup>,
}

impl InMemoryAttributionEngine {
    pub fn new(ip_helper: Arc<dyn IpHelperLookup>) -> Self {
        Self {
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            ip_helper,
        }
    }
}

impl AttributionEngine for InMemoryAttributionEngine {
    fn start(&mut self) -> anyhow::Result<()> {
        tracing::info!("In-memory attribution engine started");
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("In-memory attribution engine stopped");
        Ok(())
    }

    fn lookup_connection(&self, conn: &ConnectionKey) -> Option<ProcessInfo> {
        // Check cache first
        {
            if let Ok(lock) = self.active_connections.read() {
                if let Some(process) = lock.get(conn) {
                    return Some(process.clone());
                }
            }
        }

        // Fall back to IP Helper
        if let Some(pid) = self
            .ip_helper
            .get_pid_from_tcp_socket(conn.src_addr, conn.dst_addr)
        {
            let process = ProcessInfo {
                pid,
                name: format!("pid_{}", pid),
                exe_path: None,
                user_sid: String::new(),
                arch: ProcessArch::Unknown,
                match_quality: MatchQuality::Heuristic,
            };
            self.active_connections
                .write()
                .unwrap()
                .insert(conn.clone(), process.clone());
            return Some(process);
        }

        None
    }

    fn active_connections(&self) -> Vec<(ConnectionKey, ProcessInfo)> {
        if let Ok(lock) = self.active_connections.read() {
            lock.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            Vec::new()
        }
    }
}

/// Simple IP Helper implementation for testing
#[derive(Default)]
pub struct FakeIpHelper;

impl IpHelperLookup for FakeIpHelper {
    fn get_tcp_table(&self, _family: AddressFamily) -> Vec<TcpConnection> {
        Vec::new()
    }

    fn get_pid_from_tcp_socket(&self, _src: SocketAddr, _dst: SocketAddr) -> Option<u32> {
        // Simulate finding a process
        Some(1234)
    }

    fn get_pid_from_udp_socket(&self, _socket: SocketAddr) -> Option<u32> {
        Some(5678)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribution_lookup() {
        let engine = InMemoryAttributionEngine::new(Arc::new(FakeIpHelper));
        let conn = ConnectionKey {
            src_addr: "127.0.0.1:12345".parse().unwrap(),
            dst_addr: "8.8.8.8:53".parse().unwrap(),
            protocol: flowlens_core::models::TransportProtocol::Udp,
            connection_id: 1,
            timestamp: 0,
            interface_guid: None,
        };

        let result = engine.lookup_connection(&conn);
        assert!(result.is_some());
        let process = result.unwrap();
        assert_eq!(process.pid, 1234);
        assert_eq!(process.match_quality, MatchQuality::Heuristic);
    }
}
