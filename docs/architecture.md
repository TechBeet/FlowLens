# FlowLens 架构设计

## 项目概述

FlowLens 是一个用 Rust 构建的 Windows 原生无代理 HTTP/HTTPS 抓包与调试工具。通过 WFP（Windows Filtering Platform）、ETW（Event Tracing for Windows）和本地根证书在系统协议栈中就地观察、解密并归属流量，无需注册系统代理、无需监听本地代理端口，也不需要 VPN 网卡或路由表修改。

**设计目标**：
- 无代理捕获所有 HTTP/HTTPS 流量
- TLS 1.2/1.3 中间人解密
- 精确进程归属（PID、进程名、EXE 路径、用户 SID、架构）
- 多协议解析（HTTP/1.1、HTTP/2、WebSocket、SSE、gRPC）
- 免安装便携式部署
- MCP 服务器供 AI 代理查询

---

## Crate 结构

```
FlowLens/
├── Cargo.toml                          # Workspace 根配置
├── rust-toolchain.toml                 # Rust 1.75+ (2021 edition)
├── crates/
│   ├── flowlens-core/                  # 领域模型、事件总线、会话状态机
│   ├── flowlens-capture/               # Npcap 回退 + WFP 用户态元数据
│   ├── flowlens-attribution/           # ETW + IP Helper 回退
│   ├── flowlens-tls/                   # 根证书、MITM、DPAPI 密钥
│   ├── flowlens-protocol/              # HTTP/1、HTTP/2、WebSocket、SSE、gRPC
│   ├── flowlens-storage/                # SQLite、查询、分页、索引
│   ├── flowlens-export/                # HAR 导出、SAZ 导入
│   ├── flowlens-mcp/                   # MCP 服务器
│   ├── flowlens-ui/                    # ratatui TUI / egui GUI
│   ├── flowlens-cli/                   # 命令行入口、权限检查
│   ├── flowlens-driver/                # WFP Callout 驱动 (C/C++) - 独立
│   └── flowlens-installer/             # Portable 打包、驱动管理
├── docs/                               # 规划文档
├── driver/                             # 内核态驱动 C/C++
│   ├── src/
│   │   ├── callout.c                   # Callout 核心
│   │   ├── wfp_engine.c                # WFP filter engine
│   │   ├── shared_mem.c                # 共享内存环形缓冲区
│   │   └── ioctl.c                     # IOCTL 处理
│   ├── include/
│   │   └── flowlens_driver.h
│   └── CMakeLists.txt
└── tests/
    ├── integration/
    └── e2e/
```

---

## 模块边界图

```
┌─────────────────────────────────────────────────────┐
│                    flowlens-cli                     │  ← 命令行入口
│              (admin check + arg parse)              │
└───────────────────────┬─────────────────────────────┘
                        │
┌───────────────────────┴─────────────────────────────┐
│                    flowlens-ui                       │  ← TUI / GUI
│              (ratatui + egui frames)               │
└───────────────────────┬─────────────────────────────┘
                        │
┌───────────────────────┴─────────────────────────────┐
│                   flowlens-mcp                      │  ← JSON-RPC
│            (MCP protocol + tools)                   │
└───────────────────────┬─────────────────────────────┘
                        │
┌───────────────────────┴─────────────────────────────┐
│                  flowlens-core                      │  ← 领域事件总线
│          EventBus + SessionManager + Models         │
└──────┬─────────┬──────────┬─────────┬───────────────┘
       │         │           │         │
┌──────┴──┐ ┌───┴────┐ ┌────┴───┐ ┌────┴────┐
│ capture │ │attrbut │ │  tls   │ │exports  │
│(capture)│ │(assign)│ │(tls)   │ │(export) │
└────┬────┘ └───┬────┘ └────┬───┘ └─────────┘
     │           │            │
┌────┴──────────┴────────────┴──────────────┐
│            driver (WFP callout)            │
│        (kernel-mode packet inspection)        │
└────────────────────────────────────────────┘
```

### 2. flowlens-capture

**定位**：Npcap 回退捕获 + WFP 用户态元数据过滤

#### 公共 trait

```rust
/// 流量捕获接口
pub trait CaptureEngine: Send + Sync {
    fn start(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<(), CaptureError>;
    fn is_running(&self) -> bool;
    async fn receive_packet(&mut self) -> Option<RawPacket>;
}

/// WFP 过滤器管理器（用户态元数据）
pub trait WfpFilterManager: Send + Sync {
    fn add_ipv4_filter(&self, layer: WfpLayer) -> Result<FilterHandle>;
    fn add_ipv6_filter(&self, layer: WfpLayer) -> Result<FilterHandle>;
    fn remove_filter(&self, handle: FilterHandle) -> Result<()>;
    fn set_callout_filter(&self, filter_type: CalloutFilter) -> Result<()>;
}
```

#### 核心结构体

```rust
/// 原始数据包
#[derive(Debug, Clone)]
pub struct RawPacket {
    pub timestamp: i64,
    pub interface_name: String,
    pub link_type: LinkType,
    pub data: Vec<u8>,
    pub captured_len: usize,
    pub original_len: usize,
    pub direction: PacketDirection,
    pub src_mac: Option<String>,
    pub dst_mac: Option<String>,
}

/// 捕获结果
#[derive(Debug)]
pub enum CaptureResult {
    Success,
    FallbackNpcap,
    DriverUnavailable,
    PermissionDenied,
}
```

---

### 3. flowlens-attribution

**定位**：ETW 订阅 + IP Helper 回退，输出 ProcessInfo + MatchQuality

#### 公共 trait

```rust
/// 进程归属引擎
pub trait AttributionEngine: Send + Sync {
    fn start(&mut self) -> Result<(), AttributionError>;
    fn stop(&mut self) -> Result<(), AttributionError>;
    fn lookup_connection(&self, conn: &ConnectionKey) -> Result<ProcessInfo, AttributionError>;
    fn active_connections(&self) -> Vec<(ConnectionKey, ProcessInfo)>;
}

/// IP Helper 回退
pub trait IpHelperLookup: Send + Sync {
    fn get_tcp_table(&self, family: AddressFamily) -> Result<Vec<TcpConnection>>;
    fn get_pid_from_tcp_socket(&self, src: SocketAddr, dst: SocketAddr) -> Option<u32>;
}
```

### 事件流转图

```
[1. 捕获层]
Npcap/WFP → RawPacket
    │
    ▼
[2. 连接 keyed]
Packet → Ethernet/IP/TCP → ConnectionKey
    │
    ▼
[3. 进程归属]
ETW Lookup + IPHelper Fallback → ProcessInfo + MatchQuality
    │
    ▼
[4. TLS 解密]
TlsModule → Sniff SNI → Look up CA → Sign Leaf Cert → Decrypt Stream
    │
    ▼
[5. 协议解析]
HTTPParser → HttpRequest/Response
    │
    ├──→ WebSocketParser → WsFrame stream
    ├──→ SseParser → SseEvent stream
    └──→ GrpcParser → GrpcCall aggregate
    │
    ▼
[6. 存储层]
TrafficSession → SQLite
    │
    ▼
[7. 事件总线]
TrafficEvent → UI/MCP/Real-time Subscribe
```

### 同步边界

```rust
// 主循环：async + spawn blocking
loop {
    // 1. Async capture (kernel → user)
    let packet = capture_engine.receive_packet().await?;

    // 2. Spawn blocking attribution + processing (CPU bound)
    let task = tokio::task::spawn_blocking(move || {
        let conn_key = extract_conn_key(&packet);
        let process_info = attribution_engine.lookup(&conn_key);
        let session = protocol_parser.parse(&packet.data, &process_info);
        session
    });

    // 3. Send to storage (async)
    let session = task.await?;
    storage.insert_session(&session).await;
}
```

---

## 驱动设计（C/C++）

### WFP Callout Driver

```c
// callout.c
typedef struct _FLOWLENS_CONTEXT {
    PWCHAR interface_name;
    UINT32 process_id;
    GUID callout_guid;
    LIST_ENTRY packet_queue;
    KSPIN_LOCK queue_lock;
} FLOWLENS_CONTEXT, *PFLOWLENS_CONTEXT;

// IOCTL 接口
#define IOCTL_FLOWLENS_GET_PACKETS     CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_OUT_DIRECT, FILE_ANY_ACCESS)
#define IOCTL_FLOWLENS_GET_PROCESS    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x801, METHOD_OUT_DIRECT, FILE_ANY_ACCESS)
#define IOCTL_FLOWLENS_REMOVE_FILTER  CTL_CODE(FILE_DEVICE_UNKNOWN, 0x802, METHOD_IN_DIRECT, FILE_ANY_ACCESS)
```

---

*Document generated: 2026/10/14*