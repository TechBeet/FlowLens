# FlowLens

[![Build Status](https://img.shields.io/github/actions?label=build&color=blue)](#)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![Windows](https://img.shields.io/badge/platform-Windows-blue)](https://microsoft.com)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

FlowLens 是一个用 Rust 构建的 Windows 原生无代理 HTTP/HTTPS 抓包与调试工具，通过 WFP、ETW 和本地根证书在系统协议栈中就地观察、解密并归属流量。

## 特性

- **无代理**: 无需配置系统代理、不监听端口
- **TLS 解密**: 支持 TLS 1.2/1.3 MITM 解密
- **进程归属**: 精确 PID、进程名、EXE 路径 attribution
- **多协议**: HTTP/1.1、HTTP/2、WebSocket、SSE、gRPC
- **免安装**: 便携式自释放、关闭清理可迁移
- **MCP 服务器**: AI 代理可查询流量

## 快速开始

### 安装 (Portable)
```powershell
.\FlowLens-Portable.zip
./flowlens.exe
```

### 驱动模式
```powershell
# 管理员权限
flowlens.exe --wfp-install
flowlens.exe --capture  # 启动 UI
flowlens.exe --wfp-uninstall  # 退出时自动清理
```

### MCP 服务器
```powershell
flowlens.exe --mcp --port 9229
```

Claude Code 配置 `.mcp.json`:
```json
{
  "servers": [
    {
      "name": "FlowLens",
      "url": "http://localhost:9229/mcp"
    }
  ]
}
```

## 文档
- [架构设计](docs/architecture.md)
- [路线图](docs/roadmap.md)
- [风险与回退](docs/risks.md)
- [数据模型](docs/data-model.md)
- [公共 API](docs/interfaces.md)
- [任务拆解](docs/tasks.md)

## 技术栈

| 组件 | 用途 |
|------|------|
| tokio | 异步运行时 |
| h2 | HTTP/2 解析 |
| tungstenite | WebSocket |
| rustls | TLS 1.3 |
| rcgen | CA 生成 |
| sqlx | SQLite 元数据存储 |
| ratatui | TUI |
| egui | GUI |
| windows-sys | ETW/IP Helper |
| pcap | Npcap 回退 |
| jsonrpc | MCP 协议 |

---

*Version: 0.1.0*
