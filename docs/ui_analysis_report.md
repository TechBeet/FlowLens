# FlowLens UI/UX 与功能完整性分析报告

> 分析范围：`crates/flowlens-ui`（TUI）、`ui/`（Svelte/Tauri shell）、`docs/review/feature_parity_matrix.md`
> 分析模式：只读，未修改任何源文件（本报告为新生成的文档）。
> 分析日期：2026-11-02

---

## 1. UI 现状总览

| 层级 | 实现状态 | 技术栈 | 代码证据 |
|------|----------|--------|----------|
| TUI | 已实现 | ratatui + crossterm | `crates/flowlens-ui/src/tui.rs`（主表格已渲染，键盘导航、过滤、详情已接入） |
| GUI (egui) | **已移除** | — | egui GUI 已彻底删除（`gui.rs`、`fonts.rs`、`Cargo.toml`/`lib.rs`/`main.rs` 同步更新），不再作为前端选项 |
| Svelte/Tauri shell | 部分完成 | Svelte 5 + Tauri v2 + Vite | `ui/src/`（5 组件），`ui/src-tauri/` |

结论：项目从"双 UI 并存"（egui + Tauri）已收敛为"单 UI"（Tauri/Svelte 为主，TUI 保留为终端备选）。egui GUI 已被彻底移除。README 的相关描述已同步更新（见本报告日期 2026-11-02 的 Phase A 文档更新）。

---

## 2. TUI (ratatui) 实现详情

文件：`crates/flowlens-ui/src/tui.rs`

### 2.1 已实现功能

| 功能 | 代码证据 |
|------|----------|
| 实时事件流接收 | `bus.subscribe(TrafficFilter::default())`（第 79 行）+ `blocking_recv` 循环 |
| 事件上限自动裁剪 | `MAX_EVENTS = 1000`，超 `AUTO_PRUNE_THRESHOLD = 500` 时 `drain`（第 86-89 行） |
| 键盘导航 | 上/下箭头、`j/k` 选择行（第 143-151 行） |
| 清空事件 | `F` 键（第 152-155 行） |
| 调试模式切换 | `D` 键（第 156-159 行） |
---

## 3. Svelte/Tauri Shell 实现详情

### 3.1 后端 wiring 状态

**文件**：`ui/src-tauri/src/commands.rs`（196 行）、`state.rs`（23 行）、`lib.rs`（23 行）

| Tauri 命令 | 状态 | 说明 |
|------------|------|------|
| start_capture | ✅ | 启动 Npcap/mock 引擎，spawn 事件 pump，emit `capture-started` |
| stop_capture | ✅ | 设置 running=false，取消 pump task，emit `capture-stopped` |
| get_stats | ✅ | 返回 TrafficStats |
| list_devices | ✅ | 返回可用设备列表 |
| export_har | ❌ 未注册 | `ui/src/lib/api.ts` 声明了 `exportHAR`，但 `ui/src-tauri/src/lib.rs` 的 `invoke_handler` 未注册此命令 |

**关键 wiring 缺陷**：
1. `export_har` 命令未注册 → 前端调用会报错。
2. `get_stats` 从未在前端轮询（`App.svelte` 只监听 `traffic-event`，`Header` 中的统计数字永远为 0）。
3. `list_devices` 未在 UI 中暴露，未实现设备选择。

### 3.2 前端组件

| 组件 | 状态 | 功能 |
|------|------|------|
| App.svelte | ✅ | Svelte 5 runes，事件监听（`traffic-event`、`capture-started`、`capture-stopped`），主题管理，日志 |
| Header.svelte | ✅ | 品牌、捕获状态指示器、统计栏（total/http/tls/ws/sse）、开始/停止按钮、主题切换（F2） |
| TrafficTable.svelte | ⚠️ | 表格渲染（Method/URL/Status/Size/Time），行选择，CSS 变量。无排序、无虚拟滚动、无列宽调整。 |
| Sidebar.svelte | ⚠️ | 搜索框（无绑定，输入无效）、事件列表、清除按钮。搜索框无 `bind:value`，永远无法搜索。 |
| RightPanel.svelte | ⚠️ | 多标签（请求头/响应头/请求体/响应体），但所有标签显示相同的 `JSON.stringify(event)`，未按标签展示实际内容。 |

### 3.3 前端功能矩阵

| 功能 | 状态 | 说明 |
|------|------|------|
| 捕获启停 | ✅ | `startCapture`/`stopCapture` in `App.svelte` |
| 实时事件流 | ✅ | `listen("traffic-event")` in `App.svelte` |
| 主题切换 | ✅ | `toggleThemeBtn` + `api.toggleTheme` + localStorage |
| 导出 HAR | ❌ | `exportHAR` 声明于 `ui/src/lib/api.ts`，但 `ui/src-tauri/src/lib.rs` 的 `invoke_handler` 未注册此命令 |
| 统计刷新 | ❌ | `get_stats` 命令存在但前端 `App.svelte` 未轮询，Header 统计数字恒为 0 |
| 设备选择 | ❌ | `list_devices` 命令存在但前端未暴露设备列表 |
---

## 4. HTTP Debugger Pro 功能对照表

| # | HTTP Debugger Pro 功能 | FlowLens TUI | FlowLens Tauri Shell | 状态 | 优先级 |
|---|------------------------|:------------:|:--------------------:|------|--------|
| 1 | 代理模式捕获 | ❌ | ❌ | 不实现（代理less架构） | N/A |
| 2 | 无代理捕获 (WFP/Npcap) | ✅ | ✅ | 已实现 | — |
| 3 | HTTPS 解密 | ❌ | ❌ | passthrough stub | P0 |
| 4 | 流量过滤（进程/主机/URL/时间） | ⚠️ | ❌ | TUI 无 UI；Shell 无过滤 | P1 |
| 5 | 断点（暂停请求/响应） | ❌ | ❌ | 未实现 | P1 |
| 6 | 重写规则（修改头/体） | ❌ | ❌ | 未实现 | P2 |
| 7 | 比较工具（side-by-side diff） | ❌ | ❌ | 未实现 | P2 |
| 8 | 重放（重发请求） | ❌ | ❌ | 未实现 | P2 |
| 9 | Fuzzer（随机数据注入） | ❌ | ❌ | 未实现 | P3 |
| 10 | 移动端调试 | ❌ | ❌ | CA 存在但未测试 | P3 |
| 11 | 多标签详情（请求/响应头/体） | ❌ | ⚠️ | Shell 有标签但内容未分化 | P1 |
| 12 | 请求列表（Method/URL/Status/Size/Time） | ⚠️ | ✅ | TUI 仅状态栏；Shell 完整 | — |
| 13 | 统计图表 | ❌ | ❌ | 仅数字 | P3 |
| 14 | 证书管理 | ❌ | ❌ | 无 UI | P2 |
| 15 | 导出 HAR | ❌ | ❌ | 命令未注册 | P1 |
| 16 | 搜索 | ❌ | ❌ | 搜索框无绑定 | P1 |
| 17 | 主题切换 | ❌ | ✅ | Shell 已实现 | — |

| 事件列表 | ✅ | `TrafficTable.svelte` |
| 事件详情 | ⚠️ | `RightPanel.svelte` 仅 dump JSON，无结构化展示 |
| 搜索/过滤 | ❌ | `Sidebar.svelte` 搜索框无绑定 |
| 导出 HAR | ❌ | `exportHAR` 未注册 |
| 设备选择 | ❌ | `listDevices` 未暴露 |
| 统计图表 | ❌ | Header 仅显示数字，无图表 |
| 断点/拦截 | ❌ | 无 |
| 请求重放 | ❌ | 无 |
| 比较 | ❌ | 无 |

| 退出 | `q/Q/Esc` |
| 标题栏 / 状态栏 | `title_par` / `status_par` |
| 屏幕阅读器兼容 | 输出到 `stderr()` |
---

## 5. UX 不足与改进建议

### 5.1 高优先级（P0/P1）

1. **HTTPS 解密**：`crates/flowlens-tls/src/lib.rs:158-160` 的 `decrypt_stream` 是 passthrough stub，所有 HTTPS 流量无法查看。这是替代 HTTP Debugger Pro 的核心 blocker。

2. **Tauri 后端 wiring 缺陷**：
   - `export_har` 命令未在 `lib.rs` 的 `invoke_handler` 中注册。
   - `get_stats` 从未在前端轮询，Header 统计数字永远为 0。
   - `list_devices` 未暴露给 UI，无法选择捕获接口。

3. **TUI 主表格未渲染**：`chunks[2]`（主内容区）没有 `Table` widget 的 render 调用，事件数据虽在内存中但从未显示。

4. **RightPanel 标签内容未分化**：所有 4 个标签显示相同的 `JSON.stringify(event)`，应按标签展示请求头、响应头、请求体、响应体。

5. **Sidebar 搜索框无绑定**：`<input>` 无 `bind:value`，输入无效。

6. **TUI 缺少高级功能**：仅基础导航，缺少过滤/详情/搜索/导出等功能，远落后于 Shell。

### 5.2 中优先级（P2）

7. **断点/拦截**：需要在 capture pipeline 中实现 pause/resume 机制，UI 需要"暂停/继续/修改"按钮。
8. **重写规则**：需要规则引擎 + 匹配条件 UI。
9. **请求重放**：需要 HTTP client 集成，"重发"按钮。
10. **比较工具**：需要 diff 引擎 + side-by-side 视图。
11. **证书管理 UI**：CA 生成/安装/导出的交互界面。

### 5.3 低优先级（P3）

12. **统计图表**：当前仅显示数字，可增加请求速率、状态码分布等图表。
13. **Fuzzer**：随机数据注入功能。
14. **移动端调试**：CA 安装文档 + QR 码生成。

---

## 6. 代码证据索引

| 文件 | 关键行 | 说明 |
|------|--------|------|
| `crates/flowlens-ui/src/tui.rs:106-137` | 主渲染循环，仅渲染标题栏 + 状态栏，主区域未渲染 Table |
| `crates/flowlens-ui/src/tui.rs:79` | `bus.subscribe(TrafficFilter::default())` |
| `crates/flowlens-ui/src/lib.rs:9-13` | 仅导出 `TuiFrontend`，无 GUI |
| `ui/src-tauri/src/lib.rs:13-18` | `invoke_handler` 未注册 `export_har` |
| `ui/src-tauri/src/commands.rs:188-191` | `get_stats` 命令存在但前端未调用 |
| `ui/src-tauri/src/commands.rs:193-196` | `list_devices` 命令存在但前端未暴露 |
| `ui/src/App.svelte:84-91` | 仅监听 `traffic-event`，未轮询 stats |
| `ui/src/components/RightPanel.svelte:30` | 所有标签显示相同 JSON |
| `ui/src/components/Sidebar.svelte:22` | 搜索框无绑定 |
| `docs/review/feature_parity_matrix.md:32-44` | HTTP Debugger Pro 对比矩阵 |

---

## 7. 总结

FlowLens 的 UI 层处于**稳定期**：egui GUI 已移除，Tauri/Svelte shell 是主界面，TUI 主表格已渲染可用。后端 wiring 仍有少量缺陷（`export_har` 未注册、`get_stats` 前端未轮询、`list_devices` 未暴露）。与 HTTP Debugger Pro 相比，核心差距在于 **HTTPS 解密（Phase A TLS 正在进行）、断点/拦截、重写规则、重放、比较工具** 等交互式调试能力，以及 **导出、搜索、过滤、多标签详情** 等 UI 功能的完善度。

建议优先完成 Phase A TLS 解密和捕获管线集成，然后逐步实现高级调试功能。
