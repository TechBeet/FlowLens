# ADR 0010: Unified Capture Pipeline & Event Model

**Status:** Proposed  
**Date:** 2025-01-01  
**Author:** FlowLens Lead Architect  
**Supersedes:** —  
**Related:** ADR 0001 (Tauri over Electron), 0002 (Svelte 5 runes), 0003 (event bus & batched UI), 0004 (CA & TLS interception), 0005 (PathProvider settings)

---

## 1. Context

FlowLens aims to surpass HTTP Debugger Pro by providing transparent, kernel-level packet capture with non-system-proxy HTTPS MITM decryption. The existing codebase has the following known issues:

- `get_session_detail` returns `None` (persistent placeholder; no persistence wired)
- `query_sessions` returns empty list (storage query not wired into Tauri commands)
- Process attribution shows PID 0 / `unknown` for most packets
- `WinDivert` default mode is closed; capturing only works when explicitly enabled
- HTTPS decryption is not end-to-end validated; certificate chain trust is incomplete
- UI layout and event rendering are inconsistent across capture volumes

This ADR proposes a unified architecture to resolve these issues by:

- Abstracting the capture engine behind a Rust `Engine` trait
- Enforcing strict TCP stream reconstruction
- Standardizing an SNI-aware MITM TLS flow using a local root CA
- Defining a centralized `TrafficEvent` model with process attribution
- Formalizing all Tauri IPC command interfaces
- Implementing session persistence with pagination

---

## 2. Goals

| # | Goal | Success Metric |
|---|---|---|
| 1 | Provide a unified `Engine` trait for packet capture | Two implementations (`WinDivertEngine`, `NpcapEngine`) compile and interchange |
| 2 | Ensure correct TCP stream reassembly | Streams ordered by sequence number; reordering/retransmission handled |
| 3 | Achieve non-proxy, SNI-aware MITM TLS decryption | Per-host leaf certs issued from local CA; SNI peeked before termination |
| 4 | Establish a centralized `TrafficEvent` model across all traffic types | All events carry `session_id`; process attribution no longer defaults to PID 0 |
| 5 | Define clear, deterministic Tauri IPC command interfaces | Each command has documented input/output JSON schemas |
| 6 | Implement session detail persistence and robust pagination | `get_session_detail` returns full session; `query_sessions` supports `pid`/`host`/`protocol`/limit/offset |
| 7 | Resolve process attribution via WFP / netfilter hooks | >95% of packets have a non-zero PID and process name |

Non-goals for this iteration: UDP payload inspection (QUIC/HTTP3 decode), macOS/Linux feature parity, TLS 1.3 decryption of ChaCha20-only stacks.

---

## 3. Architectural Design Decisions

### 3.1 Layered pipeline

```
+----------------------------------------------------------------------+
| UI (Svelte 5)  Header/Toolbar | Sidebar | FilterBar | TrafficTable    |
+----------------------------------------------------------------------+
+----------------------------------------------------------------------+
| Core: SessionStore | EventBus | PolicyEngine | Replay | Rewrite       |
+----------------------------------------------------------------------+
| Parsers: HTTP/1.1 | HTTP/2 | WebSocket | SSE | TLS records            |
+----------------------------------------------------------------------+
| Reconstruction: StreamReassembler (S5)                                |
+----------------------------------------------------------------------+
| Engine trait (S4): WinDivert | Npcap | Mock                             |
+----------------------------------------------------------------------+
| Kernel: WinDivert driver | Npcap LWF | WFP flow hooks                 |
+----------------------------------------------------------------------+
```

Layering rules:

1. **Strict downward dependency.** `Engine` knows nothing about TCP; parsers know nothing about WinDivert; the UI never touches a `RawPacket`.
2. **Async runtime.** All long-lived work runs on the Tokio runtime. Blocking driver calls (`WinDivertRecv`) run in `spawn_blocking`, or the engine exposes a non-blocking `recv_batch` so the caller can poll for shutdown.
3. **Single source of truth.** Only `EventBus` and `SessionStore` hold mutable shared state; every other component is a pure transformation `fn(T) -> SmallVec<U>`.
4. **Configuration.** All config comes from `PathProvider` (ADR 0005); no ambient env reads in production code paths.

### 3.2 Crate split

| Crate | Responsibility |
|---|---|
| `flowlens-engine` | `Engine` trait, `RawPacket`, WinDivert/Npcap/Mock implementations |
| `flowlens-reassembly` | `StreamReassembler`, `ConnectionKey`, `TcpStreamChunk` |
| `flowlens-mitm` | SNI peek, `DomainPolicy`, cert authority, rustls accept/connect |
| `flowlens-parse` | HTTP/1.1, HTTP/2, WS, SSE, TLS record framing |
| `flowlens-core` | `TrafficEvent`, `SessionStore`, `EventBus`, `Replay`, `Rewrite` |
| `flowlens-ffi` | Tauri commands, IPC DTOs, serde boundaries |
| `src/` (Svelte 5) | `SettingsView`, `TrafficTable`, `DetailPanel`, `ReplayHistory` |

---

## 4. Capture Engine Abstraction

### 4.1 Trait definition

```rust
// crates/flowlens-engine/src/lib.rs
pub enum CaptureMode { HttpOnly, AllIp }

pub struct EngineConfig {
    pub mode: CaptureMode,
    pub device: Option<String>,
    pub filter: Option<String>,     // user override, wins over mode-derived filter
    pub ring_buffer_kb: u32,        // WinDivert packet queue
    pub batch_size: usize,          // default 256
    pub direction: Direction,       // Bidirectional / Inbound / Outbound
}

pub trait Engine: Send + 'static {
    fn name(&self) -> &'static str;
    fn start(&mut self, cfg: &EngineConfig) -> Result<(), EngineError>;
    fn stop(&mut self) -> Result<(), EngineError>;
    fn is_running(&self) -> bool;

    fn recv_packet(&mut self, timeout: Duration) -> Result<Option<RawPacket>, EngineError>;
    fn recv_batch(&mut self, max: usize) -> Result<Vec<RawPacket>, EngineError>;

    fn send_packet(&self, bytes: &[u8], addr: Option<SocketAddr>) -> Result<usize, EngineError>;
}
```

Design notes:

- `start(&mut self, cfg)` takes configuration **rather than constructor arguments**, so one instance can switch `HttpOnly` <-> `AllIp` without tearing down the driver handle.
- `recv_batch` blocks up to the driver timeout and returns an **empty `Vec`** on timeout (not `Ok(None)`), so a single `while` loop can both drain and observe `is_running()`/shutdown without busy-waiting.
- `send_packet(&self)` (not `&mut self`) because the underlying WinDivert handle is shared behind `Arc` and can be written from the MITM task concurrently with `recv_batch`.
- `addr` is only meaningful for Npcap (which injects at L2 without a route table lookup); WinDivert ignores it.
- All errors are typed; no `String` leaks across the trait boundary.
### 4.2 Engine Matrix

| Engine | Mode | Intercept/Redirect | Inject/Modify | Needs Admin | Test Tier | Default |
|---|---|---|---|---|---|---|
| **WinDivertEngine** | HttpOnly/AllIp | Yes (MITM) | Yes (NAT/Checksum) | Yes | No | ✅ |
| **NpcapEngine** | HttpOnly/AllIp | No (Passive Copy Only) | No (Send via pcap_sendpacket) | No | No | ❌ |
| **MockEngine** | N/A | No | No | No | Yes | ⚙️ |

**Key Differentiation:** WinDivert supports full L3/L4 interception and transparent packet reinjection, which is *mandatory* for non-proxy HTTPS MITM. Npcap is strictly a passive capture tool; its utility here is limited to non-interception scenarios or specialized loopback capture.

### 4.3 WinDivertEngine (Default)

- **Handles:** Utilizes two handles: 1. `ProbeHandle` (Priority 1000) for passive mirroring (AllIp mode, or undecryptable flows); 2. `MitmHandle` (Priority 0) for active interception on decrypt-eligible flows.
- **Filtering:** Derived from `EngineConfig.mode` (e.g., HttpOnly: `(tcp.DstPort == 80 or tcp.SrcPort == 80 or tcp.DstPort == 443 or tcp.SrcPort == 443)`; AllIp: `"ip or ipv6"`).
- **MITM Flow:** Intercept -> **NAT Rewrite** (Dst $\rightarrow$ `127.0.0.1:<ephemeral>`) -> Checksum Recalc (`WINDIVERT_HELPER_NO_IP_CHECKSUM`) -> `WinDivertSend`.
- **Backpressure:** If the internal send queue reaches `max_queued_packets` (default 1024), the engine raises `EngineError::Backpressure` and the flow is immediately switched to **Passthrough** (fail-open), preventing traffic loss.
- **Elevation:** `start()` requires Admin rights; `EngineError::NeedsElevation` signals this, triggering a UI toast and "Run as Administrator" affordance.
- **Driver:** `WinDivert64.sys` and `WinDivert.dll` are vendored via `build.rs` from the `windivert/` resource directory; driver version pinned to 2.2.x.

### 4.3.1 NpcapEngine (Optional)

- **Build Dependency:** Requires `feature = "npcap"` and successful probing of `NPCAP_SDK_DIR` environment variable in `build.rs` to locate `wpcap.dll`/`Packet.lib`.
- **Function:** Primarily uses `pcap::Capture::open()` with `immediate_mode(true)` for efficient packet delivery to the Rust async runtime.
- **Limitation:** Cannot perform transparent redirection/termination for MITM; it is **capture-only** in this architecture.
- **Loopback:** Requires user to configure and select the Npcap Loopback Adapter in `list_devices()` output for local host traffic capture.

### 4.4 MockEngine

- **Usage:** Enabled via `#[cfg(any(test, feature = "mock"))]`.
- **Determinism:** Uses an injected `EngineClock` trait instead of `std::time::Instant`, allowing unit tests of §5 (Reassembler) and §6 (SNI) to run synchronously/paused via `tokio::test(start_paused = true)`.
- **Simulation:** `MockEngine::from_packets(Vec<RawPacket>)` or `MockEngine::from_script(Script)` where `Script` controls packet delivery, delays, and drops deterministically.
- **Verification:** `send_packet` records output into an internal buffer, allowing tests to assert precisely on the injected bytes, verifying the MITM logic without network dependency.

### 4.5 Factories

```rust
pub fn create_https_engine(mode: CaptureMode, device: Option<&str>) -> Result<Box<dyn Engine>, EngineError>;
pub fn create_all_ip_engine(device: Option<&str>) -> Result<Box<dyn Engine>, EngineError>;
pub fn create_npcap_engine(device: &str) -> Result<Box<dyn Engine>, EngineError>;
pub fn create_mock_engine(script: Script) -> Box<dyn Engine>;
```
- **Mapping:** `create_https_engine` $\implies$ WinDivert with both SNIFF (Probe) and INTERCEPT (MITM) handles based on domain rules.
- **Polymorphism:** All factories return `Box<dyn Engine>`; runtime checks (`engine.name()`) distinguish the implementation.
## 5. TCP Stream Reassembly

### 5.1 ConnectionKey and Direction

ConnectionKey carries the normalized 4-tuple (src, dst). Direction is decided by the first SYN: the side that initiates the connection is the client. The reassembler normalizes both directions so a single flow is keyed by (client, server) regardless of which side sent the segment.

### 5.2 StreamReassembler

```rust
pub struct TcpStreamChunk {
    pub key: ConnectionKey,
    pub direction: Direction,
    pub seq_start: u32,
    pub data: Vec<u8>,
    pub timestamp_micros: i64,
    pub flags: TcpFlags,
}

pub enum GapPolicy {
    EmitGap { gap_bytes: usize, reason: GapReason },
    DropGap,
}

enum GapReason { RetransmissionLoss, PacketLoss, Overflow }

pub struct StreamReassembler {
    flows: BTreeMap<ConnectionKey, TcpFlow>,
    sweep_interval: Duration,
}

struct TcpFlow {
    client_next_seq: u32,
    server_next_seq: u32,
    pending_client: BTreeMap<u32, Vec<u8>>,
    pending_server: BTreeMap<u32, Vec<u8>>,
    client_fin: bool,
    server_fin: bool,
    last_seen: Instant,
}
```

impl StreamReassembler {
    pub fn new(sweep_interval: Duration) -> Self;
    pub fn push(&mut self, pkt: &RawPacket) -> SmallVec<[TcpStreamChunk; 4]>;
    pub fn sweep(&mut self, now: Instant);
}
```

### 5.3 Reassembly Policy

- Seq arithmetic uses wrapping_sub helpers; wraparound is detected and handled.
- Retransmission: seq < expected => trim already-emitted portion; expected seq unchanged.
- Out-of-order: seq > expected => buffer in pending BTreeMap. Max pending entries MAX_PENDING (default 4096); overflow => EmitGap { reason: Overflow }.
- Contiguous drain: after insert, scan pending for lowest seq matching expected; emit chunks; advance expected.
- Timeout: last_seen updated per insert; sweep every 5s drops flows idle > 120s (client) / 60s (server).
- Half-close: FIN sets client_fin/server_fin; data up to FIN seq drained; trailing bytes after FIN become a gap per policy.
### 5.4 Decoupling Contract

The reassembler is a pure state machine: fn push(&mut self, pkt: &RawPacket) -> SmallVec<[TcpStreamChunk; 4]>. It knows nothing about WinDivert, Npcap, or the UI. Downstream consumers (HTTP/1.1 parser, HTTP/2 framing, TLS record decoder, event bus) subscribe to chunks.

Chunk variants:

```rust
pub enum TcpStreamChunk {
    Data { key, direction, seq_start, data, timestamp_micros },
    Gap { key, direction, gap_bytes, reason, timestamp_micros },
    Closed { key, direction, timestamp_micros },
}
```

Because gaps are explicit, the UI can render an incomplete body (truncated download) instead of silently showing a partial payload. Because closed is explicit, the session detail command can mark a session as finished with a reason.

### 5.5 Why not rely on the OS stack

Using the OS TCP stack (socket) would hide retransmissions and reordering, making it impossible to show network-level symptoms. Reassembling at the packet level lets FlowLens surface: retransmission counts, zero-window probes, keep-alive timeouts, and RST reasons. These become TrafficEvent::Packet and feed the right panel diagnostics.
## 6. SNI-Aware MITM TLS (no system proxy)

### 6.1 Hard constraint

FlowLens never writes WinINET/WinHTTP proxy settings, never installs a PAC file, never touches the Firefox prefs, and never registers an LSP/WFP callout driver. Decryption is achieved by transparent packet redirection, so applications that ignore system proxy settings (Go, Rust, Java, Electron with --no-proxy) are still decrypted. This is the primary differentiator against HTTP Debugger Pro and Fiddler Classic.

### 6.2 Peek ClientHello for SNI (non-consuming)

```rust
pub struct SniPeek { pub sni: Option<String>, pub alpn: Vec<String>, pub consumed: usize }

impl SniPeek {
    pub const MAX_BYTES: usize = 16 * 1024;
    pub fn from_client_hello(buf: &[u8]) -> Result<SniPeek, SniParseError>;
}
```

Rules:

1. Read from the intercepted socket into a PeekBuffer until a full TLS record is readable (record type 0x16, version 0x03 0x01..0x03, length prefix).
2. Parse handshake ClientHello: skip to extensions, walk extension type 0x0000 (server_name) and 0x0010 (ALPN).
3. The buffer is NOT consumed. It is handed to rustls via a PrefixedStream<S> adapter that replays the buffered bytes before delegating to the live socket.

### 6.3 DomainPolicy

```rust
pub struct DomainRule {
    pub domain: String,      // normalized: lowercase, no trailing dot
    pub decrypt: bool,
    pub wildcard: bool,      // *.example.com
}

pub struct DomainPolicy { rules: Vec<DomainRule> }   // ORDERED, first match wins
```

Matching semantics:

| Rule | Matches | Does NOT match |
|---|---|---|
| example.com, wildcard=false | example.com | api.example.com |
| *.example.com, wildcard=true | api.example.com, a.b.example.com | example.com (apex) |
| * (wildcard-only) | any SNI | requests with no SNI |

First match wins, which lets a user override a broad decrypt with a narrow passthrough rule by placing it earlier. Rules are persisted through ADR 0005 settings and reordered only via the UI drag handle.
### 6.4 SniDecision

```rust
pub enum SniDecision {
    Intercept { sni: String, rule_index: usize },
    Passthrough { reason: PassthroughReason },
}

pub enum PassthroughReason {
    NoSni,
    NoRuleMatched,
    RuleDisabled,
    NotDecryptable,   // e.g. UDP/QUIC, non-443, or no SNI
    CertError,        // CA not installed / cert generation failed
}
```

`SniDecision` is computed once per connection and logged in the TrafficEvent::Tls so the UI can explain why a flow was not decrypted.

### 6.5 Intercept path

1. Acquire `ServerConfig` from `CertAuthority::server_config_for_sni(&sni)` (LRU cache of 256 per-SNI configs).
2. Issue leaf cert with rcgen: SAN = [DnsName(sni)], CN = sni, validity 397 days, ECDSA P-256 key, random 20-byte serial, signed by the local CA.
3. Accept the client TLS connection with rustls (ALPN mirrored from the peeked ClientHello).
4. Open upstream TLS connection to the real server using the same ALPN; verifier = rustls-native-certs plus webpki-roots fallback; verification failures are recorded as TlsEvent::VerifyError rather than silently downgraded.
5. Splice decrypted bytes: client->server and server->client in two tokio tasks; feed into HTTP parsers.

### 6.6 Passthrough path

For passthrough decisions, open a plain TCP connection to the true destination and byte-relay both directions. Even though the original packet was redirected, the relay is byte-transparent and preserves the original bytes on the wire (only the TCP sequence numbers differ). Passthrough flows are still recorded as TrafficEvent::Tls with decrypted=false.

### 6.7 No system proxy mutation

- No registry writes under `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`.
- No `netsh winhttp set proxy`.
- No Firefox `prefs.js` edits.
- No environment variable injection into child processes.

Tests (tests/system_proxy_untouched.rs) snapshot the relevant state before/after a capture session and assert equality.
## 7. TrafficEvent Model and Process Attribution

### 7.1 Event model

```rust
pub enum TrafficEvent {
    Connection(ConnectionEvent),
    Http(HttpEvent),
    Tls(TlsEvent),
    Ws(WsEvent),
    Sse(SseEvent),
    Packet(PacketEvent),
    Closed(ClosedEvent),
    Dropped(DroppedEvent),
}

pub struct EventMeta {
    pub session_id: Option<u64>,
    pub timestamp_micros: i64,
    pub direction: Direction,
    pub process: Option<ProcessInfo>,
}
```

Every variant embeds an `EventMeta`. `session_id` is assigned by the core once a flow is correlated (may be None for early packets before correlation). `timestamp_micros` is captured at packet arrival, not at processing time.

### 7.2 ProcessInfo

```rust
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<PathBuf>,
    pub user_sid: Option<String>,
    pub arch: Arch,
    pub match_quality: MatchQuality,
}

pub enum Arch { X86, X64, Arm64, Unknown }
pub enum MatchQuality { Exact, HeuristicPort, Cached, Unknown }
```

### 7.3 Process attribution strategy

1. **Primary (Windows WFP).** Subscribe to ALE flow-established events via `FwpmNetEventSubscribe` / `FwpmFlowContextGet`. These carry the PID at connection time, before PID reuse can occur. Match quality = Exact.
2. **Fallback (GetExtendedTcpTable).** Poll `TCP_TABLE_OWNER_PID_ALL` every 250 ms and cache a map of 4-tuple -> PID with a 2 s TTL. Match quality = HeuristicPort. This covers the case where WFP is unavailable (older Windows, restricted policy).
3. **PID reuse guard.** Cache entries store the process creation FILETIME from `GetProcessTimes`. On lookup, re-verify the creation time matches; otherwise invalidate and re-resolve.
4. **Cache.** `HashMap<u32, Arc<ProcessInfo>>` with TTL + background refresh task. `sysinfo` is NEVER called in the hot path; it is only used by the refresh task to resolve names/exe/user_sid.
5. **No PID 0 leak.** When no attribution is available, `process` is `None` and the UI shows "System / unknown". PID 0 is never surfaced as a real process.

### 7.4 Platform trait

```rust
pub trait ProcessResolver: Send + Sync {
    fn resolve(&self, key: &ConnectionKey) -> Option<ProcessInfo>;
}
```

Windows implementation is the default; Linux/macOS are stubs returning None (scope is Windows-first for this ADR).
## 8. Tauri IPC Command Contracts

All commands are registered in `flowlens-ffi/src/commands.rs` and exposed via `tauri::generate_handler!`. Inputs/outputs are serde JSON. Errors are returned as `Result<T, AppError>` where `AppError` serializes to `{ code, message, hint? }`.

### 8.1 Capture lifecycle

| Command | Input | Output | Errors |
|---|---|---|---|
| `start_capture` | `{ mode: CaptureMode, device: Option<String> }` | `{ started_at: i64, engine: String }` | `EngineNotInstalled`, `NeedsElevation`, `DeviceNotFound`, `AlreadyRunning` |
| `stop_capture` | `{}` | `{ stopped_at: i64, packets: u64, sessions: u64 }` | `NotRunning` |
| `get_stats` | `{}` | `{ packets: u64, bytes: u64, sessions: u64, dropped: u64, retransmissions: u64 }` | - |
| `list_devices` | `{}` | `{ devices: Vec<DeviceInfo> }` | - |
| `clear_events` | `{}` | `{ cleared: u64 }` | - |

### 8.2 Sessions and replay

| Command | Input | Output | Errors |
|---|---|---|---|
| `get_session_detail` | `{ session_id: u64 }` | `Option<SessionDetail>` | `StoreError` |
| `query_sessions` | `{ pid: Option<u32>, host: Option<String>, protocol: Option<String>, limit: u32 (<=500), offset: u64 }` | `{ sessions: Vec<SessionSummary>, total: u64 }` | `InvalidLimit`, `StoreError` |
| `replay_request` | `{ session_id: u64 }` | `{ replay_id: u64 }` | `SessionNotFound`, `ReplayFailed` |
| `replay_request_with_delay` | `{ session_id: u64, delay_ms: u64 }` | `{ replay_id: u64 }` | `SessionNotFound`, `ReplayFailed` |
| `replay_batch` | `{ session_ids: Vec<u64>, delay_ms: Option<u64> }` | `{ replay_ids: Vec<u64> }` | `PartialFailure` |
| `pause_session` | `{ session_id: u64 }` | `{ paused: bool }` | `SessionNotFound`, `AlreadyPaused` |
| `release_session` | `{ session_id: u64 }` | `{ released: bool }` | `NotPaused` |
| `modify_session` | `{ session_id: u64, edits: SessionEdits }` | `{ modified: bool }` | `SessionNotFound`, `InvalidEdit` |
| `list_paused_sessions` | `{}` | `{ sessions: Vec<PausedSession> }` | - |
### 8.3 Decryption/CA Management

| Command | Input | Output | Errors |
|---|---|---|---|
| `add_rewrite_rule` | `{ rule: RewriteRule }` | `{ success: bool }` | `InvalidRule` |
| `remove_rewrite_rule` | `{ rule_id: u64 }` | `{ success: bool }` | `RuleNotFound` |
| `toggle_rewrite_rule` | `{ rule_id: u64, state: bool }` | `{ success: bool }` | `RuleNotFound` |
| `list_rewrite_rules` | `{}` | `{ rules: Vec<RewriteRule> }` | - |
| `list_decrypt_domains` | `{}` | `{ domains: Vec<DomainRule> }` | - |
| `add_decrypt_domain` | `{ rule: DomainRule }` | `{ success: bool }` | `InvalidDomain` |
| `remove_decrypt_domain` | `{ domain: String }` | `{ success: bool }` | `DomainNotFound` |
| `toggle_decrypt_domain` | `{ domain: String, state: bool }` | `{ success: bool }` | `DomainNotFound` |
| `issue_for_sni` | `{ sni: String }` | `{ cert_id: u64 }` | `SNINotFound`, `CertFailure` |
| `get_ca_pem` | `{}` | `{ pem_string: String }` | `NoCA` |
| `generate_ca` | `{ key_size: u32 (2048) }` | `{ ca_cert: String, ca_key: String }` | `CryptoError` |
| `install_ca` | `{ ca_pem: String }` | `{ success: bool }` | `IOError` |
| `uninstall_ca` | `{}` | `{ success: bool }` | `NoCA` |
| `check_ca_status` | `{}` | `{ status: CAStatus }` | - |

### 8.4 UI Configuration

| Command | Input | Output | Errors |
|---|---|---|---|
| `read_theme` | `{}` | `{ theme: Theme }` | - |
| `write_theme` | `{ theme: Theme }` | `{ success: bool }` | - |
| `set_shortcut` | `{ key: String, action: String }` | `{ success: bool }` | `InvalidKey` |
| `get_event_max` | `{}` | `{ max: u32 }` | - |
| `set_event_max` | `{ max: u32 }` | `{ success: bool }` | `InvalidMax` |
## 9. Session Persistence

### 9.1 SessionStore trait

```rust
pub trait SessionStore: Send + Sync + 'static {
    fn append(&self, event: TrafficEvent) -> Result<(), StoreError>;
    fn get(&self, id: u64) -> Result<Option<SessionDetail>, StoreError>;
    fn query(&self, q: Query) -> Result<QueryResult, StoreError>;
    fn delete(&self, id: u64) -> Result<(), StoreError>;
    fn clear(&self) -> Result<(), StoreError>;
}

pub struct Query {
    pub pid: Option<u32>,
    pub host: Option<String>,
    pub protocol: Option<String>,
    pub limit: u32,
    pub offset: u64,
}

pub struct QueryResult { pub sessions: Vec<SessionSummary>, pub total: u64 }
```

### 9.2 Implementations

- **RingStore (in-memory).** Fixed capacity (default 50,000 events, configurable via `set_event_max`). Backed by `Arc<RwLock<VecDeque<Arc<TrafficEvent>>>>`. O(1) append; eviction emits `TrafficEvent::Dropped { reason: Capacity }` so the UI can show the drop.
- **DiskStore (JSONL per run).** Append-only file at `$FLOWLENS_DATA/sessions/<run_id>.jsonl`. Each line is a serialized TrafficEvent. Index file `<run_id>.idx` holds `(session_id -> file_offset, length)` for O(1) random access. Rotate when file exceeds 256 MiB.
- **Hybrid (default).** RingStore for hot data; DiskStore for cold persistence; a background flush task drains RingStore to DiskStore every 5 s or 1000 events, whichever comes first.

### 9.3 SessionDetail schema

```rust
pub struct SessionDetail {
    pub id: u64,
    pub meta: EventMeta,
    pub request: HttpRequest,
    pub response: Option<HttpResponse>,
    pub tls: Option<TlsInfo>,
    pub websocket_frames: Vec<WsFrame>,
    pub sse_events: Vec<SseEvent>,
    pub packets: Vec<PacketRef>,
    pub timing: SessionTiming,
    pub replay_history: Vec<ReplayResult>,
}
```

### 9.4 Pagination

- `limit` clamped to [1, 500]; `offset` is a byte-offset into the index, not a row offset, to remain stable under concurrent appends.
- `total` is computed from the index without materializing rows.
- Filter by `pid`/`host`/`protocol` uses the index columns (pid, host, protocol) stored in the index, so queries never scan the full event log.
## 10. UI Architecture (Svelte 5)

### 10.1 Layout

Three-column layout driven by CSS grid:

```
+--------------------------------------------------------------+
| Header / Toolbar (capture toggle, mode, device, CA, theme)    |
+--------------+-----------------------------+-----------------+
| Sidebar      | FilterBar + TrafficTable    | DetailPanel     |
| (device,     | (virtualized, 50k rows)     | (request/response,
|  mode, CA,   |                             |  TLS, WS, SSE,
|  domains,    |                             |  replay history,
|  SNI)        |                             |  process info)  |
+--------------+-----------------------------+-----------------+
| StatsPanel (packets, bytes, sessions, dropped, retransmits)   |
+--------------------------------------------------------------+
```

### 10.2 Components

- `Header`, `Toolbar`: capture toggle, mode selector, device picker, CA status badge, theme toggle.
- `Sidebar`: device list, capture mode, CA install/uninstall, decrypt domain list (drag-reorder), SNI rules.
- `FilterBar`: text search, protocol filter, process filter, time range, host filter.
- `TrafficTable`: virtualized list (windowing), row selection, multi-select for batch replay, column resize.
- `DetailPanel`: request/response bodies with syntax highlighting, TLS cert chain, WS/SSE streams, process info, replay button.
- `RightPanel`: replay history, paused sessions, rewrite rules.
- `SettingsView`: theme, shortcuts, event max, export preferences.
- `StatsPanel`: live counters updated from `get_stats` polling (1 Hz) or pushed via event.

### 10.3 State management

- Svelte 5 runes: `$state`, `$derived`, `$effect`.
- Stores live in `src/lib/stores/*.svelte.ts` and are imported as modules.
- `eventStore`: holds the ring buffer of `TrafficEvent`s in `$state`; appended via `listen('traffic-event')`.
- Batching: the backend emits `traffic-event` in batches of up to 256; the frontend flushes into the store inside `requestAnimationFrame` to avoid re-render storms at >10k events/s.
- `shortcutRegistry`: maps key combos to actions; persisted via `read_shortcuts`/`write_shortcuts`.

### 10.4 Theme

- Dark/light via CSS variables (`--bg`, `--fg`, `--accent`, `--border`, `--code-bg`).
- Persisted via `read_theme`/`write_theme`.
- Respects OS preference on first run (`prefers-color-scheme`).
## 11. Build & Verification

### 11.1 Cargo / Rust

| Check | Command | Expected |
|---|---|---|
| `cargo check --workspace` | `cargo check --workspace` | Zero errors, zero warnings |
| `cargo test --workspace` | `cargo test --workspace` | All unit + integration tests pass (≈ 120 tests) |

### 11.2 Tauri / UI

| Check | Command | Expected |
|---|---|---|
| `npm run build` | `npm run build` | Tauri bundle build succeeds, theme CSS extracted |
| `npm run tauri dev` | `npm run tauri dev` | App window launches, capture UI renders, device list populated |
| `npm run tauri build` | `npm run tauri build` | Windows MSI produced in `src/tauri/target/...` |

### 11.3 End-to-end

| Step | Action | Success Criteria |
|---|---|---|
| 1. Start capture (AllIp) | Traffic events begin flowing | `get_stats.pkts > 0` within 2 s |
| 2. Generate real HTTPS traffic (curl https://example.com) | TLS handshake visible | SNI = `example.com` appears in first ClientHello (recorded in TrafficEvent::Tls) |
| 3. Verify decrypted payload | Intercept path activates | HTTP GET request line visible in `DetailPanel` (e.g., `GET /index.html HTTP/1.1`) |
| 4. Switch to passthrough for a domain | Domain rule toggled to `decrypt=false` | TrafficEvent::Tls.decrypted=false; DetailPanel shows binary, no HTTP line |
| 5. Export HAR | `export_har` command | JSON file valid HAR 1.2 with all captured requests/responses |
| 6. Session persistence | Stop capture, restart app | All sessions reappear via `query_sessions`, pagination correct |
| 7. CA management | `install_ca` then `check_ca_status` | CA cert installed and status `Installed`; UI badge green |
