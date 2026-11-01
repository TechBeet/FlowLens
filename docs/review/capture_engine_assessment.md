# FlowLens Capture Engine Assessment

## 1. Verified Current State

- **PcapEngine** (`crates/flowlens-capture/src/pcap.rs`): Uses the `pcap` crate; reader thread pushes into a `Mutex<VecDeque>` capped at 4096, dropping the oldest on overflow. `parse_ethernet` hardcodes `direction=Outbound`. No TCP reassembly, no loopback config, no BPF filter for port 443.
- **WfpDriver** (`crates/flowlens-driver/src/lib.rs`): PLACEHOLDER. `receive_packet()` sleeps 100 ms and returns `None`.
- **driver/src/callout.c**: 40-line skeleton (`DriverEntry` + empty IOCTL). No `FwpmEngineOpen`, no callout registration, no ring buffer.
- **Attribution** (`crates/flowlens-attribution/src/lib.rs`): `FakeIpHelper` returns hardcoded PID 1234. No `GetExtendedTcpTable`, no ETW.

## 2. Npcap Limitations

- **Loopback capture requires the Npcap Loopback Adapter.** On Windows the raw loopback interface is not exposed to user-mode capture by default. Npcap installs a virtual adapter (`Npcap Loopback Adapter`) and redirects `127.0.0.0/8` traffic through it; without it, localhost traffic is invisible. This also means loopback capture is opt-in at install time and adds an extra virtual NIC that can confuse routing/metrics tools.
- **No process attribution.** Npcap delivers raw frames only; PID/process name must be resolved out-of-band (e.g. `GetExtendedTcpTable` + port matching, or ETW `TcpIp` provider). Race windows exist between capture and lookup, especially for short-lived connections.
- **No TCP reassembly.** Npcap hands off individual IP/TCP segments. The application must track sequence numbers, handle out-of-order delivery, retransmits, and fragmentation itself. HTTP request/response bodies split across segments will be invisible without reassembly.
- **No TLS visibility.** Capture occurs at the link/IP layer, after TLS encryption by the OS TLS stack (SChannel) or application. Passive capture sees ciphertext only; decryption requires keys (SSLKEYLOG) or active interception.

## 3. Architectural Requirement for "Proxy-less HTTPS Decryption"

Passive capture sees ciphertext only, so "proxy-less HTTPS decryption" cannot be achieved by observation alone. Two viable options:

- **(a) SSLKEYLOG-style key extraction (passive decrypt).** Extract session keys from the TLS implementation (browser `SSLKEYLOGFILE`, or hooking SChannel). Keeps traffic passive but is fragile, browser-specific, and often unavailable for non-browser apps. No system proxy changes required.
- **(b) WFP REDIRECT to local MITM proxy (active) — RECOMMENDED.** Use `FWPM_LAYER_ALE_CONNECT_REDIRECT_V4/V6` with `FwpsRedirectHandleCreate` to transparently redirect remote port-443 connections to a local rustls MITM listener. The original destination is preserved via FWPS connection redirect records, so the MITM can re-establish the upstream TLS session. No system proxy settings are touched, honoring the "no system proxy" promise; redirection happens in-kernel and is invisible to applications.
- **(c) Full kernel MITM (rejected).** Terminating TLS inside the kernel driver is far too complex and risky (BSOD surface, cert management in kernel, crypto in kernel-mode). Rejected.
## 4. Recommended WFP Redirect Architecture

- **Callout at ALE_CONNECT_REDIRECT layer** (`FWPM_LAYER_ALE_CONNECT_REDIRECT_V4` / `_V6`): redirect remote `:443` to `127.0.0.1:LISTEN_PORT` while preserving the original destination via FWPS connection redirect records (`FwpsRedirectHandleCreate` + redirect context).
- **Local rustls MITM server** terminates TLS with a dynamically generated leaf certificate (signed by a local CA installed in the user trust store), then re-encrypts upstream. See `tls_decryption_design.md`.
- **Process attribution** at classify time via `FwpsOpenToken` / process handle from the classify context, with `GetExtendedTcpTable` as a user-mode fallback.
- **Ring buffer:** 64 MB shared memory between driver and user-mode, seq-locked packet headers, overflow policy = drop oldest with a dropped-counter.
- **IOCTL contract:** `GET_PACKETS`, `GET_STATS`, `SET_FILTER` (plus `SET_REDIRECT_PORT`, `GET_DROPPED_COUNT`).
- **Signing:** EV code-signing certificate (~$400/yr) for production; Microsoft Hardware Dev Center **attestation signing** is the lower-cost alternative for test/insider builds.

## 5. TCP Reassembly Design

- **Per-flow state:** connection tuple (src/dst IP+port, protocol) → seq base, expected next seq, out-of-order segment heap (min-heap by seq), retransmit dedup (seq+length signature), flow eviction via LRU + idle timeout (e.g. 120 s).
- **Segment processing:** strip Ethernet + IPv4/IPv6 + TCP headers; handle VLAN (802.1Q) tags by skipping the 4-byte tag; reassemble in-order bytes into a per-flow byte stream; deliver completed HTTP messages to the parser.
- **Edge cases:** overlapping segments, zero-window probes, RST/FIN handling, out-of-order with holes, retransmit storms, connection reuse (HTTP keep-alive).

## 6. Phased Recommendation

- **Phase A (now):** Npcap + TCP reassembly + `GetExtendedTcpTable` attribution, all user-mode, no driver. Unblocks HTTP (non-TLS) capture and process attribution quickly.
- **Phase B:** WFP redirect callout + local rustls MITM. Meets the "replace HTTP Debugger Pro" goal (transparent HTTPS decryption without system proxy).
- **Phase C:** Full streaming callout / kernel-side filtering if performance or stealth requirements demand it.

## 7. Effort & Risk Table

| Phase | Effort | BSOD Risk | Signing Risk | Complexity |
|-------|--------|-----------|--------------|------------|
| A — Npcap + reassembly + attribution | 2–3 weeks | None (user-mode) | None | Medium |
| B — WFP redirect + MITM | 4–6 weeks | Medium (callout correctness) | High (EV/attestation) | High |
| C — Full streaming callout | 3–5 weeks | High | High | Very High |

**Key risks:** Phase B requires a correctly signed driver or attestation-signed test build; redirect-layer bugs can black-hole traffic (mitigate with bypass flags + safe-mode disable). Phase A's attribution has race windows for short-lived conns; use ETW `TcpIp` provider to close them later.

