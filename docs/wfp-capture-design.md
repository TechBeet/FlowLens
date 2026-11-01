# FlowLens Windows 流量捕获技术设计（Phase 4）

> 目标：为 FlowLens 设计无需代理（proxy-less）的流量捕获方案，最终以 WFP Callout
> 驱动替代 Npcap，实现对标/超越 HTTP Debugger Pro 的流量拦截能力。
> 本文档覆盖：竞品技术对比、方案选型、WFP 驱动架构、进程归属、代码框架、Rust FFI、
> 签名部署、开发测试指南与风险评估。

---

## 1. 竞品分析：HTTP Debugger Pro 如何实现无代理捕获

HTTP Debugger Pro（HTTPDebugger）宣称"无需设置代理即可捕获任意进程的 HTTP/HTTPS"。
根据其行为特征（安装服务、加载内核组件、按进程过滤、可实时修改/阻断请求）可推断其技术组合：

| 特征 | 实现机制 |
|------|----------|
| 安装 `HTTPDebuggerSrv` 服务 + 驱动 | 驱动负责在内核挂钩网络路径 |
| 无需系统代理设置 | 不走 WinINET 代理路径，直接在网络栈拦截 |
| 按进程精确过滤、进程内上下文 | 在连接建立点（socket 层附近）挂钩，携带调用进程上下文 |
| 实时拦截/修改/断开连接 | 需要可 **pend/complete/block** 分类的钩子，即 WFP-style 的授权层过滤 |
| 解密 HTTPS | 用户态安装根 CA 做 MITM，与 FlowLens 相同思路 |

结论：
- 它的本质是一套 **授权层（Authorization Layer）过滤 + 用户态 MITM** 的组合，
  最可能的实现是 **WFP Callout（ALE 层）** 或更老的 TDI 过滤驱动。
- 与单纯抓包（Wireshark/Npcap）不同，它的杀手锏是：
  1. **每连接的进程上下文天然可得**（在 ALE AUTH_CONNECT 层，WFP 直接给出 PID）
  2. **可拦截（block/modify）而不仅是观察**
- FlowLens Phase 4 的 WFP 方案正是对这两点的复刻与增强（增加方向还原、环形零拷贝、Rust 管道）。

不可借鉴之处：HTTP Debugger 的闭源驱动无签名透明度；FlowLens 应公开驱动源码并走正规签名。

---

## 2. Windows 流量捕获方案对比

### 2.1 总览

| 维度 | Npcap（现状） | WFP Callout | LSP | WinDivert | ETW Kernel-Network |
|------|--------------|------------|-----|-----------|-------------------|
| 态别 | 用户态（NDIS 内核驱动只转发） | 内核态 | 用户态 DLL 注入 | 用户态 API + 内核驱动 | 用户态消费事件 |
| 进程归属 | ❌ 需另行推导（IP Helper/ETW） | ✅ 分类时原生提供 PID | ✅（在进程内） | ✅（上层可用） | ✅ 事件自带 PID |
| 修改/拦截流量 | ❌ | ✅ block/pend/inject | ✅ | ✅ | ❌ |
| 循环流量（loopback） | 需装 Loopback Adapter | ✅ 天然支持（有专门层） | ✅ | ✅ | 部分 |
| TLS 解密配合 | 需 MITM 代理端口/透明重定向 | ✅ 可在 ALE 层做透明 REDIRECT | ✅ | ✅ | ❌ |
| 稳定性风险 | 低（第三方成熟） | 高（自己写内核） | 高（注入易崩、生态混乱） | 低（驱动已签名） | 无 |
| 部署依赖 | 需安装 Npcap | 自带驱动 | 无 | 带 WinDivert64.sys | 系统内置 |
| 签名成本 | Npcap 已签名 | 需自签/EV | 无需 | 作者已 EV 签名 | 无需 |

### 2.2 各方案详评

#### Npcap（当前方案）
优点：
- 成熟稳定，BPF 过滤在内核完成，CPU 开销可控
- 生态好（pcap/Wireshark 格式互通），已实现 `flowlens-capture::PcapEngine`

缺点（决定必须演进）：
- **无进程上下文**：必须在用户态二次关联（当前 `flowlens-attribution` 的
  IP Helper 回退只有 Heuristic 质量）
- **无拦截能力**：只能看不能改/断，永远无法对标 HTTP Debugger Pro
- **loopback 需要 Npcap Loopback Adapter**，且其 DLT_NULL 封装需特殊解析
- **安装摩擦**：用户必须先装 Npcap（第三方驱动），便携化目标受阻
- 线程模型差：`pcap_next` 阻塞轮询，现有代码用 sleep 轮询队列，高吞吐丢帧

定位：保留为 **回退路径**（roadmap 中"WFP 加载失败自动切回 Npcap"），以及 CI 无驱动测试。

#### WFP Callout（目标方案）
- Vista+ 全版本支持，微软官方推荐的过滤框架，取代 TDI 过滤/LSP/Hook
- 关键优势：**分类元数据免费携带 PID、进程创建时间、用户 SID、AppContainer**
  （`FWPS_INCOMING_METADATA0.processId / processCreateTimestamp / processPath`）
- 可用层（详见 §3.1）：ALE_AUTH_CONNECT / ALE_FLOW_ESTABLISHED / TRANSPORT /
  INBOUND_IPPACKET / OUTBOUND_IPPACKET / STREAM
- 代价：内核开发 + WHQL/EV 签名 + 蓝屏风险控制

#### LSP（Layered Service Provider）
- **已废弃**：微软自 Win8/Server2012 起将 LSP 列为弃用，推荐迁移 WFP；
  WSAInstallProvider 在 Win8 上对非 inbox LSP 大幅受限
- 每个 ws2_32.dll 消费方都会加载 LSP DLL，崩溃域极大、且 Windows Store/沙盒应用绕过
- **结论：不采用**。仅在文档中保留为历史背景。

#### WinDivert
- 事实上的开源标准（reqrypt/WinDivert），本质就是"别人写好的 WFP 用户态封装"
- 提供 `WINDIVERT_LAYER_NETWORK/FLOW/SOCKET/REFLECT`，可注入/修改/丢弃
- **FLOW 层直接给事件含 PID + 进程创建时间**——性价比极高
- 限制：GPL/LGPL 双许可（商用需注意）、驱动由原作者 EV 签名（版本绑定单一 sys）、
  数据仍需用户态拆封重注入，高吞吐（>1Gbps）性能弱于内核内截取

**建议**：将 WinDivert 作为 Phase 4 之前的 **过渡引擎**（flowlens-capture 增加
`Engine::WinDivert` 变体），验证会话关联管道后再上自研驱动。理由：
1. 无需自己写内核就能拿到 PID 归属与方向（FLOW 层建 hash map，NETWORK 层收包）
2. 深度体验"观察者模式 + 数据面"的 API 形态，为自研驱动定型

#### 其他简述
- **pktmon / netsh trace**：系统内置但面向诊断，实时性与数据面控制不足
- **NDIS 轻量过滤器（LWF）**：链路层可见 MAC/全帧，但无进程上下文、无法挂起；
  仅当未来需要链路层重放时考虑
- **AF_PACKET 式 RAW socket（WSK）**：可发不可收全量，排除

### 2.3 选型结论

```
Phase 4 最终形态：
  主路径  : 自研 WFP Callout Driver（内核零拷贝环形缓冲 → 用户态 Rust）
  过渡路径: WinDivert 引擎（同一 CaptureEngine trait）
  回退路径: Npcap（现状保持）
选择器  : flowlens-capture::Engine::auto() 扩展为三态探测
```

---
## 3. WFP Callout Driver 架构设计

### 3.1 关键层（WFP Layers）与调用顺序

```
应用 connect()
       │
       ▼
ALE_AUTH_CONNECT_V4/V6     ← 授权层，天然携带 PID/进程创建时间/SID（归属黄金位置）
       │
       ▼ (三次握手完成)
ALE_FLOW_ESTABLISHED_V4/V6 ← 建 FlowContext，关联 5-tuple ↔ processId
       │
       ▼ (数据面，每包/每流)
STREAM_V4/V6               ← TCP 流数据（带 flowContext，免重复归属）
TRANSPORT_V4/V6            ← 备选：收完整 IP 包（类 pcap 视角）
```

**分层职责**：

| 层 | 作用 | 说明 |
|----|------|------|
| ALE_AUTH_CONNECT_V4/V6 | 记录 `(5-tuple → PID, createTime, exe)` 映射 | 每连接一次，开销极低；connect/accept 均触发 |
| ALE_FLOW_ESTABLISHED_V4/V6 | 关联 flowContext | 把 ALE 得到的进程信息绑到后续数据包 |
| STREAM_V4/V6 (出+入方向) | 拷贝 TCP payload 进环形缓冲 | 流级而非包级，天然重组、有序、去重 |
| DATAGRAM_DATA_V4/V6 | UDP payload | QUIC/DNS 场景 |

**STREAM 层 vs TRANSPORT 层的选择**：
- STREAM 层优点：内核已完成重组、按流交付、无需处理乱序/重传/分片，
  直接得到方向明确的 payload——**强烈推荐为 HTTP 抓包主通道**
- TRANSPORT/IPPACKET 层优点：原始字节保真（含包头），便于导出 pcap 与哈希
  ——可作为可选"raw mode"，Phase 4.2 再加

**数据投递模式（重要决策）**：
- 观察-only（默认）：classify 直接返回 `PERMIT`，同时拷贝数据入 ring——零影响网络
- 拦截模式（后续）：`FWP_ACTION_BLOCK` 阻断；修改需 pend + 注入
  （`FwpsStreamInjectAsync0` / `FwpsInjectNetworkReceiveAsync0`），先不实现

### 3.2 Callout 注册与 Filter 设置

```c
// DriverEntry 中（简化骨架，完整版见 §5）
// 1. FwpsCalloutRegister3  × (每层一个)     — KMDF 侧注册 classify/notify/flowDelete
// 2. FwpmEngineOpen0(...)                       — BFE 会话
// 3. FwpmTransactionBegin0 → FwpmCalloutAdd0 ×N
//    → FwpmFilterAdd0 ×N (action=FWP_ACTION_CALLOUT_UNKNOWN, weight=最低)
//    → FwpmTransactionCommit0                   — 事务保证原子性
// 4. IoCreateDevice / 设备接口 \\.\FlowLensWfp  — IOCTL 通道
```

要点：
- **事务化**：callout + filter 必须在同一 `FwpmTransaction` 中提交，避免半装状态
- **weight 设为最小值**（`FWP_EMPTY` 子过滤 + weight 0）：FlowLens 只观察，绝不抢占
  防火墙策略的判定顺序
- **sublayer 独立**：注册专属 `FLOWLENS_SUBLAYER`，卸载时 `FwpmEngineClose0` +
  DriverUnload 清理全部 filter，不留孤儿规则
- **Pended 检查**：STREAM 层 classify 必须 `FwpsAcquireClassifyHandle0` +
  在 `flowContext` 中保存 `flowId`，供日后拦截改造使用

### 3.3 内核 → 用户态数据传输：共享内存环形缓冲 vs IOCTL

| 方案 | 延迟 | 吞吐 | 复杂度 | 结论 |
|------|------|------|--------|------|
| 同步 IOCTL 逐包 | 差（往返 1-5μs×N） | < 20k pps | 低 | 仅用于元数据/控制 |
| METHOD_OUT_DIRECT 批量拉取 | 中 | 中 | 低 | **MVP 阶段采用** |
| 共享内存 ring + 命名事件唤醒 | 优（<1μs 通告） | 极高（零拷贝读） | 中高 | **目标方案** |
| 反向回调（内核调用户态） | — | — | — | 不可行/不安全 |

**选定混合架构**：
1. **数据面**：环形缓冲区，内核单写者（classify @ DISPATCH_LEVEL）→ 用户态单读者
2. **控制面**：IOCTL（打开/关闭捕获、设置过滤条件、查询统计、读取 PID 映射表快照）
3. **唤醒**：`KeSetEvent` 触发用户态 overlapped ReadFile 完成通知（或挂起的
   `IOCTL_GET_PACKETS` 立即完成），避免用户态忙轮询

环形缓冲布局（`ring.h`，SPSC、变长记录、非分页池）：

```c
#define FLOWLENS_RING_SIZE   (8 * 1024 * 1024)   // 8 MiB，IOCTL 可调
#define FLOWLENS_MAX_PKT     (256 * 1024)

typedef struct DECLSPEC_ALIGN(64) _RING_HEADER {   // 只读页
    volatile ULONG64 write_pos;      // 单调递增字节数
    ULONG32          capacity;
    ULONG32          magic;           // 'FLNS'
    ULONG64          dropped_count;  // 满则丢 + 计数
} RING_HEADER;

// 记录格式（变长，紧跟其后是 payload）
typedef struct _RECORD_HEADER {
    UINT32 record_len;       // 含本头
    UINT64 ts_ns;
    UINT32 direction;        // 0=in,1=out
    UINT32 flow_id;          // WFP flowId 或自增会话号
    UINT32 pid;
    UINT32 src_ip4, dst_ip4; // IPv6: 另用 record_v6
    UINT16 src_port, dst_port;
    UINT16 protocol;         // 6/17
    UINT16 payload_len;
    // followed by: payload[payload_len]
} RECORD_HEADER;
```

写者规则（classify 内，IRQL = DISPATCH_LEVEL，禁止阻塞/分页访问）：
- `headroom = capacity - (write_pos & (capacity-1))`；不足一条则环绕填 0 跳到头部
- 满（write_pos - committed_read_pos > capacity - MAX_PKT）→ 原子递增 dropped_count，静默丢
- 先写 payload 再以 `InterlockedExchange64(&write_pos, new)` 发布（发布屏障）

读者规则（用户态 mmap 或 `IOCTL_MAP_RING` 得到视图）：
- 读到 wrap 标记跳回 0；`committed_read_pos` 定期用 `IOCTL_SET_READ_POS` 回写
- 校验 `record_len` 合法性，防脏数据

### 3.4 双向流量（Request + Response）关联

核心思想：**以内核 flowId 为主键，而不是靠 5-tuple 二次猜**。

```
ALE_AUTH_CONNECT:  (local,remote,proto) --map--> flowCtx{pid, procCreateTime, dir=initiator}
ALE_FLOW_ESTABLISHED: fwpsFlowAssociateContext0 绑定 flowCtx
STREAM classify:   flowContext 直接携带 pid/direction → 记录带 flow_id 落 ring
用户态 SessionBuilder: flow_id 分组 → 同一 flow_id 内 ts 排序即得请求/响应交替序列
```

- 出向流（客户端发起）direction=out 即 request 方向；入向即 response 方向，
  不再需要从 MAC/端口启发式判断方向（修复现有 Npcap 引擎 `direction` 恒为
  Outbound 的缺陷，见 `pcap.rs:93`）
## 4. 进程归属技术深入分析

### 4.1 WFP 内置进程信息（首选方案）

在 `ALE_AUTH_CONNECT_V4/V6` 层 classify 函数中，`inMetaValues` 结构提供：

```c
typedef struct _FWPS_INCOMING_METADATA_VALUES0 {
    UINT32 processId;
    UINT32 reserved;
    UINT64 processCreateTime;      // ← 关键！用于防 PID 复用
    PCWSTR processPath;            // 可选：完整 exe 路径（Unicode）
    UINT32 sessionId;              // 用户登录会话 ID
    UINT64 effectiveToken;         // 访问令牌句柄（可查询 SID）
    // ...
} FWPS_INCOMING_METADATA_VALUES0;
```

**优势**：
- **零额外 syscall**：内核已准备好，无需 `PsLookupProcessByProcessId`
- **精确度最高**：`MatchQuality::Exact`（优于现有 IP Helper 的 Heuristic）
- **附带信息**：processPath 可直接展示；sessionId 可用于多用户隔离

**注意点**：
- `processPath` 在某些 AppContainer/沙盒场景为空，需回退到 PID→OpenProcess→QueryFullProcessImageName
- `effectiveToken` 仅在拥有 SeDebugPrivilege 时可访问，普通用户模式可忽略

### 4.2 ETW 订阅（备份方案）

当 WFP 未加载或需要历史追溯时启用。

**Provider**：`Microsoft-Windows-Kernel-Network` (GUID: `31b3d0a3-5c15-4d28-9eb5-3c9c9d196a84`)

**事件类型**：

| Event ID | 名称 | 字段 |
|----------|------|------|
| 4 | TcpConnect | ProcessId, ThreadId, SrcAddr, DstAddr, SrcPort, DstPort, Protocol(TCP) |
| 6 | UdpConnect | ProcessId, ThreadId, LocalAddr, RemoteAddr, LocalPort, RemotePort |
| 7 | TcpDisconnect | ProcessId, SrcAddr, DstAddr |
| 10 | TcpAccept | ProcessId, SrcAddr, DstAddr |

**Rust 封装设计**（`flowlens-attribution/src/etw_engine.rs`）：

```rust
pub struct EtwAttributor {
    session_handle: ETW_SESSION_HANDLE,
    active_connections: DashMap<ConnectionKey, ProcessInfo>,
    pid_timeline: DashMap<u32, i64>,  // pid → create_time
}

impl AttributionEngine for EtwAttributor {
    fn start(&mut self) -> anyhow::Result<()> {
        unsafe {
            EnableTrace(
                &TRACE_GUID,
                Some(etw_callback),
                &self.session_handle,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    fn lookup_connection(&self, conn: &ConnectionKey) -> Option<ProcessInfo> {
        // 查 DashMap，无则回退 IP Helper
        self.active_connections.get(conn).map(|v| v.value().clone())
    }
}
```

**性能优化**：
- 使用 `EventTraceProperties.LogFileMode = EVENT_TRACE_REAL_TIME_MODE`
- 回调函数仅做 `InterlockedCompareExchange` 更新 DashMap，避免阻塞
- 设置 `EnablePropertyGroup(WNODE_FLAG_EVENT_NAME)` 过滤掉无关事件

### 4.3 IP Helper API 回退

当 WFP 与 ETW 均不可用时（如受限环境），使用 `GetExtendedTcpTable2`。

**实现要点**：

```rust
#[cfg(windows)]
pub struct WindowsIpHelper;

impl IpHelperLookup for WindowsIpHelper {
    fn get_tcp_table(&self, family: AddressFamily) -> Vec<TcpConnection> {
        let mut buf = vec![0u8; 1024 * 1024];
        let mut size = buf.len() as u32;

        loop {
            let res = unsafe {
                GetExtendedTcpTable(
                    buf.as_mut_ptr() as *mut _,
                    &mut size,
                    TRUE,
                    family.into(),
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };
            if res == NO_ERROR { break; }
            if res != ERROR_INSUFFICIENT_BUFFER { return vec![]; }
            buf.resize(size as usize, 0);
        }

        // Parse MIB_TCPTABLE_OWNER_PID[]
        // 对每条记录：
        //   src_addr = addr_from_u32(table[i].dwLocalAddr)
        //   src_port = ntohs(table[i].dwLocalPort)
        //   dst_addr = addr_from_u32(table[i].dwRemoteAddr)
        //   dst_port = ntohs(table[i].dwRemotePort)
        //   pid = table[i].dwOwningPid
        //   state = TcpState::from(table[i].dwState)
        //   push to Vec<TcpConnection>
    }
}
```

---

## 6. Rust FFI 绑定设计

### 6.1 `flowlens-driver` Crate 接口

```toml
# crates/flowlens-driver/Cargo.toml
[package]
name = "flowlens-driver"
version = "0.1.0"
edition = "2021"

[dependencies]
windows-sys = { version = "0.52", features = ["Win32_Foundation", "Win32_System_Threading", "Win32_System_IO", "Win32_Storage_FileSystem"] }
thiserror = "1"
anyhow = "1"
```

### 6.2 Rust 侧驱动客户端

```rust
// crates/flowlens-driver/src/lib.rs
//! User-mode client for FlowLens WFP driver

mod wfp;
pub use wfp::*;

// crates/flowlens-driver/src/wfp.rs
use std::os::windows::io::{RawHandle, AsRawHandle};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, BOOL};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileA, GENERIC_READ, GENERIC_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Io::DeviceIoControl;

const DEVICE_PATH: &[u8] = b"\\\\.\\FlowLensWfp\0";

const IOCTL_GET_STATS: u32 = 0x80000401;
const IOCTL_ENABLE: u32    = 0x80000402;
const IOCTL_MAP_RING: u32  = 0x80000400;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct DriverStats {
    pub packets: u64,
    pub bytes: u64,
    pub dropped: u64,
    pub enabled: u32,
}

pub struct DriverClient {
    handle: HANDLE,
    running: AtomicBool,
}

impl DriverClient {
    pub fn open() -> anyhow::Result<Self> {
        let h = unsafe {
            CreateFileA(
                DEVICE_PATH.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null_mut(),
                OPEN_EXISTING as u32,
                0,
                0,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            anyhow::bail!("Cannot open FlowLensWfp device - driver not loaded / not admin?");
        }
        Ok(Self { handle: h, running: AtomicBool::new(false) })
    }

    pub fn enable(&self, on: bool) -> anyhow::Result<()> {
        let val: u32 = on as u32;
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle, IOCTL_ENABLE,
                &val as *const _ as *const _, 4,
                ptr::null_mut(), 0,
                &mut ret, ptr::null_mut()
            ) != 0
        };
        if !ok { anyhow::bail!("IOCTL_ENABLE failed"); }
        Ok(())
    }

    pub fn stats(&self) -> DriverStats {
        let mut s = DriverStats::default();
        let mut ret = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle, IOCTL_GET_STATS,
                ptr::null_mut(), 0,
                &mut s as *mut _ as *mut _, std::mem::size_of::<DriverStats>() as u32,
                &mut ret, ptr::null_mut()
            );
        }
        s
    }

    pub fn map_ring(&self) -> anyhow::Result<RingBuffer> {
        // 返回共享内存视图，后续调用 read_packets
        // 实现：Ioctl 传递 SECTION handle，ZwMapViewOfSection
        Ok(RingBuffer { /* 构造函数略 */ })
    }
}

impl Drop for DriverClient {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle); }
    }
}
```

### 6.3 集成到 `flowlens-capture`

```rust
// crates/flowlens-capture/src/lib.rs 追加 Engine 变体
pub enum Engine {
    Pcap(PcapEngine),
    Mock(MockEngine),
    Wfp(DriverClient),  // ← 新增
}

impl Engine {
    pub fn detect_wfp() -> Option<DriverClient> {
        DriverClient::open().ok()
    }
}
```
    }
}
```

**性能陷阱**：
- 全表扫描 O(N)，每秒轮询会导致 CPU 飙升
- 改进：仅在 `lookup_connection` 缺失时按需查询；或使用 `ChangeNotify` 事件触发刷新

### 4.4 PID 复用问题处理

**现象**：
- 进程 A (PID=1234) 创建连接 → 退出
- 进程 C (PID=1234) 启动 → 新连接
- 旧连接仍存在于内核表，归属错误

**防御策略**：

| 层级 | 机制 |
|------|------|
| WFP | `processCreateTime` 与连接建立时间比对；`createTime ≥ connTime` 才可信 |
| ETW | 维护 `pid_timeline`；收到 `ProcessCreate` 事件更新映射；查询时校验时间窗 |
| IP Helper | 无时间戳 → 标记为 `MatchQuality::Low`，UI 显示"可能过时" |
| 会话清理 | 主动删除 `TIME_WAIT` 超过 60s 的连接；定期扫 `last_seen > 120s` 条目 |

**Rust 端数据结构**：

```rust
#[derive(Clone)]
pub struct ProcessCacheEntry {
    pub pid: u32,
    pub create_time: i64,       // 纳秒
    pub exe_path: Option<PathBuf>,
    pub last_seen: i64,
    pub match_quality: MatchQuality,
}

pub struct AttributionCache {
    entries: DashMap<u32, ProcessCacheEntry>,
    conn_map: DashMap<ConnectionKey, u32>,  // conn → pid
}
```

**算法**：
```rust
fn lookup_process(&self, conn: &ConnectionKey) -> Option<ProcessInfo> {
    let pid = self.conn_map.get(conn)?.value().clone();
    let entry = self.entries.get(&pid)?;
    
    // 时间校验
    if entry.create_time > conn.timestamp + 1_000_000_000 {
        // 进程在连接之后才创建 → 不可能，标记可疑
        return None;
    }
    
    Some(ProcessInfo {
        pid,
        name: entry.exe_path.map(|p| p.file_name().unwrap()).unwrap().to_str().unwrap().into(),
        exe_path: entry.exe_path.clone(),
        match_quality: entry.match_quality,
        ..Default::default()
    })
}
```

---

- 服务端 accept 的连接（inbound flow）方向语义相反，flowCtx 中存
  `role = Initiator | Acceptor` 区分
- HTTP/2 多路复用、WebSocket 全落在同一 flow_id，交给 `flowlens-protocol`
  按 stream 拆分，会话聚合层不受影响
- UDP（QUIC）：无 flow established，用 `(pid, local, remote)` 组合 key，
  30 秒空闲过期（与 docs 中会话清理策略一致）

---

