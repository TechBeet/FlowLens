# FlowLens UI 开发计划（Svelte 5 + Tauri v2）

> 文档版本：1.0  
> 输出路径：`docs/ui-development-plan.md`  
> 适用对象：UI 架构师、前端开发、Rust/Tauri 集成开发、QA  
> 语言：中文（技术术语保留英文）

---

## 1. 目标与范围

### 1.1 目标

将当前最小化 Tauri/Svelte 界面升级为**覆盖 FlowLens 全部后端能力的桌面网络分析控制台**。目标体验对标 Wireshark / Charles / Fiddler，但保留 FlowLens 的“无代理、系统协议栈就地解密、进程归属、MCP 集成”特性。

### 1.2 设计原则

1. **Svelte 5 Runes 优先**：使用 `$state`、`$derived`、`$effect` 管理响应式状态，避免过度复杂的 store。
2. **类型先行**：TypeScript 类型与 Rust `serde` 模型一一对应；新增字段先更新 `ui/src/lib/types.ts` 与 Tauri `UiEvent`。
3. **组件化**：基础 UI 原语与业务组件分离，便于测试和复用。
4. **事件驱动**：实时流量通过 Tauri `traffic-event` 推送；重操作采用命令调用 + 进度/结果面板。
5. **主题与可访问性**：保留现有 CSS 变量主题体系，支持暗色/亮色切换和键盘操作。
6. **性能优先**：流量列表必须虚拟化，支持 10 万+ 事件不卡顿；详情面板按需解析大 body。
7. **契约优先**：对尚未实现的后端命令先定义前端契约与 mock，再联调。

### 1.3 当前基线与差距

| 模块 | 当前状态 | 主要差距 |
|---|---|---|
| 捕获控制 | `start_capture` / `stop_capture` / `get_stats` / `list_devices` | 缺少捕获状态、引擎类型、错误详情、设备切换 |
| 流量表 | 基础列、排序、选择 | 缺少协议/进程/源目的地址列、虚拟化、批量操作 |
| 详情面板 | 请求/响应头与 body | 缺少 TLS、进程、连接、时间线、十六进制、复制 |
| TLS/CA | 安装/卸载/状态 | 缺少证书展示、指纹、信任指引、移动设备流程 |
| 断点 | 后端完整 | 前端几乎无 UI |
| 重写规则 | 后端完整 | 前端无 UI |
| Replay | 后端完整 | 前端无 UI |
| 会话对比 | 后端占位 | 前端无 UI，后端需接入 `diff` |
| HAR 导出 | 已实现 | 缺少导入、进度、过滤导出 |
| 过滤/搜索 | 侧栏简单搜索 | 缺少 DSL、快速过滤、保存过滤器 |
| 统计 | 轮询 stats | 缺少仪表盘、图表、聚合 |
| 协议视图 | 无 | HTTP/2、WebSocket、SSE、gRPC 专项视图 |
| 设置 | 无 | 主题、设备、捕获源、保留策略、MCP |
| Mock 模式 | 后端支持 | 前端无显式开关/状态 |

---

## 2. 后端能力矩阵与 UI 覆盖要求

| # | 后端能力 | 后端位置 / 命令 | UI 覆盖模块 | 优先级 |
|---|---|---|---|---|
| 1 | 捕获控制 | `start_capture`, `stop_capture`, `get_stats`, `list_devices` | `CaptureControlBar`, `DeviceSelector`, `StatusBar` | P0 |
| 2 | 实时事件流 | Tauri `traffic-event` emit, `UiEvent` | `TrafficTable`, `EventStreamStore` | P0 |
| 3 | 流量详情 | `TrafficEvent`, `TrafficRow`, `TrafficSession` | `DetailPanel`, `*Viewer` | P0 |
| 4 | 进程归属 | `ProcessInfo`, `MatchQuality` | `ProcessColumn`, `ProcessDetail` | P1 |
| 5 | TLS/CA | `install_ca`, `uninstall_ca`, `check_ca_status`, `get_ca_pem`, `cert_store` | `TlsView`, `CaManagerModal` | P1 |
| 6 | 断点拦截 | `InterceptEngine`, `pause/release/modify/list_paused` | `InterceptView`, `PausedQueue`, `ModifyDialog` | P1 |
| 7 | 重写规则 | `RewriteEngine`, `add/remove/toggle/list` | `RewriteRulesView`, `RuleEditor` | P1 |
| 8 | Replay | `replay_request`, `ReplayClient` | `ReplayDialog`, `ReplayResult` | P2 |
| 9 | 会话对比 | `compare_sessions`（占位）, `diff_sessions` | `SessionCompareView` | P2 |
| 10 | HAR 导出/导入 | `export_har`, `HarExporter`, `SazImporter` | `ExportDialog`, `ImportDialog` | P2 |
| 11 | 过滤/搜索 | `TrafficFilter`, `parse_filter` | `FilterBar`, `AdvancedFilterEditor` | P0 |
| 12 | 统计仪表盘 | `TrafficStats`, MCP 聚合 | `StatsDashboard` | P2 |
| 13 | 协议专项 | `ProtocolKind`, HTTP/2, WS, SSE, gRPC 模型 | `ProtocolTabs`, 专项 viewers | P2 |
| 14 | 设置 | 主题、设备、捕获源、保留策略、MCP | `SettingsView` | P2 |
| 15 | Mock 模式 | `Engine::Mock`, `spawn_mock_producer` | `CaptureControlBar` 状态 | P1 |
| 16 | MCP | `McpServer`, tools | `McpStatusPanel`（可选） | P3 |
| 17 | 会话存储 | `StorageEngine`（Rust trait） | `SessionsView` | P2 |
| 18 | 主题 | `loadTheme/applyTheme/saveTheme` | `ThemeToggle` | P0 |
## 3. 信息架构与视图路由

虽然 Tauri 是单窗口桌面应用，但建议采用**视图状态机**（hash 路由可选）组织功能，避免所有功能堆叠在一个页面。

### 3.1 主视图

| 路由 | 视图 | 说明 |
|---|---|---|
| `/live` | 实时捕获工作台（默认） | 捕获控制、过滤、流量表、详情 |
| `/sessions` | 会话历史 | 查询、分页、详情、导出/删除 |
| `/intercept` | 断点与重写 | 暂停队列、断点规则、重写规则 |
| `/replay` | Replay 与对比 | 重放请求、会话 diff、Fuzz |
| `/stats` | 统计仪表盘 | 计数、聚合、图表 |
| `/tls` | TLS/CA 管理 | CA 生成/安装/卸载/信任指引 |
| `/settings` | 设置 | 主题、设备、捕获源、保留策略、快捷键 |
| `/help` | 帮助 | 快捷键、文档、诊断 |

### 3.2 全局布局

```text
┌─────────────────────────────────────────────────────────────┐
│ AppShell / TitleBar (捕获状态、主题、快捷操作)              │
├──────────────┬──────────────────────────────┬───────────────┤
│ Navigation   │ Main Workspace               │ Detail/Tools  │
│ Rail         │ (当前视图)                   │ Drawer        │
│ (可折叠)     │                              │ (可折叠)      │
├──────────────┴──────────────────────────────┴───────────────┤
│ StatusBar (事件数、捕获状态、错误、快捷键提示)               │
└─────────────────────────────────────────────────────────────┘
```

- 支持拖拽调整三栏宽度，状态持久化到 `localStorage`。
- 详情面板可停靠右侧或弹出为独立窗口（Tauri multi-window，P3）。

---

## 4. 组件树

### 4.1 顶层组件树

```text
AppShell
├── TitleBar
│   ├── Brand
│   ├── CaptureStatusBadge
│   ├── QuickStats
│   ├── ThemeToggle
│   └── GlobalActions (export, CA, settings)
├── NavigationRail
│   └── NavItem[] (live, sessions, intercept, replay, stats, tls, settings, help)
├── MainWorkspace
│   ├── LiveView
│   │   ├── CaptureControlBar
│   │   ├── FilterBar
│   │   ├── TrafficTable (virtualized)
│   │   ├── EmptyState
│   │   └── StatusBar
│   ├── SessionsView
│   ├── InterceptView
│   ├── ReplayView
│   ├── StatsView
│   ├── TlsView
│   ├── SettingsView
│   └── HelpView
├── DetailDrawer
│   ├── DetailHeader
│   ├── DetailTabs
│   └── TabPanels
│       ├── OverviewTab
│       ├── RequestTab
│       ├── ResponseTab
│       ├── TlsTab
│       ├── ProcessTab
│       ├── ProtocolTab
│       ├── TimingTab
│       └── RawTab
├── ModalHost
│   ├── CaManagerModal
│   ├── ExportDialog
│   ├── ImportDialog
│   ├── ReplayDialog
│   ├── CompareDialog
│   ├── BreakpointRuleEditor
│   ├── RewriteRuleEditor
│   └── ConfirmDialog
├── ToastHost
└── CommandPalette (可选)
```

### 4.2 基础 UI 原语（`ui/src/components/ui/`）

- `Button`, `IconButton`, `SplitButton`
- `Input`, `Select`, `Checkbox`, `Toggle`, `Slider`
- `Tabs`, `TabPanel`
- `Modal`, `Drawer`, `Popover`, `Tooltip`
- `Table`, `VirtualList`, `ResizablePanel`
- `Badge`, `Tag`, `StatusDot`
- `JsonViewer`, `HexViewer`, `DiffViewer`
- `EmptyState`, `ErrorState`, `LoadingSpinner`
- `Toast`, `NotificationBanner`

### 4.3 业务组件（`ui/src/components/`）

- 捕获：`CaptureControlBar`, `DeviceSelector`, `CaptureStatusBadge`
- 流量：`TrafficTable`, `TrafficRow`, `FilterBar`, `QuickFilterChips`, `AdvancedFilterEditor`
- 详情：`DetailPanel`, `HeadersViewer`, `BodyViewer`, `TlsInfoPanel`, `ProcessInfoPanel`, `TimingPanel`
- 协议：`Http2StreamView`, `WebSocketTimeline`, `SseEventList`, `GrpcCallView`
- TLS：`CaStatusCard`, `CaInstallGuide`, `CertificateFingerprint`
- 断点：`PausedSessionQueue`, `BreakpointRuleList`, `BreakpointRuleEditor`, `ModifySessionDialog`
- 重写：`RewriteRuleList`, `RewriteRuleEditor`, `RewriteMatchPreview`
- Replay：`ReplayForm`, `ReplayResultPanel`, `SessionCompareView`, `DiffResultView`
- 统计：`StatsCards`, `ProtocolPieChart`, `TopHostsTable`, `TopProcessesTable`
- 设置：`SettingsForm`, `ShortcutSettings`

### 4.4 状态管理（Svelte 5 Runes）

建议在 `ui/src/lib/stores/` 下创建按领域划分的状态模块：

- `capture.svelte.ts`：运行状态、设备、引擎类型、错误
- `events.svelte.ts`：事件数组、选中事件、过滤结果、缓存上限
- `stats.svelte.ts`：轮询/推送统计
- `filter.svelte.ts`：快速过滤 + DSL 过滤器
- `selection.svelte.ts`：选中行、多选、详情
- `ca.svelte.ts`：CA 状态、PEM、安装进度
- `intercept.svelte.ts`：暂停队列、断点规则
- `rewrite.svelte.ts`：重写规则列表
- `settings.svelte.ts`：主题、布局、快捷键
- `ui.svelte.ts`：当前视图、面板折叠、toast

状态通过 `$state` 创建，组件通过 props / context 共享。复杂派生使用 `$derived`。

---

## 5. 数据流与 Tauri 集成

### 5.1 事件流

```text
Rust capture engine
  -> EventBus (BroadcastEventBus)
  -> pump task (commands.rs)
  -> Tauri emit("traffic-event", UiEvent)
  -> Svelte $effect listener
  -> events store
  -> derived filteredEvents
  -> TrafficTable
```

### 5.2 命令调用层扩展（`ui/src/lib/api.ts`）

建议按领域拆分 API 模块：

- `captureApi.ts`：`startCapture`, `stopCapture`, `getStats`, `listDevices`, `clearEvents`
- `eventsApi.ts`：事件监听封装、取消订阅
- `caApi.ts`：`installCA`, `uninstallCA`, `checkCAStatus`, `getCAPem`
- `interceptApi.ts`：`pauseSession`, `releaseSession`, `modifySession`, `listPausedSessions`, 断点规则 CRUD
- `rewriteApi.ts`：`addRewriteRule`, `removeRewriteRule`, `toggleRewriteRule`, `listRewriteRules`
- `replayApi.ts`：`replayRequest`, `compareSessions`
- `exportApi.ts`：`exportHAR`, `importHAR`/`importSAZ`
- `settingsApi.ts`：读写设置

### 5.3 Tauri 命令扩展建议

| 命令 | 用途 | 备注 |
|---|---|---|
| `get_stats` | 获取流量统计 | 前端需定期轮询或改为事件推送 |
| `list_devices` | 列出可用网卡 | 已有，需前端暴露 |
| `export_har` | 导出 HAR 文件 | 已注册，需前端文件保存对话框 |
| `get_ca_pem` | 获取 CA PEM | 当前返回 `None`，需实现 |
| `subscribe_stats` | 订阅统计更新事件 | 建议新增，替代轮询 |
| `clear_events` | 清空已捕获事件 | 建议新增 |
| `get_session_detail` | 获取单条会话完整详情 | 建议新增 |
| `import_saz` | 导入 SAZ 文件 | 可选，Phase 4 |
| `get_rewrite_rule_history` | 重写规则应用历史 | 可选 |

### 5.4 `UiEvent` 扩展建议

当前 `UiEvent` 仅包含 `id/time/kind/info/size/src_addr/dst_addr/method/url/status`。建议扩展为：

```ts
interface UiEvent {
  id: string;
  session_id?: number;
  time: string;
  kind: string;
  protocol?: 'http1' | 'http2' | 'websocket' | 'sse' | 'grpc' | 'tls' | 'connection';
  info: string;
  size: number;
  src_addr?: string;
  dst_addr?: string;
  process?: { pid: number; name: string; exe_path?: string };
  method?: string;
  url?: string;
  status?: string;
  request_headers?: Record<string, string>;
  response_headers?: Record<string, string>;
  request_body?: string;
  response_body?: string;
  tls_version?: string;
  cipher_suite?: string;
  sni?: string;
  websocket_frames?: unknown[];
  sse_events?: unknown[];
  grpc_call?: unknown;
}
```

### 5.5 性能策略

- 流量表使用虚拟滚动（如 `svelte-virtual` 或自定义）。
- 事件数组设置上限（如 10,000），超出后写入磁盘或丢弃旧事件。
- 大 body 默认折叠/截断，点击“加载完整内容”。
- 统计轮询间隔 1s，后续改为事件推送。
- 详情面板按需解析 JSON，避免每次渲染重复 `JSON.stringify`。

---

## 6. Feature-to-Component 映射

| 后端功能 | 主要组件 | 状态存储 | Tauri 命令 |
|---|---|---|---|
| 捕获开始/停止 | `CaptureControlBar` | `capture` | `start_capture`, `stop_capture` |
| 设备选择 | `DeviceSelector` | `capture` | `list_devices` |
| Mock 模式 | `CaptureControlBar` | `capture` | `start_capture`（内部自动） |
| 实时流量表 | `TrafficTable` | `events` | `traffic-event` |
| 过滤/搜索 | `FilterBar`, `AdvancedFilterEditor` | `filter` | 前端本地 + 后端 `TrafficFilter` |
| 流量详情 | `DetailPanel` | `selection` | `get_session_detail`（建议新增） |
| 进程归属 | `ProcessColumn`, `ProcessDetail` | `events` | 事件内 `ProcessInfo` |
| TLS 信息 | `TlsInfoPanel` | `selection` | 事件内 `TlsSession` |
| CA 管理 | `TlsView`, `CaManagerModal` | `ca` | `install_ca`, `uninstall_ca`, `check_ca_status`, `get_ca_pem` |
| 断点规则 | `BreakpointRuleList`, `BreakpointRuleEditor` | `intercept` | 断点规则 CRUD（建议新增命令） |
| 暂停队列 | `PausedSessionQueue` | `intercept` | `list_paused_sessions` |
| 修改会话 | `ModifySessionDialog` | `intercept` | `modify_session` |
| 释放会话 | `PausedSessionQueue` | `intercept` | `release_session` |
| 重写规则 | `RewriteRuleList`, `RewriteRuleEditor` | `rewrite` | `add/remove/toggle/list_rewrite_rules` |
| Replay | `ReplayForm`, `ReplayResultPanel` | `replay` | `replay_request` |
| 会话对比 | `SessionCompareView`, `DiffResultView` | `replay` | `compare_sessions`（需接入 diff） |
| Fuzz | `FuzzPanel`（可选） | `replay` | 建议新增命令 |
| HAR 导出 | `ExportDialog` | `export` | `export_har` |
| HAR/SAZ 导入 | `ImportDialog` | `sessions` | 建议新增命令 |
| 统计仪表盘 | `StatsView` | `stats` | `get_stats`, MCP 聚合 |
| 主题切换 | `ThemeToggle` | `settings` | 前端 localStorage |
| 设置 | `SettingsView` | `settings` | 前端 localStorage + 后端配置（可选） |

---

## 7. UI/UX 规范

### 7.1 布局

- 三栏布局：导航栏（可折叠）、主内容区、详情面板（可折叠）。
- 可拖拽分隔条，宽度持久化。
- 最小窗口：1000×600，核心功能仍可用。

### 7.2 主题

- 使用现有 CSS 变量（`ui/src/styles/global.css`）。
- 支持深色/浅色，F2 快捷键切换，持久化到 `localStorage`。
- 所有组件必须在两种主题下可读。

### 7.3 流量表列

建议列顺序：

| 列 | 说明 |
|---|---|
| # | 序号 |
| Time | 时间戳 |
| Protocol | HTTP/1.1, HTTP/2, WS, SSE, gRPC, TLS |
| Method | GET/POST/... |
| Host / Path | URL 或 host/path |
| Status | 状态码 |
| Size | 请求/响应大小 |
| Process | 进程名 (PID) |
| Source | 源地址 |
| Destination | 目的地址 |

### 7.4 详情面板标签

- Overview：基本信息、进程、连接、TLS 摘要
- Request：请求头、请求体、原始请求
- Response：响应头、响应体、原始响应
- TLS：TLS 版本、密码套件、SNI、ALPN、证书指纹
- Process：PID、进程名、EXE 路径、用户 SID、架构、匹配质量
- Protocol：HTTP/2 流、WebSocket 帧、SSE 事件、gRPC 调用
- Timing：时间线、耗时
- Raw：十六进制/文本原始数据

### 7.5 交互规范

- 键盘：上下选择、Enter 打开详情、F2 主题、Ctrl+F 过滤、Esc 清空。
- 右键菜单：复制 URL、复制 cURL、Replay、Pause、Open in new tab。
- 空状态与错误状态必须有明确提示。
- 所有异步操作显示 loading 状态。

### 7.6 可访问性

- 使用语义化 HTML，支持键盘导航。
- 焦点可见，`aria-label` 完整。
- 颜色对比度符合 WCAG AA。

---

## 8. 分阶段实施路线图

### Phase 0：基线重构与工程化（1 周）

目标：修复现有缺陷，建立组件化基础。

任务：
- 修复 Sidebar 搜索框 `bind:value`。
- 修复 Header 统计轮询（或改为事件推送）。
- 扩展 `UiEvent` 与 TS 类型。
- 建立 `ui/src/components/ui/` 基础原语。
- 建立 `ui/src/lib/stores/` 状态模块。
- 配置 ESLint/Prettier/TypeScript strict。

交付物：
- 可运行的重构后 UI。
- 类型同步文档。

验收标准：
- `npm run build` 无类型错误。
- 搜索、统计、主题正常工作。

### Phase 1：实时捕获工作台完善（1-2 周）

目标：完成 LiveView 核心体验。

任务：
- 实现虚拟滚动流量表。
- 增加协议、进程、源/目的地址列。
- 完善 FilterBar（快速过滤 + DSL）。
- 完善 DetailPanel 标签页。
- 添加空状态、错误状态、loading 状态。
- 添加键盘导航与右键菜单。

交付物：
- 完整的实时捕获工作台。

验收标准：
- 10,000 条事件滚动流畅。
- 过滤、排序、选择、详情正常。

### Phase 2：TLS/CA 管理与进程归属（1 周）

目标：完成 TLS 与进程信息展示。

任务：
- 实现 `get_ca_pem` 后端命令。
- 完善 `TlsView`：CA 状态、指纹、安装/卸载、信任指引。
- 完善 `ProcessDetail` 与进程聚合。
- 添加移动设备 CA 信任指引（QR code、步骤）。

交付物：
- TLS/CA 管理页面。
- 进程归属详情。

验收标准：
- CA 安装/卸载/状态检查正常。
- 进程信息正确展示。

### Phase 3：断点与重写（2 周）

目标：完成拦截与重写 UI。

任务：
- 新增断点规则 CRUD 命令。
- 实现 `InterceptView`：暂停队列、断点规则列表/编辑器。
- 实现 `ModifySessionDialog`。
- 实现 `RewriteRulesView` 与 `RewriteRuleEditor`。
- 添加自动超时释放机制。

交付物：
- 断点与重写管理界面。

验收标准：
- 能创建/编辑/删除断点与重写规则。
- 暂停会话可修改并释放。

### Phase 4：Replay、对比与 HAR（2 周）

目标：完成 Replay、会话对比与 HAR 导入导出。

任务：
- 接入 `compare_sessions` 到 `diff_sessions`。
- 实现 `ReplayForm` 与 `ReplayResultPanel`。
- 实现 `SessionCompareView` 与 `DiffResultView`。
- 完善 `ExportDialog`（过滤导出、进度）。
- 实现 `ImportDialog`（HAR/SAZ）。

交付物：
- Replay 与对比界面。
- HAR 导入导出。

验收标准：
- 能重放请求并查看响应。
- 能对比两个会话并展示差异。
- 能导出/导入 HAR。

### Phase 5：协议专项视图（2 周）

目标：完成 HTTP/2、WebSocket、SSE、gRPC 专项视图。

任务：
- 实现 `Http2StreamView`。
- 实现 `WebSocketTimeline`。
- 实现 `SseEventList`。
- 实现 `GrpcCallView`。
- 在 DetailPanel 中按协议切换视图。

交付物：
- 协议专项视图。

验收标准：
- 各协议数据正确展示。
- 大帧/大消息可分页或折叠。

### Phase 6：统计仪表盘与会话管理（1-2 周）

目标：完成统计与会话历史。

任务：
- 实现 `StatsView`：计数、聚合、图表。
- 实现 `SessionsView`：查询、分页、详情、删除。
- 接入 MCP 聚合工具（可选）。

交付物：
- 统计仪表盘。
- 会话管理页面。

验收标准：
- 统计数据实时更新。
- 会话查询与分页正常。

### Phase 7：设置、主题、可访问性、性能优化（1-2 周）

目标：完成设置与体验优化。

任务：
- 实现 `SettingsView`。
- 完善主题切换与持久化。
- 键盘快捷键配置。
- 性能优化：虚拟滚动、缓存上限、按需解析。
- 可访问性审计与修复。

交付物：
- 设置页面。
- 性能优化报告。

验收标准：
- 设置持久化正常。
- 性能基线达标。

### Phase 8：测试、打包与发布（1 周）

目标：完成测试与发布准备。

任务：
- 单元测试（Vitest）。
- 组件测试。
- E2E 测试（可选）。
- Tauri 打包配置。
- 文档与用户指南。

交付物：
- 测试报告。
- 发布包。

验收标准：
- 测试通过率 ≥ 90%。
- 打包成功。

---

## 9. 测试与质量保障

### 9.1 测试策略

- **单元测试**：过滤逻辑、排序、重写规则匹配、DSL 解析。
- **组件测试**：关键组件渲染与交互。
- **集成测试**：Tauri 命令调用。
- **E2E 测试**：完整用户流程（可选）。

### 9.2 工具

- Vitest + Testing Library（前端）。
- Playwright（E2E，可选）。
- Rust `cargo test`（后端）。

### 9.3 性能基线

- 10,000 条流量表格滚动 ≥ 30fps。
- 统计面板刷新不阻塞主线程。
- 内存占用 < 500MB。

---

## 10. 交付物清单

| 交付物 | 说明 |
|---|---|
| `docs/ui-development-plan.md` | 本开发计划 |
| `ui/src/components/ui/*` | 基础 UI 原语 |
| `ui/src/components/*` | 业务组件 |
| `ui/src/lib/stores/*` | 状态管理模块 |
| `ui/src/lib/api/*` | Tauri 命令封装 |
| `ui/src/lib/types.ts` | TypeScript 类型定义 |
| `ui/src-tauri/src/commands.rs` | Tauri 命令扩展 |
| 测试文件 | 单元/组件/E2E 测试 |
| 用户文档 | 操作说明 |

---

## 11. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| Tauri WebView2 首次启动慢 | 用户体验 | 优化构建资源，提供加载动画 |
| 大量流量导致前端内存占用高 | UI 卡顿 | 虚拟滚动 + 事件缓存上限 + 自动写入磁盘 |
| TLS 解密失败或未完成 | HTTPS 流量不可查 | 右侧面板显示“未解密”回退，不阻塞主流程 |
| 断点导致会话挂死 | 捕获阻塞 | 自动超时释放（默认 30 秒） |
| 跨平台打包问题 | 发布受阻 | 优先 Windows，后续评估 macOS/Linux |
| Svelte 5 Runes 学习曲线 | 开发效率 | 团队内部分享，统一组件规范 |
| 后端命令占位 | 功能不可用 | 前端契约优先，mock 数据联调 |

---

## 12. 验收标准（Definition of Done）

每个阶段完成时，必须满足：

1. 功能完整：阶段内所有功能点已实现并可交互验证。
2. 类型安全：TypeScript `strict` 模式下无类型错误，Rust 后端 `clippy` 无警告。
3. 主题兼容：深色/浅色主题下所有组件颜色、对比度正常。
4. 响应式布局：窗口最小 1000×600 时核心功能仍可用。
5. 性能基线：10,000 条流量表格滚动帧率 ≥ 30fps，统计面板刷新不阻塞主线程。
6. 错误处理：Tauri 命令异常时前端有用户可见提示（非静默吞掉）。
7. 文档更新：新增功能对应的用户操作说明已写入用户文档。

---

## 13. 后续扩展路线（Post-MVP）

- [ ] 实时流量拦截代理模式（未来，若产品方向转变）
- [ ] 插件系统（基于 MCP，允许第三方编写 UI 插件）
- [ ] 远程捕获（将本机 UI 连接到远程 FlowLens Agent）
- [ ] AI 辅助分析（接入 MCP，让 AI 自动分析流量异常、建议重写规则）
- [ ] 移动端伴随应用（扫码安装 CA，远程查看手机端流量）
- [ ] 流量回放录制（将捕获会话录制为可回放的脚本）

---

> 本文档为 FlowLens 桌面 UI 的全面开发规划，供 UI 架构师、前端开发与 Rust 后端集成人员共同参考。随实施进度与后端 API 演进，本文档应同步更新。

