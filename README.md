# FlowLens

FlowLens 是一款开源、MIT 协议的 Windows 本机 HTTP/HTTPS 抓包与调试工具，旨在替代 HTTP Debugger Pro。

## 核心特性

- **无需代理的 HTTPS 解密**：基于 WinDivert 驱动层拦截，无需修改系统代理设置即可解密 HTTPS 流量
- **开源免费**：MIT 许可证，可自由使用、修改和分发
- **SNI 感知解密**：按域名灵活控制解密范围，支持通配符规则
- **本地 MITM 引擎**：动态签发域名证书，支持 TLS 1.2/1.3
- **流量分析**：实时捕获、过滤、查看请求/响应详情
- **会话重放与重写**：支持重放请求、自定义重写规则
- **HAR 导出**：导出标准 HAR 1.2 格式，兼容浏览器开发者工具

## 技术架构

- **前端**：Tauri 2 + Svelte 5 + TypeScript
- **后端**：Rust 工作区
- **抓包引擎**：WinDivert（网络层拦截）+ Npcap（可选）
- **TLS 解密**：rustls + rcgen 动态证书签发

## 环境要求

- Windows 10/11（64 位）
- Microsoft Edge WebView2 运行时
- 管理员权限（用于 WinDivert 驱动安装和证书管理）
- 可选：Npcap（用于传统抓包模式）

## 快速开始

1. 下载并安装 FlowLens
2. 以管理员身份运行
3. 在设置中生成并安装根 CA 证书
4. 选择要解密的域名，开始捕获

## 构建

```powershell
# 安装依赖
cd ui; npm ci

# 构建前端
npm run build

# 构建 Rust 工作区
cargo build --workspace

# 运行
cd ui; npm run tauri dev
```

## 测试

```powershell
cargo test --workspace --all-targets
cd ui; npm test
```

## 许可证

本项目采用 [MIT License](LICENSE) 开源。

## 免责声明

FlowLens 仅用于本机网络流量诊断与调试。使用本工具解密网络流量时，请遵守当地法律法规，并确保您有合法权限监控相关网络。安装根 CA 证书意味着您信任 FlowLens 为您签发任意域名的证书，请在不需要时及时卸载。

## 贡献

欢迎提交 Issue 和 Pull Request！

## 相关链接

- [用户手册](docs/USER_GUIDE.md)
- [开发指南](docs/DEVELOPMENT.md)
- [架构决策记录](docs/adr/)