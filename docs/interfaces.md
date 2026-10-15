# FlowLens 公共 API 接口

## flowlens-core

### EventBus

```rust
pub trait EventBus: Send + Sync + 'static {
    fn publish(&self, event: TrafficEvent) -> Result<(), EventBusError>;
    fn subscribe(&self, filter: TrafficFilter) -> broadcast::Receiver<TrafficEvent>;
}
```

### SessionManager

```rust
pub trait SessionManager: Send + Sync + 'static {
    fn create_session(&self, conn: ConnectionKey) -> SessionId;
    fn update_session(&self, id: SessionId, event: TrafficEvent) -> Result<(), StorageError>;
    fn get_session(&self, id: SessionId) -> Option<TrafficSession>;
    fn persist(&self, id: SessionId) -> Result<(), StorageError>;
}
```

---

## flowlens-capture

### CaptureEngine

```rust
pub trait CaptureEngine: Send + Sync {
    fn start(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<(), CaptureError>;
    fn is_running(&self) -> bool;
    async fn receive_packet(&mut self) -> Option<RawPacket>;
}
```

### WfpFilterManager

```rust
pub trait WfpFilterManager: Send + Sync {
    fn add_ipv4_filter(&self, layer: WfpLayer) -> Result<FilterHandle>;
    fn add_ipv6_filter(&self, layer: WfpLayer) -> Result<FilterHandle>;
    fn remove_filter(&self, handle: FilterHandle) -> Result<()>;
    fn set_callout_filter(&self, filter_type: CalloutFilter) -> Result<()>;
}
```

---

## flowlens-attribution

### AttributionEngine

```rust
pub trait AttributionEngine: Send + Sync {
    fn start(&mut self) -> Result<(), AttributionError>;
    fn stop(&mut self) -> Result<(), AttributionError>;
    fn lookup_connection(&self, conn: &ConnectionKey) -> Result<ProcessInfo, AttributionError>;
    fn active_connections(&self) -> Vec<(ConnectionKey, ProcessInfo)>;
}
```

### IpHelperLookup

```rust
pub trait IpHelperLookup: Send + Sync {
    fn get_tcp_table(&self, family: AddressFamily) -> Result<Vec<TcpConnection>>;
    fn get_pid_from_tcp_socket(&self, src: SocketAddr, dst: SocketAddr) -> Option<u32>;
}
```

---

## flowlens-tls

### CaManager

```rust
pub trait CaManager: Send + Sync {
    fn generate_ca(&mut self, config: CaConfig) -> Result<CaInfo, TlsError>;
    fn install_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn uninstall_ca(&self, ca: &CaInfo) -> Result<(), TlsError>;
    fn check_installed(&self, ca: &CaInfo) -> bool;
}
```

### CertIssuer

```rust
pub trait CertIssuer: Send + Sync {
    fn sign_leaf_cert(
        &self,
        domain: &str,
        ca_info: &CaInfo
    ) -> Result<(Certificate, PrivateKey), TlsError>;
    fn cache_cert(&self, domain: &str, cert: (Certificate, PrivateKey));
}
```

---

## flowlens-storage

### StorageEngine

```rust
pub trait StorageEngine: Send + Sync {
    fn create_tables(&self) -> Result<(), StorageError>;
    fn insert_session(&self, session: &TrafficSession) -> Result<SessionId, StorageError>;
    fn get_session(&self, id: SessionId) -> Result<TrafficSession, StorageError>;
    fn query_sessions(&self, filter: StorageFilter, page: PageInfo) -> Result<Vec<TrafficSession>, StorageError>;
    fn delete_sessions(&self, before: i64) -> Result<u64, StorageError>;
}
```

---

## flowlens-export

### HarExporter

```rust
pub trait HarExporter: Send + Sync {
    fn export(&self, sessions: &[TrafficSession]) -> Result<String, ExportError>;
    fn export_to_file(&self, sessions: &[TrafficSession], path: &Path) -> Result<(), ExportError>;
}
```

### SazImporter

```rust
pub trait SazImporter: Send + Sync {
    fn import(&self, path: &Path) -> Result<Vec<TrafficSession>, SazError>;
}
```

---

## flowlens-mcp

### McpServer

```rust
pub trait McpServer: Send + Sync {
    fn start(&mut self, bind_addr: SocketAddr) -> Result<(), McpError>;
    fn stop(&mut self) -> Result<(), McpError>;
    fn register_tool(&mut self, tool: McpTool);
}
```

### McpTool

```rust
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value) -> Result<serde_json::Value, McpError>;
}
```

---

## flowlens-ui

### UiFrontend

```rust
pub trait UiFrontend: Send + Sync {
    fn run(&mut self, bus: Arc<dyn EventBus>) -> Result<(), UiError>;
    fn stop(&mut self) -> Result<(), UiError>;
}
```

---

*Document generated: 2026/10/14*