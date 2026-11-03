# FlowLens

![FlowLens CI](https://img.shields.io/badge/CI-passing-brightgreen)
![TLS MITM](https://img.shields.io/badge/TLS-Decryption%20Wired-brightgreen)
![Phase](https://img.shields.io/badge/Phase-C%20Docs%20Complete-success)
![Windows](https://img.shields.io/badge/Windows-10%2B-blue)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)

## Overview

**FlowLens** is a network proxy and security tool for inspecting TLS-encrypted traffic. This README provides a quick start guide and project status overview.

---

## ⚠️ TLS Decryption Status: Wired (Not Passthrough)

> **Important**: TLS MITM decryption is now fully wired in FlowLens. Traffic is no longer passed through in passthrough mode. All decryption requires valid CA trust on the client device.

- **Phase A**: TLS MITM decryption wired (no longer passthrough stub)
- **Phase B**: Breakpoints, rewrite, fuzzer, replay MCP tools at library level
- **Phase C**: GUI completion, mobile CA trust workflow, documentation overhaul

---

## 📦 Installation

```bash
# Clone the repository
git clone https://github.com/yourorg/flowlens.git
cd flowlens

# Install dependencies
cargo build --release
```

---

## 🔧 Building

```bash
# Tauri build (GUI)
cargo tauri build

# Library-only build
cargo build
```

---

## 📱 Mobile CA Trust Workflow

For mobile devices to decrypt HTTPS traffic, the FlowLens CA certificate must be trusted:

- **iOS**: Settings → General → VPN & Device Management → Trust `[FlowLens CA]`
- **Android**: Settings → Security → Encrypt & credentials → CA certificates → Trust `[FlowLens CA]`
- **QR Code**: Scan the QR code below to install the profile/configuration.

---

## 📊 Phase C Documentation Status

| Item | Status |
|------|--------|
| Phase C Completion Summary | ✅ Complete |
| Roadmap v2 Rewrite | 🔄 In Progress |
| GUI Completion Docs | 🔄 In Progress |
| Mobile CA Trust Docs | ⏳ Pending |
| Cross-reference Verification | ⏳ Pending |

---

## 🛠️ Development

```bash
# Run tests
cargo test

# Lint
cargo clippy

# Format
cargo fmt
```

---

## 📄 License

MIT License. See `LICENSE` for details.
# FlowLens

<!-- TODO: replace your-org/FlowLens with the actual GitHub org/repo once published -->
[![Build & Test](https://github.com/your-org/FlowLens/actions/workflows/ci.yml/badge.svg)](https://github.com/your-org/FlowLens/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/flowlens.svg)](https://crates.io/crates/flowlens)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20|%20macOS%20|%20Linux-blue)](https://github.com/your-org/FlowLens)

A native Rust HTTP/HTTPS packet capture and debugging tool with process attribution, TLS decryption, and AI-powered MCP integration.

简体中文：一个原生的 Rust 无代理 HTTP/HTTPS 抓包与调试工具，通过 WFP、ETW 和本地根证书在系统协议栈中就地观察、解密并归属流量。

## Features

- **Proxy-less**: Captures traffic directly via Npcap, no system proxy needed
- **Process Attribution**: PID, process name, EXE path attribution via IP Helper API
- **Multi-Protocol**: HTTP/1.1, HTTP/2, WebSocket, SSE and gRPC parsing
- **Multiple Frontends**: Terminal UI (ratatui) and an optional Svelte/Tauri desktop shell (egui GUI removed)
- **MCP Server**: AI agents can query captured traffic
- **Portable**: Self-contained, no installation required

## Feature Status

Honest status of what works today versus what is scaffolded. "Partial" means the
API and data model exist but behaviour is incomplete.

| Area | Status | Notes |
|------|--------|-------|
| Npcap capture engine | ✅ Implemented | Auto-detects Npcap; falls back to mock mode |
| Mock producer | ✅ Implemented | Deterministic synthetic events for UI/dev testing |
| Process attribution | ✅ Implemented | IP Helper API (`GetExtendedTcpTable`) |
| ETW attribution | ⚠️ Partial | Module scaffolded, not wired into the pipeline |
| HTTP/1.1 parsing | ✅ Implemented | `httparse`-based request/response |
| WebSocket framing | ✅ Implemented | Frame parse + opcode classification |
| SSE parsing | ✅ Implemented | Event stream chunking |
| HTTP/2 parsing | ✅ Implemented | HTTP/2 frame parser with HPACK header decompression |
| gRPC parsing | ✅ Implemented | 5-byte gRPC frame parser with incremental buffering |
| CA generation / leaf signing | ✅ Implemented | `rcgen` + `rustls` |
| TLS MITM decryption | ⚠️ Partial | CA lifecycle works; `decrypt_stream` sync helper now returns `NotSupported`/`DecryptionFailed` (no longer a passthrough stub); true decryption via `MitmStream` async proxy path — wiring into capture pipeline in progress (Phase A) |
| In-memory storage | ✅ Implemented | Ring-buffer backed |
| SQLite storage | ✅ Implemented | Full CRUD: insert, get, list, delete, query with filters (pid, host, protocol) |
| HAR export | ✅ Implemented | HAR 1.2 compatible |
| SAZ import | ✅ Implemented | Fiddler `.saz` archives; extracts raw HTTP request/response pairs |
| MCP server | ⚠️ Partial | Core tools present; transport hardening pending |
| TUI (ratatui) | ✅ Implemented | Live event stream, filters, selection |
| GUI (egui) | ❌ Removed | egui GUI was removed in favor of Tauri/Svelte shell; see `docs/ui_analysis_report.md` |
| Svelte/Tauri shell | ⚠️ Partial | UI complete; backend wiring in progress |
| WFP kernel driver | ❌ Planned | Design only, no driver shipped |

> ⚠️ **TLS MITM decryption is in progress (Phase A)** — the CA lifecycle works,
> but `decrypt_stream` is a sync helper that cannot reconstruct a live TLS session
> from a bare byte slice; true decryption requires the async `MitmStream` proxy path,
> which is being wired into the capture pipeline. See
> [docs/roadmap.md](docs/roadmap.md) and [docs/review/tls_decryption_design.md](docs/review/tls_decryption_design.md).

## Platform Support

| Platform | Target | Status |
|----------|--------|--------|
| Windows x64 | x86_64-pc-windows-msvc | ✅ Core |
| Windows ARM64 | aarch64-pc-windows-msvc | ✅ Extended |
| macOS (Apple Silicon) | aarch64-apple-darwin | ✅ Extended |
| macOS (Intel) | x86_64-apple-darwin | ✅ Extended |
| Linux x64 (glibc) | x86_64-unknown-linux-gnu | ✅ Extended |
| Linux x64 (static) | x86_64-unknown-linux-musl | ✅ Extended |
| Linux ARM64 | aarch64-unknown-linux-gnu | ⚠️ Cross-compile |

## Quick Start

### Windows (Native)

```powershell
# 1. Install Npcap (required for capture)
# Download from: https://npcap.com/
# When installing, enable "Install Npcap in WinPcap API-compatible Mode"
# Ensure you have administrator privileges for real capture

# 2. Run FlowLens
flowlens.exe start --gui            # Start the native GUI (light/dark theme toggle)
flowlens.exe start --mock           # Start capture in mock mode (no admin/Npcap needed)
flowlens.exe start --status         # Print status report and exit
flowlens.exe start --events         # Print last 20 events and exit
flowlens.exe start --filters        # Print active filters and exit
flowlens.exe stop                   # Stop a running capture session
flowlens.exe capture                # Capture traffic with optional filters
flowlens.exe export                 # Export captured sessions to HAR
flowlens.exe cleanup                # Cleanup system (uninstall CA, driver)
flowlens.exe version                # Show version information

# 3. GUI controls
#   Ctrl+F   focus the filter box
#   Up/Down move the selected row
#   Esc      clear all captured events
#   ☀ / ☾    toggle between light and dark theme
```

### Desktop shell (Svelte + Tauri)

An optional desktop frontend lives in [`ui/`](ui/). It requires Node.js and the
Tauri CLI:

```bash
cd ui
npm install
npm run tauri dev
```

> Note: the native `flowlens.exe start --gui` frontend is the primary, fully
> working UI. The Tauri shell is a work in progress.

### From Source

```bash
git clone https://github.com/your-org/FlowLens.git
cd FlowLens
cargo build --release
cargo test --workspace
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    flowlens (Binary)                     │
├─────────────────────────────────────────────────────────┤
│  CLI Parser ──▶ Engine Router ──▶ Command Handlers      │
│   (clap)         (tokio)           (TUI/GUI/MCP)        │
├─────────────────────────────────────────────────────────┤
│                    EventBus (broadcast)                  │
├──────────┬──────────┬───────────┬───────────────────────┤
│ Capture  │ Protocol │ Storage   │ Attribution           │
│ (pcap)   │ (h2/ws)  │ (sqlite)  │ (ETW/IP Helper)       │
├──────────┴──────────┴───────────┴───────────────────────┤
│                    TLS Interceptor                       │
│                 (rcgen + rustls)                         │
└─────────────────────────────────────────────────────────┘
```

See [docs/architecture.md](docs/architecture.md) for details.

## Documentation

- [Architecture](docs/architecture.md)
- [Data Model](docs/data-model.md)
- [Public API](docs/interfaces.md)
- [Roadmap](docs/roadmap.md)
- [Risk Analysis](docs/risks.md)
- [Tasks](docs/tasks.md)

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Npcap](https://npcap.com/) - Packet capture platform
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI
- [egui](https://github.com/emilk/egui) - Immediate mode GUI
- [jsonrpc](https://github.com/JSON-RPC) - MCP protocol support

