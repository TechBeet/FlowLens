# FlowLens GUI Conversion Plan

> **Status:** Planning document only — no implementation code. All code blocks are illustrative pseudo-code and are not compilable.
> **Project:** FlowLens (Windows native proxyless HTTP/HTTPS packet capture / debug tool)
> **Scope:** Migrate the existing ratatui TUI (`crates/flowlens-ui`) to a native window GUI (`crates/flowlens-gui`, `eframe`/`egui`).
> **Language:** Chinese primary, with English technical terms retained (bilingual headings where helpful).

---

## 1. Executive Summary（执行摘要）

This document outlines the migration plan from the current ratatui text-user-interface (TUI) to a native window graphical user interface (GUI) for FlowLens. The conversion introduces a new `flowlens-gui` crate built on `eframe`/`egui` (immediate-mode pure-Rust GUI) that implements the existing `UiFrontend` trait alongside the current TUI backend.

**核心结论 / Key decisions:**

| 项目 | 结论 |
|---|---|
| 推荐框架 | `eframe` / `egui` — 纯 Rust、单一原生窗口、无需 Node.js / WebView2 |
| 新增 crate | `crates/flowlens-gui`（与现有 `crates/flowlens-ui` 并列） |
| 抽象层 | 复用现有 `UiFrontend` trait（`run(bus)` / `stop()`），TUI 与 GUI 双前端共存 |
| 线程模型 | GUI 独立事件循环（`eframe`/`winit`）；抓包与解析管线沿用现有 `tokio` + `spawn_blocking`；`egui::Context` 仅在 UI 线程访问，后台线程通过 `ctx.request_repaint()` 触发重绘 |
| 离线构建 | 当前本地 registry **不含** 任何 GUI 框架 crate，必须先执行一次 `cargo fetch`（见 Phase 0） |
| TUI 去向 | 保留为回退模式（`flowlens start`），不删除 |

**目标 / Goal:** 以原生窗口 GUI 取代 TUI 作为主交互界面，同时保持与当前 TUI 的功能对等（事件表格、过滤、捕获控制、TLS 明文流量展示）。

---

## 2. Current State Analysis（现状分析）

### 2.1 Terminal TUI — `crates/flowlens-ui`

| 维度 | 现状 |
|---|---|
| 框架 | `ratatui` 0.26 + `crossterm` 0.27 |
| 渲染方式 | Alternate screen + raw mode，`Table` widget 渲染流量事件 |
| 键盘处理 | 内联在渲染循环中（`q` / `j` / `k` / `f` / `d` / 上下键） |
| 事件来源 | `flowlens_core::event_bus::BroadcastEventBus`，后台线程 `blocking_recv()` |
| 状态 | `AppState { events: Arc<Mutex<Vec<TrafficRow>>>, running: AtomicBool, table_state: TableState, debug_mode, status_msg }` |
| 容量控制 | `MAX_EVENTS = 1000`，超过 `AUTO_PRUNE_THRESHOLD = 500` 时 `drain` 头部 |
| 已知不足 | 过滤仅支持清空；详情视图弱；无导出/统计/设备选择 |

**Evidence:** `crates/flowlens-ui/src/tui.rs`（`subscribe` 见 L79；容量裁剪见 L86-89；按键处理见 L217-251）。

### 2.2 Wiring in `src/main.rs`

- 根二进制 crate 通过 `clap` 解析 `flowlens_cli::Cli`。
- `Start` 子命令携带 `--gui` / `-g` 标志（当前标记为 deprecated）。
- 启动流程：创建 `Arc<BroadcastEventBus>` → `flowlens_capture::Engine::auto()`（Npcap 真实捕获，失败或 `--mock` 时回退到 `spawn_mock_producer`）→ engine pump 把包推入事件总线。
- 前端选择分支（约 L160-171）：`if gui { ... } else { ... }` —— **两个分支当前都构造 `flowlens_ui::TuiFrontend::new()`**；`--gui` 路径仅打印 “egui frontend was removed” 弃用警告后启动 TUI。
- 结论：GUI 接入点已经预留，只需替换 `--gui` 分支的实现。

### 2.3 Tauri / Svelte shell — `ui/`

| 项目 | 现状 |
|---|---|
| 技术栈 | Tauri v2 + Svelte 5 + Vite，`ui/src-tauri/` 与 `ui/src/` 分离 |
| 已注册命令 | `start_capture`、`stop_capture`、`get_stats`、`list_devices`、`export_har`、`install_ca` / `uninstall_ca` / `check_ca_status` / `get_ca_pem`、`pause_session` / `release_session` / `modify_session` / `list_paused_sessions`、`add_rewrite_rule` / `remove_rewrite_rule` / `toggle_rewrite_rule` / `list_rewrite_rules`、`replay_request`、`compare_sessions` |
| 缺陷 | `get_stats` 前端未轮询（统计恒为 0）；`list_devices` 未在 UI 暴露；`Sidebar` 搜索框无 `bind:value`；`RightPanel` 各标签内容未分化 |
| 离线可用性 | ❌ 不可用：`ui/src-tauri/target/release/deps` 有历史编译产物，但 `tauri` / `wry` / `webview2-com` **不在** `.cargo-home/registry/src` 中，离线 `cargo build` 会失败 |
| 定位 | **设计参考**（布局、命令集、事件模型），不是本次迁移的目标实现 |

### 2.4 Frontend abstraction — `crates/flowlens-ui/src/lib.rs`

```rust
// 现状摘录（非新增实现）
pub trait UiFrontend: Send + Sync {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()>;
    fn stop(&self) -> anyhow::Result<()>;
}
pub use tui::TuiFrontend;
```

该 trait 是 TUI 与未来 GUI 的共同契约，是本次迁移可复用的关键资产。

### 2.5 Core API surface consumed by the GUI

| 模块 | 可用能力 |
|---|---|
| `flowlens-core::event_bus` | `EventBus` / `BroadcastEventBus`，`subscribe(TrafficFilter)`、`blocking_recv()` |
| `flowlens-core::models` | `TrafficEvent`、`TrafficRow`（含 `TrafficRow::from_event()`）、`TrafficFilter` |
| `flowlens-core::intercept` | `InterceptEngine`（pause / release / modify / list_rules）—— 库级已实现，UI 只需接线 |
| `flowlens-core::rewrite` | `RewriteEngine`（增删开关规则、应用规则）—— 库级已实现 |
| `flowlens-replay` | `ReplayClient`、`diff_sessions`、`fuzz_param` —— 库级已实现 |
| `flowlens-export` | HAR 导出 / SAZ 导入 |
| `flowlens-tls` | `CaManager`、CA 安装/卸载/状态查询 |
| `flowlens-capture` | `Engine::auto()`、`spawn_mock_producer()`、`list_devices` |

---

### 2.6 Offline Crate Availability（离线 Crate 可用性）—— 来自 `gui-architect` 的发现

> **关键发现**：本地 Cargo registry 缓存（`d:\github\FlowLens\.cargo-home\registry\src`） **不包含** 任何候选原生 GUI 框架的 crate。
>
> 仅有 `ratatui` (0.26.3) + `crossterm` (0.27) 以及少量工具 crate（`chrono`、`serde`/`serde_json`、`tokio`、`tracing` 等）可离线构建。

| 框架 | 本地 registry 状态 |
|---|---|
| `egui` / `eframe` | ❌ **不在缓存** — 需要联网 `cargo fetch` |
| `iced` | ❌ 不在缓存 |
| `slint` / `slint-build` | ❌ 不在缓存 |
| `tauri` / `tauri-build` | ❌ 不在缓存（且依赖 Node.js / Vite） |
| `druid` | ❌ 不在缓存 |
| `winit` / `wgpu` / `glow` / `tiny-skia` | ❌ 不在缓存（`egui` 的传递依赖） |
| `wry` / `webview` | ❌ 不在缓存 |

> **结论**：推荐 `egui`/`eframe` 路径 **必须** 在离线构建前执行一次 `cargo fetch`（见 Phase 0），否则 `cargo check --no-fetch` 会因网络不可达而失败。
> 在 `egui`/`eframe` 及其传递依赖进入本地缓存之前，**唯一可离线构建的 UI 仍是现有 ratatui TUI**。

---

## 3. Goals and Non-Goals（目标与非目标）

| 目标 (Goals) | 非目标 (Non-Goals) |
|---|---|
| **目标 1**: 新增 `crates/flowlens-gui`，基于 `eframe`/`egui` 实现 `UiFrontend` trait。 | 非目标 1: 保留或替换现有 Tauri/Svelte shell（`ui/` 目录）—— 该 shell 为独立 web 前端，本次迁移仅负责 Rust 原生 GUI。 |
| **目标 2**: 单一原生窗口（无需打开终端/Alternate screen），`--gui` 启动即窗口。 | 非目标 2: 完整 Windows 原生主题（egui 自绘），不追求系统主题/高对比度/系统级 DPI 适配等完美效果。 |
| **目标 3**: 与当前 TUI 功能对等：事件表格、键盘导航、`f` 清空、`d` 调试模式、过滤、捕获状态栏。 | 非目标 3: 移动端调试工作流、QR 码生成、证书信任向导（属 `docs/mobile_ca_trust.md` 范畴）。 |
| **目标 4**: 支持 TLS 解密后的流量展示（SNI、HTTP method、URL、状态码、请求/响应头体）。 | 非目标 4: 重新实现库级已存在的功能（breakpoints、rewrite rules、replay、compare、fuzzer）—— 仅负责 UI 层展示与交互接线。 |
| **目标 5**: 一次联网取包后，可重复 `--no-fetch` 离线编译。 | 非目标 5: Web 热重载 / Vite HMR / 前端构建链（egui 采用“改代码即重编重启”的工作流）。 |
| **目标 6**: 保留 TUI 作为回退模式：`flowlens start`（无 `--gui`）仍启动 `TuiFrontend`。 | 非目标 6: 拆分独立 GUI 可执行文件 —— GUI 与 CLI 仍为同一个二进制，由 `--gui` 选择。 |

---
## 5. Architecture Design（架构设计）

### 5.1 Crate 结构

```
crates/flowlens-gui/            # NEW — eframe/egui GUI 前端
├── Cargo.toml                  # 依赖 flowlens-core、egui、eframe、winit、wgpu
├── src/
│   ├── lib.rs                  # 声明 `GuiFrontend` 实现 `UiFrontend`
│   ├── main.rs                 # `eframe::run()` 入口点
│   ├── ui.rs                   # egui 框架构建逻辑（表格、面板、输入框）
│   ├── state.rs                # 与 `AppState` 互通的状态结构（egui 友好）
│   └── threading.rs            # 生产者 → 事件总线 → GUI 状态桥（后台 receiver thread）
```

**已有 crate 维持不变**：
- `crates/flowlens-ui` — 保留 TUI 实现 + `UiFrontend` trait
- `crates/flowlens-core` — 事件总线、模型、管线（不依赖 GUI）
- `src/main.rs` — CLI 解析 `--gui` 标志，选择前端实现

### 5.2 API 消费模型

`flowlens-gui` 将**不**直接链接 `flowlens-capture`/`flowlens-tls` 等内部 crate，而通过 **事件总线 + 已有 trait** 消费能力：

1. **导入 `UiFrontend` trait** 从 `crates/flowlens-ui`（或本地重新导出）。
2. **`GuiFrontend` 结构体持有**：
   - `Arc<dyn EventBus>` — 共享事件总线
   - `Arc<Mutex<Vec<TrafficRow>>>` — 事件缓冲区（镜像 TUI `AppState.events`，已证明线程安全）
   - `Arc<AtomicBool>` — 运行标志（同 TUI 同步）
3. **`run()` 方法**：
   - 在 UI 线程启动 `eframe::run()`。
   - 同时启动**后台 receiver 线程**（与 TUI 完全一致的模式）：`bus.subscribe()` → `blocking_recv()` → `lock_mutate()` 事件缓冲区 → `ctx.request_repaint()`。
4. **`stop()` 方法**：
   - 设置 `running = false`；
   - `eframe` 窗口关闭；
   - 后台 receiver 线程 join。

### 5.3 线程模型细节（Threading model）

| 线程 | 责任 | 同步机制 |
|---|---|---|
| **UI 线程** | `eframe` 事件循环 + `egui::Context`。读取事件缓冲区（`Mutex<Vec<TrafficRow>>`）并在 `egui::Window` / `egui::CentralPanel::frame` 构建 UI。 | 仅在 `update` / `ui` 中**短暂持锁**读取 `events`；`request_repaint()` 由后台线程触发。 |
| **后台 receiver 线程** | `bus.subscribe()` → `blocking_recv()` → `lock_mutate()` 事件缓冲区 → `ctx.request_repaint()`。 | `events.lock()` → `push(row)` → `request_repaint()`，**与 TUI `tui.rs` 第 79-96 行模式完全一致**。 |
| **捕获/解析管线** | `tokio` + `spawn_blocking` → storage → `EventBus` → `blocking_recv()`。 | 与 GUI **无直接耦合**，通过 `EventBus` 与 `Mutex<Vec<TrafficRow>>` 解耦。 |

### 5.4 API 示意（示例，非可编译实现）

```rust
// crates/flowlens-gui/src/lib.rs
use std::sync::Arc;
use flowlens_core::event_bus::EventBus;
use flowlens_ui::UiFrontend;

pub struct GuiFrontend {
    bus: Arc<dyn EventBus>,
    events: Arc<std::sync::Mutex<Vec<TrafficRow>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    // egui 状态（可选：过滤文本、debug 模式、表格选中等）见 state.rs
}

impl GuiFrontend {
    pub fn new(bus: Arc<dyn EventBus>) -> Self {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        // **后台 receiver 线程在此启动**（与 TUI 完全一致模式）
        Self { bus, events, running }
    }
}

impl UiFrontend for GuiFrontend {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()> {
        // eframe::run(NativeOptions::default(), Box::new(|cc| Box::new(GuiApp::new(self))))
        Ok(())
    }
    fn stop(&self) -> anyhow::Result<()> {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}
```

---
---


## 4. Framework Evaluation & Rationale（框架评估与选择依据）—— 基于 `gui-architecture.md`

| 评估维度 | `egui` / `eframe` | `iced` | `slint` | Tauri (Svelte) | Raw Win32 API |
|---|---|---|---|---|---|
| 语言 | 纯 Rust | 纯 Rust | Rust + .slint | Rust + TS/Svelte | Rust + raw Win32 API |
| 运行时 / 窗口 | `winit` + `wgpu`/`glow` | `winit` + `wgpu` | 自有平台抽象 | WebView2 + Node | Raw Win32 API |
| 离线构建（当前状态） | ❌ **不在缓存** | ❌ 不在缓存 | ❌ 不在缓存 | ❌ 不在缓存（需 Node） | ✅ 仅需系统 DLL |
| 单一二进制 无 Node | ✅ 是 | ✅ 是 | ✅ 是 | ❌ 需 Node/Vite | ✅ 是 |
| IMMEDIATE vs RETAINED | **Immediate**（egui） | Retained-ish | Declarative retained | Retained (web) | Imperative Win32 |
| 表格/列表视图 | 中等（`egui::Table`/`ScrollArea`） | 良好 | 良好 | 卓越（web） | 困难（原生 Win32） |
| 热迭代 / 开发体验 | 良好 | 中等 | 中等 | 良好 (Vite HMR) | 差 |
| Windows 原生外观 | 自绘（非系统主题） | 自绘 | 接近原生 | Web 样式 | 真原生 |
| 与 `UiFrontend` trait 的集成 | **自然**（新 impl） | 自然 | 自然 | **需独立二进制** | 自然 |

### 4.1 选择 `eframe`/`egui` 的理由

| 理由 | 说明 |
|---|---|
| 纯 Rust + 单一二进制 | 与 FlowLens 定位“Windows 原生无代理”一致，无需 Node.js、WebView2、Vite 等前端工具链 |
| Immediate-mode 映射事件流 | 每帧从 `Mutex<Vec<TrafficRow>>` 重新绘制表格，与 TUI `terminal.draw()` 循环精神一致 |
| API 表面极简 | 仅需实现 `run(bus)` / `stop()`，包装 `eframe::run()` 即可 |
| 离线友好度最高 | 仅需一次 `cargo fetch` 将 `egui`/`eframe`/`winit`/`wgpu`/`glow`/`pistachio`/`epaint` 等写入本地 registry，后续 `--no-fetch` 可编译（无 C 编译器依赖除 MSVC / Rust 已有的 toolchain） |

### 4.2 反对 Tauri 的理由

| 理由 | 说明 |
|---|---|
| **重复 Tauri 工作** | `ui/` 已存在 Tauri shell，本次迁移若转向 Tauri 将在同一代码库维护两套 web 前端，沟通成本与复杂度倍增 |
| **离线不可用** | Tauri 与 `wry`/`webview2-com` 未在本地 registry，离线构建会失败 |
| **职责边界不清** | Tauri 需要 Node/Vite 编译 Svelte；`eframe` 完全由 Rust 编译；两者在项目结构上属于不同维度的投入 |

> **推荐方案**：**`eframe` / `egui` 作为新 crate `crates/flowlens-gui`**，与 TUI 并列于 workspace。

---
---


## 6. Migration Phases（迁移阶段）

### Phase 0: Scaffold & Offline Dependency Setup（脚手架 & 离线依赖准备）

| 任务 | 完成标准 |
|---|---|
| [ ] 将 `flowlens-gui` 加入 workspace `Cargo.toml` `members`。 | `cargo check -p flowlens-gui` 编译通过 |
| [ ] 向 `crates/flowlens-gui/Cargo.toml` 添加依赖：`egui`, `eframe`, `winit`, `wgpu`, `glow`, `pistachio`, `epaint`（均使用 `download = false` 以尝试离线；后续 `cargo fetch` 填充）。 | 依赖声明正确 |
| [ ] **一次性 `cargo fetch`** 下载 `egui` 及其传递依赖进入本地 registry（`.cargo-home/registry/src`）。 | 缓存中出现 `egui-0.29.*`、 `eframe-0.29.*`、`wgpu-0.22.*` 等 |
| [ ] 验证 `cargo build --no-fetch -p flowlens-gui` 能成功（无网络）。 | 离线构建通过 |
| [ ] 编写 `crates/flowlens-gui/src/lib.rs`：`GuiFrontend` 骨架实现 `UiFrontend`（`run` / `stop` 空实现）。 | `GuiFrontend` 结构体与 trait 实现编译通过 |
| [ ] 编写 `crates/flowlens-gui/src/main.rs`：最小 `eframe::run()` 脚手架（创建窗口、标题）。 | `cargo run -p flowlens-gui` 弹出空窗口 |

---

### Phase 1: Event Bridge & Basic Window（事件桥接 & 基础窗口）

| 任务 | 完成标准 |
|---|---|
| [ ] 实现 `GuiFrontend::run()`：`eframe::run(NativeOptions::default(), Box::new(|cc| Box::new(GuiApp::new(self))))`。 | `eframe` 窗口启动 |
| [ ] 设计 `GuiApp`（`eframe::App` 实现）：`update` / `view` / `title`。 | egui 框架编译 |
| [ ] 启动 **后台 receiver 线程**（与 TUI 完全一致）：`bus.subscribe(TrafficFilter::default())` → `blocking_recv()` → `events.lock()?.push(row)` → `ctx.request_repaint()`。 | 事件流进入 GUI |
| [ ] 在 `GuiApp::view` 中渲染 `egui::CentralPanel` → `egui::Table`（见下）构建表格，列头：`Time`、`Source`、`Destination`、`Protocol`、`Length`（镜像 TUI）。 | 表格出现 |
| [ ] 处理 `q` / `Esc` 退出（`ctx.input(|i| i.key_pressed(egui::Key::Escape))`）。 | 窗口正常关闭 |
| [ ] **双向验证**：`flowlens start --gui` 打开窗口并实时显示流量；`flowlens start`（无 `--gui`）仍启动 TUI。 | 前端切换无缝 |

---

### Phase 2: Feature Parity with TUI（与 TUI 功能对等）

| 任务 | 完成标准 |
|---|---|
| [ ] **过滤框**：`egui::TextEdit`（单行），实时过滤表格行（TUI `f` 清空映射为 “清空过滤框”）。 | 输入框实时过滤，`f` 键/按钮清空过滤 |
| [ ] **调试模式开关**：`D` 键或 UI 按钮切换 `debug_mode`，表格列动态显示/隐藏原始列（如 `TrafficEvent` 原始 JSON）。 | `D` 键切换，表格列增/减 |
| [ ] **捕获状态栏**：顶部 `egui::TopBottomPanel` 显示 “运行中 / 暂停 / 已停止”、已接收事件数、`bytes_sent` / `bytes_received`（如 TUI 状态栏）。 | 状态栏实时更新 |
| [ ] **键盘导航**：`j`/`k` / 上下键 选中行；`Enter`/`Space` 打开行详情（Phase 3）；`F` 清空所有事件；`D` 调试模式。 | 键盘行为 100% 与 TUI 一致 |
| [ ] **窗口尺寸 / DPI / 缩放**：`eframe::NativeOptions` 设置初始尺寸、最小尺寸、可拉伸；DPI 适配正常（`egui` 自带）。 | 窗口可拖放、可最大化、DPI 缩放不失真 |

---

### Phase 3: TLS-Decrypted Traffic Display（TLS 解密流量展示）

| 任务 | 完成标准 |
|---|---|
| [ ] **表格列扩展**：SNI、HTTP Method、URL、Status Code（当 `TrafficEvent` 中 TLS 已解密时可见）。 | 解密流量行显式列出 SNI、Method、URL、Status |
| [ ] **右侧详情面板**：`egui::SidePanel::right()`（宽度 300-400px），点击行后显示该行的 **请求头 / 响应头 / 请求体 / 响应体**（4 标签页，与 Tauri `RightPanel` 设计对齐）。 | 单击行 → 详情面板切换标签，内容与请求/响应对应 |
| [ ] **未解密回退**：若行无 TLS 解密，面板显示 “TLS 解密不可用”或原始 `TrafficEvent` JSON（调试模式）。 | 与 TUI 调试模式行为一致 |
| [ ] **增量更新**：后台 receiver 线程接收 `TrafficEvent::HttpRequest` / `HttpResponse` 时，缓存到 `TrafficRow`，`view` 直接读取；无需额外 `blocking_recv()`。 | 实时显示不丢帧 |

---

### Phase 4: Polish & Release（润色 & 发布）

| 任务 | 完成标准 |
|---|---|
| [ ] **窗口图标 & 标题**：`NativeOptions::icon`（`FlowLens.ico`/`png`），`title: "FlowLens"`。 | 窗口图标与标题一致 |
| [ ] **`eframe::NativeOptions` 完整配置**：`initial_size`, `min_size`, `resizable`, `always_on_top`, `transparent`, `decorations`。 | 窗口行为符合期望 |
| [ ] **单元测试**：`crates/flowlens-gui/tests/gui_integration.rs`：伪造 `EventBus`、`TrafficEvent`，验证 `GuiFrontend` 状态同步与 `request_repaint()` 触发。 | `cargo test -p flowlens-gui` 通过 |
| [ ] **`src/main.rs` 更新**：`--gui` 分支改为 `flowlens_gui::GuiFrontend::new(bus)`（首选），回退至 `TuiFrontend`（兼容旧版）。 | `--gui` 启动 GUI 且回退机制可用 |
| [ ] **文档**：`docs/gui-implementation-checklist.md`、贡献指南更新离线构建步骤 (`cargo fetch`)、`CONTRIBUTING.md`。 | 文档完整 |
| [ ] **TUI 回退验证**：`flowlens start`（无 `--gui`）仍启动 `TuiFrontend`，`q`/`j`/`k`/`f`/`d` 按键行为未变。 | TUI 完全可用 |

---

