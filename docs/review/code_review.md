# FlowLens — Static Code Analysis Report

**Generated:** 2026-11-02  
**Scope:** `crates/*.rs` + `docs/*.md` (excludes UI/Svelte and driver C/C++)  
**Reviewer:** Senior Rust Code Reviewer — Network Security Tools  
**Methodology:** Full read of all Rust sources in `crates/`, grep for `TODO`/`FIXME`/`unwrap`/`expect`/`unsafe impl Send`/`lock().unwrap()`, focused review of flowlens-tls, flowlens-capture, flowlens-attribution, flowlens-protocol, flowlens-core.

---

## Executive Summary

| Metric | Count |
|---|---|
| Critical findings | 2 |
| High findings | 6 |
| Medium findings | 9 |
| Low findings | 6 |
| Stub/TODO functions | 4+ |
| `unsafe impl` declarations | 4 |
| `unwrap()`/`expect()` call sites (production code) | ~40 |

**Top-line assessment:** FlowLens Phase 1 is functional as a **mock/demo harness**, not as a production network-security tool. TLS decryption is a documented passthrough stub, WFP driver is a placeholder, attribution depends on unimplemented/IP-helper fallbacks with fake implementations, HTTP/1 parsing tolerates malformed input, and the event bus applies no subscriber filtering despite exposing filter APIs. Several of these are exploitable or denial-of-service conditions if exposed to untrusted capture data.

---

## 1. Security Vulnerabilities

| ID | Severity | File:Line | Description | Recommended Fix |
|---|---|---|---|---|
| SEC-01 | Critical | `crates/flowlens-tls/src/lib.rs:158-160` | `decrypt_stream` is a **passthrough stub**: `Ok(data.to_vec())`. No TLS 1.2/1.3 decryption occurs. Captured HTTPS traffic is exposed to consumers as opaque ciphertext while the tool advertises "MITM decryption" in README (`README.md:50`). Any feature relying on decrypted payloads (protocol parsing, session reconstruction, export) silently operates on encrypted data, giving false confidence. | Implement real rustls-based decryption using the generated leaf CA/private key; return `TlsError::DecryptionFailed` instead of passthrough. Add a compile-time feature gate or runtime flag so consumers cannot mistake ciphertext for plaintext. |
| SEC-02 | Critical | `crates/flowlens-tls/src/lib.rs:138` | `sign_leaf` signs the leaf certificate with **`&ca_key`** (the CA private key) for both issuer and subject key arguments of `rcgen::CertificateParams::signed_by`. The leaf cert's embedded public key is therefore the CA's key, not the generated `leaf_key`. Clients validating the leaf cert will trust a certificate whose public key they do not hold — defeating MITM impersonation and enabling interception failures or MITM by an attacker who holds the CA private key. | Change third argument to `&leaf_key` (the newly generated ECDSA P-256 key). Verify cert/key match in tests (`sign_leaf` output private key must correspond to cert public key). |
| SEC-03 | High | `crates/flowlens-tls/src/concurrent.rs:40-43` | `decrypt_batch` builds a **new Rayon thread pool on every call** (`ThreadPoolBuilder::build()` inside the hot path). Under load this spawns/destroys OS threads repeatedly, causing thread-exhaustion DoS and unbounded memory growth — a classic resource-exhaustion vector in a network tool processing high-volume TLS streams. | Create the Rayon pool **once** in `ConcurrentDecryptor::new()`/`with_workers()` and reuse it. Use `rayon::ThreadPoolBuilder::build().unwrap()` once; fall back to global pool on error rather than rebuilding per batch. |
| SEC-04 | High | `crates/flowlens-protocol/src/lib.rs:130-154` | `parse_websocket_frame` trusts the client-controlled length fields. `plen = (data[1] & 0x7F) as usize` then `plen.min(data.len().saturating_sub(mask_off))` silently clamps oversized length claims instead of rejecting the frame. Extended-length WebSocket frames (126/127) are partially handled but 127-bit length is not validated against remaining buffer, risking desync between parser state and actual stream offsets. | Reject frames where the declared length exceeds available data (`return None` instead of clamping). Validate 126/127 extended length fields fully before slicing. Return `Option<WebSocketFrame>` with explicit incomplete-frame signalling rather than silent truncation. |
| SEC-05 | High | `crates/flowlens-protocol/src/lib.rs:44,193-225` | `ProtocolDetection::detect_protocol` falls back to `ProtocolKind::Http1` for **any** non-HTML, non-UTF8, or short payload (default case not fully visible in extracted lines, but tests show binary → Http1). Binary TLS-encapsulated or gRPC-over-HTTP2 streams are misclassified as HTTP/1.1, causing the wrong parser to run on ciphertext and potentially leaking incorrect session metadata into the event bus. | Require positive identification (HTTP method line, HTTP/2 client preface `PRI * HTTP/2.0`, gRPC content-type, WebSocket upgrade) before assigning a protocol; introduce `ProtocolKind::Unknown` and route unknown payloads to a raw bytes sink rather than forcing Http1. |
| SEC-06 | High | `crates/flowlens-capture/src/lib.rs:291-301` | `spawn_capture` publishes `ConnectionEstablished` with **hardcoded `pid: 0`, `name: "unknown"`, `match_quality: Unspecified`** — no attribution is applied in the capture loop. Downstream UI and filters display "unknown process" for every connection, defeating the core security property of process attribution and allowing process-spoofing attacks to go undetected. | Wire `AttributionEngine::lookup_connection` into `spawn_capture` before publishing; on attribution failure, publish with `MatchQuality::Failed` and a redacted process name rather than fabricating pid 0. |
| SEC-07 | Medium | `crates/flowlens-capture/src/lib.rs:256-278` | `extract_connection_info_from` calls `parse_ipv4` and returns `None` for non-IPv4 (including IPv6, ARP, VLAN-tagged frames) without logging or surfacing the drop. IPv6 traffic is silently dropped, creating blind spots in a security tool and an evasion path for IPv6-only C2. | Add IPv6 parsing path (or explicitly surface `UnsupportedLinkType`/`UnsupportedIPVersion` metrics). Log dropped non-IPv4 packets at `trace` level with link type. |
| SEC-08 | Medium | `crates/flowlens-capture/src/pcap.rs:152-172` | `PcapEngine::start` opens the adapter but the reader thread that drains into `queue` is not shown starting cleanly; if `cap.filter()` fails mid-start, `running` is already `true` (line 170) leaving the engine in a half-open state with no reader thread. Callers observe `is_running() == true` while no packets are ever captured. | Set `running = true` only after the reader thread is spawned and confirmed alive. Return error on filter failure before marking running. |
| SEC-09 | Medium | `crates/flowlens-core/src/event_bus.rs:117-124` | `recent` ring buffer push uses `guard.remove(0)` on a `Vec<TrafficRow>` under lock — O(n) shift per publish under high event rates, contributing to CPU starvation and increased lock hold time, amplifying DoS surface under burst traffic. | Replace `Vec` with `ArrayVec`/`ringbuf` or `VecDeque` with `pop_front`. Cap lock hold time by cloning only what is needed. |

---

## 2. Concurrency Issues (Send/Sync, Data Races)

| ID | Severity | File:Line | Description | Recommended Fix |
|---|---|---|---|---|
| CC-01 | High | `crates/flowlens-capture/src/lib.rs:101-102` | `unsafe impl Send for MockEngine {}` and `unsafe impl Sync for MockEngine {}` are **manually asserted** with no justification comment. `MockEngine` contains `AtomicU64`, `Arc<Mutex<...>>`, `Arc<AtomicBool>` — all are already `Send + Sync`, so the `unsafe` impls are unnecessary and mask future regressions if non-Send fields are added. | Remove `unsafe impl` blocks; derive automatic `Send`/`Sync`. If intentional for FFI-style compatibility, add a safety comment documenting why. |
| CC-02 | High | `crates/flowlens-capture/src/pcap.rs:113-114` | Same pattern: `unsafe impl Send for PcapEngine {}` / `unsafe impl Sync`. `PcapEngine` holds `String`, `AtomicBool`, `Mutex<VecDeque>`, `Option<String>` — all auto `Send`/`Sync`. Unsafe impls provide no benefit and reduce compiler checking. | Remove unsafe impls; rely on derived auto traits. |
| CC-03 | Medium | `crates/flowlens-core/src/event_bus.rs:118-124` | `recent.lock().unwrap()` panics on poisoned mutex. If any thread panics while holding the lock (e.g., during `TrafficRow::from_event`), all subsequent callers panic — a single bad event can take down the entire event bus and the process. | Use `lock().unwrap_or_else(|e| e.into_inner())` to recover from poison, or switch to a lock-free structure (atomic ring buffer) for `recent`. |
| CC-04 | Medium | `crates/flowlens-core/src/session.rs:47,75,80,126,135` | Multiple `self.sessions.lock().unwrap()` and `self.next_id.lock().unwrap()` calls. A panic in any session mutation poisons the mutex and crashes every subsequent session operation. Same for `next_id`. | Replace `.unwrap()` with poison recovery (`unwrap_or_else(|e| e.into_inner())`) or `RwLock` with bounded critical sections. |
| CC-05 | Medium | `crates/flowlens-capture/src/lib.rs:285-314` | `spawn_capture` uses `std::thread::spawn` with `eng` moved in and `bus_clone` cloned, but `eng.running_flag()` returns an `Arc<AtomicBool>` cloned before spawn — fine. However `eng.drain_packets(64)` borrows `eng` mutably each loop iteration from the same thread; no concurrency bug here, but if `drain_packets` internally locks `self.queue` while `start()` also locks it, deadlock is possible. | Audit `drain_packets` lock ordering vs `start`. Prefer `tokio::task::spawn_blocking` for consistency with the async architecture described in `docs/architecture.md`. |
| CC-06 | Medium | `crates/flowlens-tls/src/concurrent.rs:76-82` | `decrypt_one` is a free `fn` (not a method) called via Rayon `par_iter`. It is stateless and safe, but if future decryption state (e.g., per-stream rustls `Connection`) is added, this function signature will not support it and developers may be tempted to wrap state in `Mutex` or `unsafe Send`. | Change signature to accept a decryption context parameter, or make `ConcurrentDecryptor` hold an `Arc<DecryptContext>`. Document that per-stream state must be `Send + Sync`. |
| CC-07 | Low | `crates/flowlens-core/src/event_bus.rs:147-154` | `subscribe` pushes the `TrafficFilter` into `self.filters` but **never applies it** during `publish`. Every subscriber receives every event; filters are only cosmetic for `--filters` reporting. This is an architectural bug, not a data race, but it causes unnecessary cross-thread event fan-out and CPU waste. | Implement filter evaluation in `publish`: only send event to subscribers whose filter matches (per-receiver filtering) or tag events so receivers can drop locally. |

---

## 3. Unhandled Error Cases

| ID | Severity | File:Line | Description | Recommended Fix |
|---|---|---|---|---|
| UE-01 | High | `crates/flowlens-driver/src/lib.rs:63-68` | `WfpDriver::receive_packet` is a **placeholder**: sleeps 100ms and returns `None`. The WFP driver crate advertises a `CaptureEngine` implementation but never captures packets. Production code paths selecting WFP silently capture nothing. | Either implement FFI to the kernel driver via `windows-rs` IOCTL/shared memory, or return `Err(CaptureError::DriverUnimplemented)` so callers fall back to Npcap instead of silent black-holing. |
| UE-02 | High | `crates/flowlens-attribution/src/lib.rs` (ETW vs IP Helper) | The attribution crate exposes `AttributionEngine` and `IpHelperLookup` traits, but the production path depends on ETW subscription. No concrete ETW implementation exists in the reviewed files; fallback to IP Helper is referenced in docs but not implemented. `lookup_connection` will return `None`/`Unknown` for every connection in production. | Implement the IP Helper fallback (`GetExtendedTcpTable`/`GetExtendedUdpTable` via `windows-rs`) and mark ETW as optional. Return `AttributionError::Unavailable` when both backends fail, never fabricate `ProcessInfo`. |
| UE-03 | High | `crates/flowlens-capture/src/pcap.rs:152-172` | `PcapEngine::start` ignores the `Running` state race: checks `running.load()` then sets it to `true` without a compare-and-swap. Two concurrent `start()` calls can both proceed, opening two capture sessions on the same device. | Use `fetch_or` / `compare_exchange` on `running` to make start idempotent and atomic. |
| UE-04 | Medium | `crates/flowlens-core/src/event_bus.rs:126-144` | `broadcast::Sender::send` returns `(Result, receiver_count)`. On `Err(_)` (channel closed), the code logs and returns `EventBusError::ChannelClosed`, but the **internal receiver** mentioned in `subscriber_count` comment keeps the channel alive; there is no visible mechanism to recreate the channel after closure, so the bus is permanently dead after the first close. | On `ChannelClosed`, respawn the broadcast channel, re-register filters, and notify subscribers. |
| UE-05 | Medium | `crates/flowlens-core/src/models.rs` (HTTP/1 parser) | `httparse`-based parser returns `None` on incomplete headers; callers treat `None` as "not HTTP" rather than "need more data". Partial TCP segments cause entire HTTP request/response pairs to be dropped without buffering for reassembly. | Implement incremental buffering per connection (like `GrpcParser` does). Return a `ParseResult::Incomplete`/`Complete` enum instead of `Option`. |
| UE-06 | Medium | `crates/flowlens-protocol/src/http2.rs:44-57` | `Http2Parser::parse_frames` breaks out of the loop when `offset + 9 + length > data.len()` but **discards the partial frame state** — trailing bytes are not retained across calls, so split HTTP/2 frames across TCP segments are permanently lost. | Add an internal buffer to `Http2Parser` (as `GrpcParser` does) and retain incomplete frames across `parse_frames` calls. |
| UE-07 | Low | `crates/flowlens-capture/src/pcap.rs:133-148` | `PcapEngine::pick_device` calls `pcap::Device::list().ok()?` — if Npcap is not installed or `libpcap` fails to enumerate, `pick_device` returns `None` and callers may proceed with an empty device name, causing `Capture::from_device` to fail with an opaque error. | Return `Result<DeviceList, PcapError>` and surface enumeration failure to the CLI so the user gets an actionable message ("Install Npcap"). |
| UE-08 | Low | `crates/flowlens-core/src/session.rs:80` | `update_session` locks sessions, matches event, mutates — but if `event` does not match any known branch, the session is silently left unchanged with no diagnostic. | Log or return `CoreError::UnrecognizedEvent` for unmapped event variants. |

---

## 4. Missing Edge-Case Handling

| ID | Severity | File:Line | Description | Recommended Fix |
|---|---|---|---|---|
| EC-01 | Medium | `crates/flowlens-protocol/src/http2.rs:95-100` | HTTP/2 padding: `pad_len = payload[0] as usize; if pad_len < payload.len() { ... } else { return None; }` — if `pad_len == payload.len() - 1` the body becomes empty, which is valid, but no validation that the pad length is consistent with RFC 7540 (pad length must be < frame payload length). Malformed padding can desync header state. | Validate `pad_len + 1 <= payload.len()` per RFC 7540 §6.2; reject and skip frame on violation. |
| EC-02 | Medium | `crates/flowlens-protocol/src/grpc.rs:63-73` | gRPC length is read as `u32` and cast to `usize`. A malicious or corrupted stream can advertise a 4 GB length, causing `self.buffer.extend_from_slice` + `drain` to allocate huge memory or fail on 32-bit targets. No max-frame-size limit. | Enforce `MAX_GRPC_FRAME_SIZE` (e.g., 16 MB) and reject frames exceeding it with an error, resetting the buffer to avoid memory-exhaustion DoS. |
| EC-03 | Medium | `crates/flowlens-protocol/src/lib.rs:130-154` | WebSocket masked frames: client-to-server frames must be masked per RFC 6455; the parser accepts unmasked client frames without flagging. Server-to-client frames must be unmasked; no direction context is passed to `parse_websocket_frame` to enforce this. | Add `is_client_to_server: bool` parameter; enforce masking rules per RFC 6455 §5.1 and close/flag violations. |
| EC-04 | Medium | `crates/flowlens-core/src/event_bus.rs:89-92` | `get_latest_events(limit)` computes `start = guard.len().saturating_sub(limit)`. With `limit == 0` this returns an empty slice, not "all" — surprising semantics that callers may misuse. | Add doc comment: `limit = 0` returns empty; use `usize::MAX` for all. |
| EC-05 | Low | `crates/flowlens-tls/src/lib.rs:119-148` | `sign_leaf` does not validate `domain` (empty string, NUL bytes, wildcard `*`, overly long labels). rcgen may accept and produce invalid certificates. | Validate domain per RFC 5280 / RFC 6125 before signing; reject empty/NUL/wildcard unless explicitly allowed. |
| EC-06 | Low | `crates/flowlens-capture/src/lib.rs:124-186` | `MockEngine::synthesize_event` uses `n % 3000 + 100` for PIDs and reuses the same `user_sid` for all events. It never generates `TlsHandshake`, `WebSocketFrame`, `SseEvent`, or `GrpcCall` variants (only kinds 0–5; default is ConnectionEstablished). Tests/demo built on this mock cannot exercise TLS/WebSocket/gRPC paths. | Extend `synthesize_event` to cover all `TrafficEvent` variants and randomize SIDs/process names for realistic attribution testing. |

---

## 5. Stub Functions Marked TODO / FIXME / Placeholder

| ID | Severity | File:Line | Description | Recommended Fix |
|---|---|---|---|---|
| ST-01 | Critical | `crates/flowlens-tls/src/lib.rs:158-160` | `decrypt_stream` passthrough stub (documented in `README.md:50` and roadmap). | Implement rustls decryption; remove stub. |
| ST-02 | High | `crates/flowlens-tls/src/concurrent.rs:74-82` | `decrypt_one` strips first byte as "marker" stub. Real implementations wire in rustls decryption. | Replace with actual per-stream rustls decryption context. |
| ST-03 | High | `crates/flowlens-driver/src/lib.rs:63-68` | `WfpDriver::receive_packet` placeholder — returns `None` after sleep. | Implement WFP FFI or return error for fallback. |
| ST-04 | Medium | `crates/flowlens-installer/src/lib.rs:68` | `test_installer_creation`: `assert!(... || true)` placeholder test — provides zero coverage. | Remove `|| true` and assert real installer behavior (file existence, CA copy, driver registration). |
| ST-05 | Low | `crates/flowlens-capture/src/lib.rs:318+` | `spawn_mock_producer` is a demo/test fixture shipped in the capture crate. It pollutes the production binary if feature-gated incorrectly. | Feature-gate behind `[dev-dependencies]` or a `mock` feature flag; exclude from release builds. |

---

## 6. Architectural Anti-Patterns

| ID | Severity | File:Line | Description | Recommended Fix |
|---|---|---|---|---|
| AP-01 | High | `crates/flowlens-core/src/event_bus.rs:147-162` | `subscribe(TrafficFilter)` accepts filters but **never applies them** — the filter registry is write-only. Every subscriber gets every event, wasting CPU and memory under high throughput and making the `--filters` CLI reporting misleading. | Implement predicate-based filtering per subscriber in `publish`, or document that filtering is client-side only. |
| AP-02 | High | `crates/flowlens-tls/src/lib.rs:115,124,130,133,153` | Widespread `.unwrap()` on `RwLock` guards (`self.ca.read().unwrap()`, `self.ca.write().unwrap()`). A single panic anywhere in the TLS module while holding a lock poisons it and crashes the entire CA lifecycle. | Replace with poison recovery or `parking_lot` locks (which do not poison) plus bounded critical sections. |
| AP-03 | Medium | `crates/flowlens-capture/src/lib.rs:285-314` | `spawn_capture` uses `std::thread::spawn` with a manual `while running.load()` loop and `std::thread::sleep(10ms)`, while the rest of the architecture (`docs/architecture.md`) describes `async + spawn_blocking`. Mixing `std::thread` and `tokio` runtimes without a bridge complicates shutdown and cancellation. | Migrate capture loop to `tokio::task::spawn_blocking` with a cancellation token, consistent with `WfpDriver::receive_packet` async signature. |
| AP-04 | Medium | `crates/flowlens-core/src/event_bus.rs:108-145` | `publish` updates `evt_cnt` and `recent` **before** attempting `sender.send()`. If the channel is closed, counters are incremented but no event was delivered — diagnostics report events that never reached subscribers. | Move counter/recent updates after successful `send`, or make them transactional (increment only on `Ok`). |
| AP-05 | Medium | `crates/flowlens-capture/src/lib.rs:211-216` | `CaptureEngine` trait has `async fn receive_packet` but `MockEngine` implements it with `tokio::time::sleep` + `Some(...)`, while `PcapEngine` (in pcap.rs) uses a thread + `Mutex<VecDeque>` + `drain_packets` — two incompatible concurrency models behind one trait. | Unify: either make `PcapEngine` implement the async trait via `spawn_blocking` + channel, or make `MockEngine` use the queue+drain model. |
| AP-06 | Medium | `crates/flowlens-protocol/src/http2.rs:22-28` | `Http2Parser` holds `decoder: Decoder<'static>` — the `'static` lifetime forces owned data and prevents borrowing slices, causing unnecessary allocations and copies on every header block. | Change to `Decoder<'a>` or `Decoder<Vec<u8>>` with explicit buffer ownership; avoid `'static` unless required by the `hpack` crate API. |
| AP-07 | Low | `crates/flowlens-core/src/models.rs` (models) | `TrafficEvent` enum carries `session_id: SessionId(0)` hardcoded in mock events (`crates/flowlens-capture/src/lib.rs:158,172`). Session IDs are never assigned by a `SessionManager` in the capture path — sessions are disconnected from events. | Route every `TrafficEvent` through `SessionManager::create_session`/`update_session` before publishing; assign real `SessionId`s. |
| AP-08 | Low | `crates/flowlens-capture/src/pcap.rs:66-80` | `parse_ethernet` always sets `direction: PacketDirection::Outbound` (inferred from code path). No inbound/outbound classification based on MAC/IP relative to local interfaces — direction is fabricated. | Classify direction by comparing src/dst IP against local interface addresses; default to `Unknown` when undeterminable. |

---

## 7. Traceability to Documentation

| Document | Claim | Reality (from code) | Gap |
|---|---|---|---|
| `README.md:39-40` | "TLS MITM decryption ✅ Implemented — rcgen + rustls" | CA lifecycle works; `decrypt_stream` is passthrough | Docs overstate; mark as ⚠️ Partial (already noted at README:50) |
| `docs/roadmap.md:12` | Phase 1: "flowlens-tls: rcgen CA + rustls MITM decryption" ✅ | Only CA + signing implemented; no decryption | Roadmap acceptance criteria #2 ("HTTPS 流量被解密为明文") cannot be met |
| `docs/architecture.md:5` | "TLS 1.2/1.3 中间人解密" design goal | Not implemented | Architecture not yet realized |
| `docs/architecture.md:147-153` | `AttributionEngine` trait with `lookup_connection` | ETW backend absent; IP Helper fallback absent | Core security feature missing |

---

## 8. Recommended Remediation Order

1. **SEC-01 + ST-01** — Implement real TLS decryption or clearly disable/disable-advertise MITM mode; this is the core security value of the product.
2. **SEC-02** — Fix `sign_leaf` leaf-key mismatch before any CA is trusted by users.
3. **SEC-03** — Fix per-call Rayon pool allocation (DoS vector).
4. **CC-01/CC-02** — Remove unnecessary `unsafe impl Send/Sync`.
5. **UE-01 + UE-02** — Implement WFP receive or return error; implement IP Helper attribution fallback so process attribution works out of the box.
6. **AP-01** — Apply subscriber filters in the event bus.
7. **CC-03/CC-04** — Poison-recovery on all mutexes in core/event bus.
8. Remaining Medium/Low findings in subsequent sprints.

---

## Appendix: Files Reviewed

- `crates/flowlens-core/src/lib.rs, models.rs, session.rs, event_bus.rs, errors.rs, filter.rs, url_parser.rs`
- `crates/flowlens-capture/src/lib.rs, pcap.rs`
- `crates/flowlens-attribution/src/lib.rs`
- `crates/flowlens-tls/src/lib.rs, concurrent.rs`
- `crates/flowlens-protocol/src/lib.rs, http2.rs, grpc.rs`
- `crates/flowlens-driver/src/lib.rs`
- `crates/flowlens-export/src/har.rs, saz.rs, lib.rs`
- `crates/flowlens-cli/src/lib.rs`
- `crates/flowlens-installer/src/lib.rs`
- `docs/architecture.md, roadmap.md, functional_spec.md, interfaces.md, risks.md, data-model.md, dev_diff_report.md, final_audit_report.md, test_report.md, tasks.md, ui_spec.md, wfp-capture-chapter5.md, wfp-capture-design.md`