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
    fn get_pid_from_udp_socket(&self, sock: SocketAddr) -> Option<u32>;
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
                .unwrap_or_else(|e| e.into_inner())
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

/// Real Windows IP Helper implementation using iphlpapi.dll
#[cfg(windows)]
pub struct WindowsIpHelper;

#[cfg(windows)]
impl WindowsIpHelper {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl Default for WindowsIpHelper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl IpHelperLookup for WindowsIpHelper {
    fn get_tcp_table(&self, _family: AddressFamily) -> Vec<TcpConnection> {
        Vec::new()
    }

    fn get_pid_from_tcp_socket(&self, src: SocketAddr, dst: SocketAddr) -> Option<u32> {
        use windows_sys::Win32::NetworkManagement::IpHelper::*;
        use windows_sys::Win32::Networking::WinSock::*;

        let mut size: u32 = 0;
        unsafe {
            // First call to get required size
            let _ = GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                false as i32,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            );
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let ret = GetExtendedTcpTable(
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut size,
                false as i32,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            );
            if ret != 0 {
                return None;
            }
            let table = buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
            let count = (*table).dwNumEntries;
            for i in 0..count {
                let row = &(*table).table[i as usize];
                let src_addr = SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::from(row.dwLocalAddr.to_le_bytes())),
                    row.dwLocalPort as u16,
                );
                let dst_addr = SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::from(row.dwRemoteAddr.to_le_bytes())),
                    row.dwRemotePort as u16,
                );
                if src_addr == src && dst_addr == dst {
                    return Some(row.dwOwningPid);
                }
            }
            None
        }
    }

    fn get_pid_from_udp_socket(&self, sock: SocketAddr) -> Option<u32> {
        use windows_sys::Win32::NetworkManagement::IpHelper::*;
        use windows_sys::Win32::Networking::WinSock::*;

        let mut size: u32 = 0;
        unsafe {
            let _ = GetExtendedUdpTable(
                std::ptr::null_mut(),
                &mut size,
                false as i32,
                AF_INET as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let ret = GetExtendedUdpTable(
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut size,
                false as i32,
                AF_INET as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );
            if ret != 0 {
                return None;
            }
            let table = buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID;
            let count = (*table).dwNumEntries;
            for i in 0..count {
                let row = &(*table).table[i as usize];
                let local_sock = SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::from(row.dwLocalAddr.to_le_bytes())),
                    row.dwLocalPort as u16,
                );
                if local_sock == sock {
                    return Some(row.dwOwningPid);
                }
            }
            None
        }
    }
}

/// Non-Windows fallback: use FakeIpHelper behavior
#[cfg(not(windows))]
pub struct WindowsIpHelper;

#[cfg(not(windows))]
impl WindowsIpHelper {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(windows))]
impl IpHelperLookup for WindowsIpHelper {
    fn get_tcp_table(&self, _family: AddressFamily) -> Vec<TcpConnection> {
        Vec::new()
    }
    fn get_pid_from_tcp_socket(&self, _src: SocketAddr, _dst: SocketAddr) -> Option<u32> {
        None
    }
    fn get_pid_from_udp_socket(&self, _sock: SocketAddr) -> Option<u32> {
        None
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

    fn get_pid_from_udp_socket(&self, _sock: SocketAddr) -> Option<u32> {
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
