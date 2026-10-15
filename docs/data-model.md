# FlowLens 数据模型

## 核心模型定义

### ConnectionKey

```rust
/// 连接键 - 唯一标识一个连接
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConnectionKey {
    pub src_addr: SocketAddr,
    pub dst_addr: SocketAddr,
    pub protocol: TransportProtocol,      // TCP/UDP
    pub connection_id: u64,               // 全局 monotonic ID
    pub timestamp: i64,                   // 微秒时间戳
    pub interface_guid: Option<String>,   // 捕获接口 GUID
}
```

### ProcessInfo

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,                     // 进程名 (e.g. "chrome.exe")
    pub exe_path: Option<PathBuf>,        // 完整路径
    pub user_sid: String,                 // e.g. S-1-5-21-...
    pub arch: ProcessArch,                // x86/x64/Arm64
    pub match_quality: MatchQuality,
}
```

### TlsSession

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsSession {
    pub tls_version: TlsVersion,          // TLS1.2/TLS1.3
    pub cipher_suite: String,
    pub server_name: Option<String>,       // SNI
    pub alpn: Option<String>,             // HTTP/2
    pub client_hello: Option<Vec<u8>>,
    pub server_hello: Option<Vec<u8>>,
    pub master_secret: Option<String>,    // 仅调试时保留
}
```

---

## 协议层模型

### HttpRequest

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,                   // GET/POST/PUT/DELETE
    pub url: String,
    pub path: String,
    pub request_line_version: HttpVersion,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub body_size: usize,
    pub timestamp: i64,
}
```

### HttpResponse

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub version: HttpVersion,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub body_size: usize,
    pub timestamp: i64,
}
```

### WebSocketFrame

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketFrame {
    pub opcode: WsOpcode,                 // Text/Binary/Close/Ping/Pong
    pub payload: Vec<u8>,
    pub payload_size: usize,
    pub is_masked: bool,
    pub timestamp: i64,
}
```

### SseEvent

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub id: Option<String>,
    pub data: String,
    pub retry_ms: Option<u64>,
    pub timestamp: i64,
}
```

### GrpcCall

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcCall {
    pub service: String,
    pub method: String,
    pub request_frames: Vec<Vec<u8>>,
    pub response_frames: Vec<Vec<u8>>,
    pub status: Option<u32>,              // gRPC status code
    pub is_streaming: bool,
    pub timestamp: i64,
}
```

---

## 会话模型

### TrafficSession

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSession {
    pub session_id: SessionId,
    pub conn: ConnectionKey,
    pub process: ProcessInfo,
    pub tls: Option<TlsSession>,
    pub protocol: ProtocolKind,
    pub http_request: Option<HttpRequest>,
    pub http_response: Option<HttpResponse>,
    pub websocket_frames: Vec<WebSocketFrame>,
    pub sse_events: Vec<SseEvent>,
    pub grpc_call: Option<GrpcCall>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
```

---

## 枚举类型

### MatchQuality

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchQuality {
    ExactTuple,        // ETW精确匹配五元组
    Heuristic,        // IP Helper启发式
    Unspecified,      // 未知
}
```

### CaptureSource

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureSource {
    Npcap,           // Npcap 用户态
    WfpCallout,      // WFP 内核态 Callout
    EtCombined,      // ETW + WFP 联合
}
```

### ProtocolKind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolKind {
    Http1,
    Http2,
    WebSocket,
    Sse,
    Grpc,
    Unknown,
}
```

### ProcessArch

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessArch {
    X86,
    X64,
    Arm64,
    Unknown,
}
```

### TlsVersion

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsVersion {
    Tls10,
    Tls11,
    Tls12,
    Tls13,
}
```

---

## 数据库索引策略

```sql
-- 主键
PRIMARY KEY (id)

-- 时间范围查询
CREATE INDEX idx_timestamp ON sessions(timestamp);

-- PID 过滤
CREATE INDEX idx_pid ON sessions(process_pid);

-- Host 过滤
CREATE INDEX idx_host ON sessions(sni, http_url);

-- 协议过滤
CREATE INDEX idx_protocol ON sessions(protocol);

-- 匹配质量
CREATE INDEX idx_match ON sessions(match_quality);

-- 创建时间
CREATE INDEX idx_created ON sessions(created_at);
```

---

## JSON Schema 验证

所有模型必须实现 `Serialize` 和 `Deserialize` trait，确保：
1. MCP 服务器返回正确 JSON
2. HAR 导出格式兼容
3. 跨语言序列化