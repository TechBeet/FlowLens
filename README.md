# FlowLens

[![Build & Test](https://github.com/your-org/FlowLens/actions/workflows/ci.yml/badge.svg)](https://github.com/your-org/FlowLens/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/flowlens.svg)](https://crates.io/crates/flowlens)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20|%20macOS%20|%20Linux-blue)](https://github.com/your-org/FlowLens)

A native Rust HTTP/HTTPS packet capture and debugging tool with process attribution, TLS decryption, and AI-powered MCP integration.

简体中文：一个原生的 Rust 无代理 HTTP/HTTPS 抓包与调试工具，通过 WFP、ETW 和本地根证书在系统协议栈中就地观察、解密并归属流量。

## Features

- **Proxy-less**: Captures traffic directly via WFP/Npcap, no system proxy needed
- **TLS Decryption**: Supports TLS 1.2/1.3 MITM decryption with local CA
- **Process Attribution**: Accurate PID, process name, EXE path attribution
- **Multi-Protocol**: HTTP/1.1, HTTP/2, WebSocket, SSE, gRPC
- **Portable**: Self-contained, no installation required
- **MCP Server**: AI agents can query captured traffic

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

# 2. Run FlowLens
flowlens.exe --capture          # Start GUI capture
flowlens.exe --tui              # Start terminal UI
flowlens.exe --status           # Quick diagnostic check

# 3. MCP Server (for AI integration)
flowlens.exe --mcp --port 9229
```

Claude Code configuration (.mcp.json):
```json
{
  "servers": [{
    "name": "FlowLens",
    "url": "http://localhost:9229/mcp"
  }]
}
```

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

