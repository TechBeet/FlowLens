# FlowLens 开发指南

FlowLens 是一个 Tauri 2 + Svelte 5 + Rust 工作区，用于本机 HTTP/HTTPS 抓包与调试。

- 前端：`ui/`（Svelte 5 runes + Vite + TypeScript）
- Rust 工作区：仓库根 `Cargo.toml`，成员在 `crates/*` 与 `ui/src-tauri`
- 桌面壳：`ui/src-tauri`（Tauri 2）

---

## 1. 环境准备

### Rust

- 安装 Rust 稳定版（`rustup`，默认 stable，需包含 `rustfmt` 与 `clippy` 组件）。
- CI 的最低支持版本（MSRV）为 **Rust 1.74**（见 `.github/workflows/ci.yml` 的 `msrv` job）。

```powershell
rustup toolchain install stable
rustup component add rustfmt clippy
rustc --version
cargo --version
```

### Node.js

- 使用 Node.js 20（与 CI 中 `actions/setup-node` 的 `node-version: "20"` 一致）。
- 前端依赖仅在 `ui/` 目录下安装。

```powershell
cd ui
npm ci          # 按 package-lock.json 安装（推荐）
npm --version
node --version
```

### Npcap SDK（Windows 抓包）

捕获真实网卡流量依赖 **Npcap**（含 WinPcap 兼容模式）。

1. 从 https://npcap.com/ 下载并安装 Npcap，安装时勾选 **WinPcap API-compatible Mode**。
2. 如需编译链接，另装 **Npcap SDK**（解压后把 SDK 的 `Include`/`Lib` 路径加入环境变量或按 `flowlens-capture` 构建脚本要求配置）。
3. 安装/捕获某些网卡需要管理员权限，请以管理员身份运行 FlowLens。

> Linux 下 Tauri 构建还需要 WebKitGTK 等依赖，参考 `.github/workflows/phase1-ci.yml` 中 `rust-lint` job 的 apt 安装列表。

### Tauri CLI

Tauri CLI 已作为前端 devDependency 提供（`@tauri-apps/cli`），通过 `npm run tauri` 调用，无需全局安装。

```powershell
cd ui
npm run tauri -- --version
```

---

## 2. 构建 / 测试 / Lint 命令

所有命令均在**仓库根目录**执行，除特别注明外。

### Rust

```powershell
# 全工作区构建
cargo build --workspace

# 全工作区测试
cargo test --workspace --all-targets

# 全工作区格式检查（CI 使用）
cargo fmt --all --check

# 全工作区 clippy，警告即错误（CI 使用）
cargo clippy --workspace --all-targets -- -D warnings

# 生成文档（CI 使用，警告即错误）
$env:RUSTDOCFLAGS="-D warnings"; cargo doc --workspace --no-deps
```

### 前端（在 `ui/` 目录下）

```powershell
cd ui
npm run dev        # 仅 Vite 开发服务器（端口 3000）
npm run build      # Vite 生产构建，输出到 ui/dist
npm test           # Vitest 单元测试（vitest run）
npm run test:watch # Vitest 监听模式
npm run check      # TypeScript 类型检查（tsc --noEmit）
npm run tauri dev  # 启动 Tauri 开发模式（webview + Rust）
```

### 单独调试某个 crate

```powershell
cargo build -p flowlens-tls
cargo test  -p flowlens-replay
```

---

## 3. 项目结构

```
FlowLens/
├─ Cargo.toml                 # Rust 工作区（成员 crates/* 与 ui/src-tauri）
├─ crates/
│  ├─ flowlens-core/          # 核心模型（TrafficRow/TrafficSession/RewriteRule 等）
│  ├─ flowlens-capture/       # Npcap 抓包引擎（pcap.rs）
│  ├─ flowlens-protocol/      # 协议解析
│  ├─ flowlens-attribution/   # 进程归属
│  ├─ flowlens-tls/           # 根 CA / SNI / MITM（cert_store, mitm, policy）
│  ├─ flowlens-replay/        # 会话重放
│  ├─ flowlens-export/        # HAR 1.2 导出（har.rs）
│  ├─ flowlens-storage/       # 会话存储
│  ├─ flowlens-installer/     # 安装相关
│  ├─ flowlens-integration/   # 集成
│  ├─ flowlens-mcp/           # MCP 接入
│  ├─ flowlens-cli/           # 命令行入口
│  ├─ flowlens-ui/            # UI 相关 Rust
│  └─ flowlens-driver/        # 驱动
├─ ui/
│  ├─ package.json            # 前端脚本（dev/build/test/check/tauri）
│  ├─ vite.config.mjs         # Vite（端口 3000，预览端口 4173）
│  ├─ vitest.config.mts       # Vitest（注册 svelte 插件以编译 runes）
│  ├─ src/
│  │  ├─ App.svelte           # 顶层布局
│  │  ├─ components/          # Header/Toolbar/FilterBar/TrafficTable/DetailPanel/SettingsView/SniDomainSelector 等
│  │  └─ lib/
│  │     ├─ api.ts            # Tauri 命令封装
│  │     ├─ types.ts          # UiEvent/TrafficStats 等类型
│  │     ├─ shortcuts.ts      # 快捷键注册表
│  │     ├─ rewriteRules.ts   # 重写规则
│  │     ├─ stores/           # eventStore.svelte.ts（批处理/背压）
│  │     ├─ composables/      # eventSubscriber, useFilter, useStats
│  │     └─ generated/        # ts-rs 生成的类型
│  └─ src-tauri/              # Tauri 壳
│     ├─ tauri.conf.json      # 应用/窗口/CSP 配置
│     ├─ Cargo.toml           # 依赖 tauri, flowlens-*, reqwest 等
│     └─ src/                 # commands.rs（所有 Tauri 命令）, lib.rs, state.rs
├─ .vscode/                   # launch.json / tasks.json / extensions.json
├─ .github/workflows/         # CI（ci.yml, phase1-ci.yml, build.yml, release.yml）
└─ docs/                      # 本指南、用户手册、ADR
```

---

## 4. VS Code 调试

`.vscode/launch.json` 与 `.vscode/tasks.json` 提供以下入口：

| 名称 | 说明 |
|------|------|
| **Tauri: Dev (npm run tauri dev)** | 启动完整 Tauri 开发模式（webview + Rust） |
| **Tauri: Dev (webview, Chrome/Edge)** | 仅前端开发服务器 + 浏览器调试 |
| **Vite: Preview (dist, Vite only)** | 构建后在 4173 端口预览，不启动 Rust |
| **Tauri: Rust app (lldb launch)** | 调试已构建的 Rust 可执行文件 |
| **Rust: Attach to running FlowLens** | 附加到正在运行的 FlowLens 进程 |
| **CLI: flowlens-cli (lldb launch)** | 调试命令行入口 |

推荐扩展（`.vscode/extensions.json`）：`svelte.svelte-vscode`、`rust-lang.rust-analyzer`、`vadimcn.vscode-lldb`、`tauri-apps.tauri-vscode`、`ms-playwright.playwright`、`editorconfig.editorconfig`。

---

## 5. 提交前检查（pre-commit hook）

本仓库**不自带** husky / lint-staged，而是提供一个 PowerShell 脚本 `scripts/pre-commit.ps1`，依次运行：

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `npm run build`（在 `ui/` 下）
4. `npx vitest run`（在 `ui/` 下）

任一步失败即中止提交。手动运行：

```powershell
pwsh -File scripts/pre-commit.ps1
# 或
powershell -ExecutionPolicy Bypass -File scripts/pre-commit.ps1
```

### 安装为 git hook（手动，可选）

脚本**不会**自动修改 `.git/hooks`。如需启用，请自行执行：

```powershell
# 在仓库根目录
$hook = ".git/hooks/pre-commit"
@'
#!/bin/sh
powershell -ExecutionPolicy Bypass -File scripts/pre-commit.ps1
'@ | Set-Content -Path $hook -Encoding ASCII
```

> Windows 上建议用上面的 `sh` 包装脚本，因为 git 通过 `sh` 调用钩子。若使用 Git Bash，可再执行 `chmod +x .git/hooks/pre-commit`。
> 如需临时跳过：`git commit --no-verify`。

---

## 6. 贡献检查清单

提交 Pull Request 前请确认：

- [ ] `cargo fmt --all --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 无警告
- [ ] `cargo test --workspace --all-targets` 通过
- [ ] `cd ui && npm run build` 成功（文档变更不影响构建）
- [ ] `cd ui && npm test` 通过
- [ ] `cd ui && npm run check` 类型检查通过
- [ ] 新增/修改的 Tauri 命令已在 `ui/src-tauri/src/lib.rs` 的 `generate_handler!` 中注册
- [ ] 新增/修改的类型已同步 `ui/src/lib/types.ts` 与 `ui/src/lib/generated/`（如涉及 ts-rs）
- [ ] 新增用户可见功能已更新 `docs/USER_GUIDE.md`
- [ ] 涉及架构决策时新增 `docs/adr/` ADR
- [ ] 未提交调试日志、临时文件或本地配置
- [ ] 提交信息清晰描述变更内容与影响范围

---

## 7. 架构决策记录（ADR）

架构决策记录采用 MADR-lite 格式，位于 `docs/adr/`：

- [0001 — 使用 Tauri 而非 Electron](adr/0001-use-tauri-over-electron.md)
- [0002 — 使用 Svelte 5 Runes](adr/0002-use-svelte-5-runes.md)
- [0003 — 事件总线与批量 UI 更新](adr/0003-event-bus-and-batched-ui-updates.md)
- [0004 — CA 与 TLS 拦截设计](adr/0004-ca-and-tls-interception-design.md)
- [0005 — PathProvider 与设置持久化](adr/0005-pathprovider-settings-persistence.md)

相关链接：

- [用户手册](USER_GUIDE.md)
- [ADR 索引](adr/README.md)
