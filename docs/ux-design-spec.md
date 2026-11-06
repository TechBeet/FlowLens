# FlowLens UX/UI 设计规范

> 文档路径：`docs/ux-design-spec.md`  
> 文档版本：v1.0 (Phase C UX Design)  
> 语言：中文（技术术语保留英文）  
> 基线：`ui/` Svelte 5 + Tauri v2 桌面壳、`crates/flowlens-ui` ratatui TUI、`docs/ui-development-plan.md`、`docs/backend-ui-integration-plan.md`、`docs/mobile_ca_trust.md`、`docs/data-model.md`  
> 适用对象：产品、UX/UI 设计、前端 Svelte 5、Rust/Tauri 集成、QA、文档  
> 状态：设计规范（规划文档，不含实现代码）

---

## 1. 文档目的与范围

本规范定义 FlowLens 桌面网络分析控制台的用户体验与界面设计基线。FlowLens 是一个**无代理（proxy-less）**的 HTTP/HTTPS 抓包与调试工具，通过 Npcap/WFP、ETW/IP Helper 与本地根证书在系统协议栈中就地观察、解密并归属流量。其 UI 必须同时支持：

1. **实时流量工作台**：低延迟展示海量事件，支持过滤、排序、详情、导出。
2. **交互式调试**：断点暂停/修改/释放、自动重写规则、请求重放、会话对比、模糊测试。
3. **TLS/CA 管理**：证书生成、安装、卸载、状态、导出 PEM、移动端信任指引。
4. **进程与协议归属**：PID、进程名、EXE、匹配质量、HTTP/1.1、HTTP/2、WebSocket、SSE、gRPC。
5. **多前端一致性**：Tauri/Svelte 为主界面，TUI 为终端回退，两者共享事件模型与快捷键语义。
6. **可访问性与键盘优先**：开发者工具用户高度依赖键盘，必须支持完整键盘操作与屏幕阅读器。

本规范不替代 `docs/ui-development-plan.md` 的工程排期，而是为其提供 UX 契约、组件需求、状态定义与验收标准。

---

## 2. 设计目标与原则

### 2.1 产品定位

FlowLens 的目标体验对标 Wireshark / Charles Proxy / Fiddler / Chrome DevTools Network，但差异点是：

- **无代理架构**：不修改系统代理、不监听本地端口、不安装 VPN 网卡。
- **进程归属优先**：每个请求/响应都应尽量关联到 PID、进程名、EXE 与匹配质量。
- **TLS MITM 可控**：解密必须以 CA 信任状态为前提，UI 要明确展示“未解密/已解密/解密失败”。
- **AI/MCP 友好**：界面状态与后端模型应能被 MCP 工具查询，字段命名保持稳定。
- **Windows 优先**：桌面端以 Windows 10+ 为第一目标，保留跨平台扩展可能。

### 2.2 UX 设计原则

1. **信息密度优先，但不混乱**：网络分析需要高密度表格，详情按需展开；避免把所有字段同时塞入首屏。
2. **实时性优先**：捕获运行时，UI 必须保持 30fps 以上滚动与 1 秒内统计刷新；后台任务不得阻塞主线程。
3. **状态可解释**：捕获引擎、TLS 解密、CA 信任、断点暂停、重写命中都必须有明确状态与原因。
4. **安全默认**：CA 私钥不展示；敏感 body 默认可遮罩；破坏性操作（清空、卸载 CA、删除会话）需确认。
5. **键盘优先**：所有核心动作都有快捷键；鼠标操作是增强，不是唯一路径。
6. **渐进披露**：基础用户先看到 Method/URL/Status/Size/Time；高级字段（TLS、进程、头部、body、hex）放入详情与标签。
7. **错误可恢复**：失败时提供下一步动作（安装 CA、切换设备、重试、导出日志），而不是只显示报错。
8. **主题与对比度**：深色/浅色主题都必须满足 WCAG AA 对比度；状态色不能仅靠颜色区分。

### 2.3 非目标

- 不在本规范中规定 Rust/Tauri 实现代码。
- 不重新设计已有 CSS 变量主题体系，只扩展语义映射。
- 不覆盖内核驱动安装器的完整 GUI 流程（仅定义入口与状态）。

---

## 3. 用户画像与核心场景

### 3.1 主要用户

| 用户 | 目标 | 典型任务 |
|---|---|---|
| 前端/后端开发者 | 调试 API、WebSocket、SSE、gRPC | 查看请求/响应、修改 header/body、重放、对比 |
| 安全研究员 | 分析 TLS 流量、进程归属、异常行为 | 过滤主机/PID、查看 TLS SNI/密码套件、导出 HAR |
| QA/测试工程师 | 验证网络行为与错误状态 | 搜索状态码、按进程过滤、保存会话、重放 |
| 移动开发者 | 调试手机 App/浏览器 HTTPS | 安装 CA、扫码、验证解密、定位 App 流量 |
| AI Agent (MCP) | 自动查询与分析流量 | 调用 get_sessions/get_request/aggregate 等工具 |

### 3.2 核心场景

1. **启动捕获并定位异常请求**：选择设备 → 开始捕获 → 过滤 5xx → 查看详情 → 导出 HAR。
2. **HTTPS 解密验证**：检查 CA 状态 → 安装 CA → 捕获浏览器流量 → 确认 TLS 行显示解密后 HTTP。
3. **断点修改请求**：创建断点规则 → 触发请求 → 在暂停队列修改 URL/header/body → 释放 → 观察响应。
4. **自动重写**：创建重写规则 → 命中请求 → 查看“规则已应用”标记 → 验证修改结果。
5. **重放与对比**：选择历史会话 → 重放 → 对比原始与新响应 → 导出差异。
6. **移动端调试**：生成 CA PEM/QR → 手机扫码安装并信任 → 捕获手机流量 → 验证 App 请求解密。

---

## 4. 信息架构（Information Architecture）

### 4.1 顶层信息域

FlowLens 的信息分为 6 个域：

1. **Live Capture（实时捕获）**：当前进程内存中的事件流、过滤、详情、断点触发。
2. **Sessions（会话历史）**：SQLite 中的持久化 `TrafficSession`，支持查询、分页、详情、删除、导出。
3. **Intercept（拦截与重写）**：断点规则、暂停队列、重写规则、修改对话框。
4. **Replay & Compare（重放与对比）**：重放请求、会话 diff、Fuzzer（P2/P3）。
5. **TLS & CA（证书与信任）**：CA 生成/安装/卸载/状态/PEM/QR/移动指引。
6. **Stats & Settings（统计与设置）**：聚合统计、图表、设备、主题、捕获源、保留策略、MCP。

### 4.2 路由与视图状态机

桌面端使用单窗口 + 视图状态机（可选 hash 路由）：

| 路由 | 视图 | 说明 | 优先级 |
|---|---|---|---|
| `/live` | 实时捕获工作台（默认） | 捕获控制、过滤、流量表、详情、断点触发 | P0 |
| `/sessions` | 会话历史 | 查询、分页、详情、导出/删除 | P2 |
| `/intercept` | 断点与重写 | 暂停队列、断点规则、重写规则 | P1 |
| `/replay` | Replay 与对比 | 重放、diff、Fuzzer | P2/P3 |
| `/stats` | 统计仪表盘 | 计数、聚合、图表 | P2 |
| `/tls` | TLS/CA 管理 | CA 生命周期与移动信任 | P1 |
| `/settings` | 设置 | 主题、设备、捕获源、保留策略、MCP | P2 |

### 4.3 导航层级

```text
App Shell
├─ Header (品牌、捕获状态、统计、主操作、主题)
├─ Primary Navigation (视图切换)
├─ Toolbar (设备、过滤、导出、CA 快捷入口)
├─ Main Content
│  ├─ Sidebar (搜索/过滤/事件列表/规则列表)
│  ├─ Primary Pane (流量表/规则表/会话表)
│  └─ Detail Panel (请求/响应/进程/TLS/时间线)
└─ Status Bar (事件数、捕获状态、快捷键提示、错误)
```

### 4.4 对象模型与关系

- `TrafficEvent` 是 UI 实时流的最小单位，必须能映射到 `TrafficRow` 与 `TrafficSession`。
- `TrafficSession` 是持久化与对比/重放的主对象，包含连接、进程、TLS、协议、请求/响应、帧/事件、字节统计。
- `BreakpointRule` 与 `PausePoint` 属于 Intercept 域，但引用 `SessionId`。
- `RewriteRule` 属于 Intercept 域，作用于 `HttpRequest`/`HttpResponse`。
- `CaInfo` 属于 TLS 域，包含证书、指纹、安装时间；PEM 可导出但私钥不暴露给前端。
- `TrafficFilter` 是 Live 视图的查询条件，应能保存为命名过滤器（P2）。

---

## 5. 导航结构

### 5.1 顶部 Header

Header 固定在窗口顶部，高度 48-56px，包含：

- 品牌：`流光 FlowLens` + 图标。
- 捕获状态指示器：未捕获（灰）、捕获中（绿脉冲）、错误（红）。
- 统计条：总计、HTTP、TLS、WS、SSE；点击可跳转 `/stats`。
- 主操作：开始/停止捕获（运行中变“停止”，红色危险样式）。
- 主题切换：☀/☾，快捷键 F2。
- 可选：全局搜索入口（Ctrl+K）打开命令面板（P2）。

### 5.2 主导航

主导航可放在 Header 下方的水平标签栏或左侧窄栏。建议桌面端使用**顶部水平标签**以最大化表格宽度；窄窗口时折叠为左侧图标栏。

标签：

- 实时捕获 `/live`
- 会话历史 `/sessions`
- 拦截与重写 `/intercept`
- 重放与对比 `/replay`
- TLS/CA `/tls`
- 统计 `/stats`
- 设置 `/settings`

### 5.3 侧栏（Sidebar）

侧栏宽度默认 300-340px，可拖拽，最小 200px，最大 420px。职责：

- 搜索框（实时过滤 Method/URL/Info/Status/进程）。
- 快速过滤器（协议、方法、状态码范围、进程、主机）。
- 事件列表（虚拟列表，显示 Method + URL + Status）。
- 清除全部按钮（危险色，需确认或撤销）。
- 在 `/intercept` 视图中切换为规则列表/暂停队列。

### 5.4 状态栏

固定底部，高度 24-28px：

- 事件数
- 捕获状态
- 引擎类型（Npcap/WFP/Mock）
- CA 状态（已安装/未安装/未知）
- 断点暂停数
- 快捷键提示（可折叠）
- 错误消息（右侧，红色）

---

## 6. 布局模式

### 6.1 主窗口

- 默认尺寸：1280 × 800
- 最小尺寸：1000 × 600
- 最大尺寸：无限制
- 启动位置：居中
- 可拉伸、可最大化
- DPI：跟随系统缩放，最小 100%，最大 200%

### 6.2 Live 工作台布局

```text
┌──────────────────────────────────────────────────────────────┐
│ Header: 品牌 | 状态 | 统计 | 开始/停止 | 主题              │
├──────────────────────────────────────────────────────────────┤
│ Toolbar: 设备选择 | 过滤输入 | 导出 HAR | CA 快捷 | 清空    │
├──────────────┬──────────────────────────────┬───────────────┤
│ Sidebar      │ Traffic Table (virtual)      │ Detail Panel  │
│ 300px        │ flex: 1                      │ 360-420px     │
│ resizable    │ sortable columns             │ tabs          │
├──────────────┴──────────────────────────────┴───────────────┤
│ Status Bar                                                  │
└──────────────────────────────────────────────────────────────┘
```

- 三栏可拖拽分隔。
- 详情面板可折叠（Ctrl+\）。
- 侧栏可折叠（Ctrl+B）。
- 表格区域必须占据最大空间。

### 6.3 详情面板布局

详情面板宽度默认 380px，范围 300-520px。标签：

1. 概览（Overview）
2. 请求头（Request Headers）
3. 响应头（Response Headers）
4. 请求体（Request Body）
5. 响应体（Response Body）
6. TLS（TLS Session）
7. 进程（Process）
8. 连接（Connection/Timeline）
9. Hex（请求/响应 body 十六进制，P2）

每个标签支持复制、搜索、折叠、格式化 JSON、显示原始文本。

### 6.4 Intercept 视图布局

```text
┌──────────────────────────────────────────────────────────────┐
│ Header / Toolbar: 新建断点 | 新建重写 | 导入/导出规则       │
├──────────────┬──────────────────────────────┬───────────────┤
│ Rule List    │ Paused Queue / Rule Table    │ Modify Dialog │
│ 断点/重写    │ 选中项详情                    │ JSON editor   │
└──────────────┴──────────────────────────────┴───────────────┘
```

- 左侧：规则类型切换（断点/重写）+ 规则列表。
- 中间：暂停队列（高优先级）或规则表格。
- 右侧：修改器/预览。

### 6.5 TLS/CA 视图布局

- CA 状态卡片（已安装/未安装/未知、CN、指纹、安装时间）。
- 操作区：生成 CA、安装 CA、卸载 CA、导出 PEM、显示 QR、复制 PEM。
- 移动端信任指引（iOS/Android 折叠面板）。
- 风险提示：CA 私钥不导出；卸载需确认。

### 6.6 Sessions 视图布局

- 查询栏：时间范围、PID、Host、Protocol、Status、关键词。
- 结果表：分页、排序、多选。
- 详情抽屉：选中会话的完整请求/响应/TLS/进程。
- 批量操作：导出 HAR、删除。


---

## 7. 色彩与主题系统

### 7.1 现有主题变量（必须复用）

深色/浅色主题已存在于 `ui/src/styles/global.css`，变量如下：

| 变量 | 深色 | 浅色 | 语义 |
|---|---|---|---|
| `--bg` | #1e1e1e | #f9f9fa | 应用背景 |
| `--surface` | #252526 | #ffffff | 面板背景 |
| `--panel` | #2d2d30 | #eeeef0 | 控件背景 |
| `--fg` | #d4d4d4 | #242424 | 主文本 |
| `--fg-dim` | #858585 | #6a6a6a | 次级文本 |
| `--accent` | #569cd6 | #0f4c81 | 主强调/链接 |
| `--accent-fg` | #ffffff | #ffffff | 强调文本 |
| `--border` | #3c3c3c | #d0d0d1 | 边框 |
| `--ok` | #4ec9b0 | #0c783d | 成功/运行 |
| `--warn` | #dcdcaa | #85730b | 警告 |
| `--err` | #f44747 | #b32632 | 错误/危险 |
| `--hover` | #2a2d2e | #e8e8ea | 悬停 |
| `--selected` | #094771 | #cfe4f7 | 选中 |
| `--row-alt` | #232324 | #f2f2f4 | 表格斑马纹 |

### 7.2 扩展语义色（建议新增 CSS 变量）

为避免硬编码，建议新增以下语义变量：

| 变量 | 深色建议 | 浅色建议 | 用途 |
|---|---|---|---|
| `--info` | #569cd6 | #0f4c81 | 信息/提示 |
| `--paused` | #dcdcaa | #85730b | 断点暂停 |
| `--rewrite` | #c586c0 | #7a3e7a | 重写命中 |
| `--tls-ok` | #4ec9b0 | #0c783d | TLS 解密成功 |
| `--tls-fail` | #f44747 | #b32632 | TLS 解密失败 |
| `--muted` | #5a5a5a | #9a9a9a | 禁用/不可用 |
| `--focus` | #569cd6 | #0f4c81 | 焦点环 |
| `--overlay` | rgba(0,0,0,0.55) | rgba(0,0,0,0.35) | 模态遮罩 |

### 7.3 颜色使用规则

1. **状态色**：
   - 运行/成功：`--ok`
   - 警告/暂停：`--warn`
   - 错误/危险：`--err`
   - 信息/强调：`--accent`
2. **TLS 状态**：
   - 已解密：`--tls-ok` + 图标 🔓
   - 未解密/未知：`--fg-dim` + 🔒
   - 解密失败：`--tls-fail` + ⚠
3. **匹配质量**：
   - ExactTuple：`--ok`
   - Heuristic：`--warn`
   - Unspecified：`--fg-dim`
4. **HTTP 状态码**：
   - 2xx：`--ok`
   - 3xx：`--accent`
   - 4xx：`--warn`
   - 5xx：`--err`
5. **对比度**：正文文本与背景对比度 ≥ 4.5:1；大标题/图标 ≥ 3:1。
6. **不只靠颜色**：状态必须同时有图标/文本/形状。
7. **主题切换**：F2 或 Header 按钮；保存到 localStorage；首次启动跟随系统 `prefers-color-scheme`。

### 7.4 主题切换动效

- 切换时 150ms 背景/文本过渡。
- 不对表格行做动画，避免性能问题。
- 尊重 `prefers-reduced-motion`：减少或禁用过渡。


---

## 8. 组件库需求

### 8.1 基础组件（UI Primitives）

| 组件 | 需求 |
|---|---|
| `Button` | primary/secondary/danger/ghost/icon，loading/disabled，tooltip，快捷键提示 |
| `IconButton` | 32×32，aria-label，hover/focus |
| `Input` | text/search/password，clear button，debounce，aria |
| `Select` | 单选/多选，键盘导航，虚拟列表 |
| `Checkbox` / `Radio` / `Switch` | 表单控件，明确标签 |
| `Tabs` | 键盘左右切换，aria-tablist |
| `Modal` | 焦点陷阱，Esc 关闭，aria-modal |
| `Drawer` | 右侧详情抽屉，可拖拽宽度 |
| `Toast` | 成功/警告/错误/信息，自动消失，可操作 |
| `Tooltip` | 延迟 500ms，支持快捷键 |
| `Badge` | 状态/计数/协议标签 |
| `ProgressBar` | 确定/不确定，aria-valuenow |
| `Skeleton` | 加载占位 |
| `EmptyState` | 图标 + 标题 + 说明 + CTA |
| `ErrorBanner` | 错误 + 重试/详情 |
| `ResizablePanel` | 拖拽分隔，最小/最大，持久化宽度 |
| `VirtualList` | 10k+ 行，固定/动态行高 |
| `CodeViewer` | 等宽字体，复制，折叠，行号 |
| `JsonViewer` | 格式化/原始/折叠 |
| `DiffViewer` | side-by-side/unified，高亮增删 |
| `HexViewer` | 偏移/十六进制/ASCII |
| `StatusDot` | 颜色+动画+aria |
| `Spinner` | 不确定进度 |

### 8.2 业务组件

| 组件 | 职责 |
|---|---|
| `AppShell` | 整体布局、路由、全局快捷键 |
| `Header` | 品牌、状态、统计、主操作、主题 |
| `PrimaryNav` | 视图切换 |
| `CaptureToolbar` | 设备、过滤、导出、CA、清空 |
| `Sidebar` | 搜索、快速过滤、事件列表 |
| `TrafficTable` | 虚拟表格、排序、选择、状态色 |
| `DetailPanel` | 多标签详情、复制、格式化 |
| `ProcessDetail` | PID/名称/路径/架构/匹配质量 |
| `TlsDetail` | TLS 版本、密码套件、SNI、ALPN、证书指纹 |
| `FilterBar` | 过滤 DSL 输入与 chips |
| `AdvancedFilterEditor` | 条件组合 UI |
| `PausedQueue` | 断点暂停会话列表 |
| `BreakpointRuleEditor` | 断点条件表单 |
| `RewriteRuleEditor` | 重写条件与动作表单 |
| `RuleList` | 规则表格、启用开关、优先级 |
| `ReplayDialog` | 重放参数与结果 |
| `CompareView` | 会话选择与 diff |
| `CaStatusCard` | CA 状态与操作 |
| `CaQrPanel` | PEM/QR 展示 |
| `MobileTrustGuide` | iOS/Android 步骤 |
| `StatsDashboard` | 计数、聚合、图表 |
| `SessionsTable` | 历史会话查询 |
| `SettingsView` | 设置表单 |
| `CommandPalette` | Ctrl+K 命令搜索（P2） |

### 8.3 状态管理契约

前端建议使用 Svelte 5 Runes：

- `captureStore`：running、isLoading、error、engineKind、device。
- `eventsStore`：events、selectedEvent、filters、sort、virtualization。
- `statsStore`：stats、lastUpdated。
- `caStore`：status、pem、isInstalling。
- `interceptStore`：pausedSessions、breakpointRules、rewriteRules、selectedRule。
- `uiStore`：theme、sidebarCollapsed、detailCollapsed、panelWidths、reducedMotion。
- `sessionsStore`：query、page、items、selectedSession。

所有 store 必须能从 Tauri 事件更新，并支持 optimistic UI + 回滚。

### 8.4 类型同步规则

- Rust `serde` JSON 为唯一契约来源（或引入 `ts-rs` 生成 TS）。
- 字段命名：Rust snake_case → TS camelCase 显式映射；避免隐式。
- `Option<T>` → `T | null`。
- `Vec<u8>` → base64 字符串 + `contentType`。
- 时间戳：微秒整数；前端格式化为 `HH:MM:SS.d`，详情显示完整 ISO。
- 枚举：字符串联合类型。
- 错误：Tauri 命令返回 `Result<T, String>`，前端统一 `try/catch` + Toast。

---

## 9. 响应式与窗口行为

### 9.1 断点

| 断点 | 宽度 | 行为 |
|---|---|---|
| XL | ≥1440px | 三栏全开，详情 420px |
| L | 1280-1439px | 三栏，详情 380px |
| M | 1024-1279px | 侧栏可折叠，详情 340px |
| S | 800-1023px | 侧栏折叠为图标，详情变为抽屉 |
| 最小 | 1000×600 | 核心功能仍可用；详情/侧栏可覆盖 |

### 9.2 面板行为

- 侧栏/详情面板支持拖拽调整宽度，双击分隔条恢复默认。
- 折叠状态与宽度持久化到 localStorage。
- 小窗口下详情面板以右侧抽屉覆盖，Esc 关闭。
- 表格列在窄屏下隐藏次要列（源/目的地址、进程）并可通过列设置恢复。

### 9.3 高 DPI 与缩放

- 字体最小 12px，表格 12px，详情 13px。
- 图标使用 SVG/字体图标，避免位图模糊。
- 支持 Ctrl + 滚轮缩放（P2）。

### 9.4 性能

- 事件流默认上限 10,000（前端），超出自动裁剪头部；可配置 1k/5k/10k/50k。
- 表格虚拟滚动，行高固定 28px。
- 详情面板按需解析 body，大 body（>1MB）显示“加载/截断”并提供“查看全部”。
- 统计轮询 1 秒，事件监听实时。
- 搜索/过滤 debounce 150ms。

---

## 10. 可访问性（Accessibility）

### 10.1 键盘

- 所有可交互元素可通过 Tab 聚焦。
- 焦点环：2px `--focus`，不被裁剪。
- 模态框焦点陷阱与恢复。
- 表格支持箭头导航与 Enter 打开详情。
- 快捷键不与屏幕阅读器/浏览器冲突（桌面 webview 中避免 Ctrl+W 等）。

### 10.2 屏幕阅读器

- 使用语义 HTML：header/nav/main/aside/footer。
- 表格使用 `<table>` + `scope`。
- 状态变化通过 `aria-live="polite"` 宣布（捕获开始/停止、断点暂停）。
- 图标按钮必须有 `aria-label`。
- 颜色状态附带文本。

### 10.3 动效与对比

- 尊重 `prefers-reduced-motion`。
- 尊重 `prefers-contrast: high`：增加边框与对比度。
- 不使用闪烁动画；脉冲动画可禁用。

### 10.4 国际化

- UI 文案集中管理，支持中文/英文（P2）。
- 时间/数字格式按 locale。
- 快捷键显示随平台调整（macOS Cmd）。

---

## 11. 键盘快捷键

### 11.1 全局

| 快捷键 | 动作 |
|---|---|
| F2 | 切换主题 |
| Ctrl+K | 命令面板（P2） |
| Ctrl+B | 折叠/展开侧栏 |
| Ctrl+\ | 折叠/展开详情面板 |
| Ctrl+, | 打开设置 |
| Ctrl+Shift+E | 导出 HAR |
| Ctrl+L | 清空事件（需确认） |
| Esc | 关闭模态/抽屉/取消选择 |
| ? | 显示快捷键帮助 |

### 11.2 Live 工作台

| 快捷键 | 动作 |
|---|---|
| Space | 开始/停止捕获 |
| / | 聚焦搜索框 |
| ↑/↓ 或 j/k | 选择上/下一行 |
| Enter | 打开选中事件详情 |
| o | 在详情中切换标签 |
| c | 复制当前 URL |
| p | 暂停选中会话（若匹配断点） |
| F | 清空事件 |
| D | 切换调试模式（显示原始事件） |
| Ctrl+F | 聚焦过滤输入 |

### 11.3 Intercept

| 快捷键 | 动作 |
|---|---|
| N | 新建断点规则 |
| Shift+N | 新建重写规则 |
| R | 释放选中暂停会话 |
| M | 修改选中暂停会话 |
| E | 编辑选中规则 |
| Delete | 删除选中规则（确认） |
| Space | 启用/禁用选中规则 |

### 11.4 TLS/CA

| 快捷键 | 动作 |
|---|---|
| I | 安装 CA |
| U | 卸载 CA（确认） |
| C | 复制 PEM |
| Q | 显示 QR |

### 11.5 TUI 对齐

TUI 保留 `q/Q/Esc` 退出、`j/k`、`f/F` 清空、`d/D` 调试模式。桌面端语义应一致，但不强制所有 TUI 按键。


---

## 12. 工作流设计

### 12.1 捕获生命周期

#### 状态机

```text
Idle -> Starting -> Running -> Stopping -> Idle
          │          │
          └─ Error ──┘
```

- **Idle**：未捕获；显示“开始捕获”主按钮。
- **Starting**：按钮 loading；禁用设备切换；超时 10 秒。
- **Running**：状态指示器绿色脉冲；统计轮询；事件流入。
- **Stopping**：按钮 loading；等待 pump task 结束（最多 2 秒）。
- **Error**：显示错误 Banner；提供重试、查看日志、切换设备/Mock。

#### 设备选择

- 启动前可选择设备；运行中禁用或提示“停止后切换”。
- 设备列表显示友好名称 + GUID；无设备时显示空状态与 Npcap 安装指引。
- Mock 模式明确标记“模拟数据”，避免误导。

#### 事件上限

- 默认 10,000；可配置。
- 达到 80% 时状态栏警告“事件接近上限，将自动裁剪”。
- 裁剪时 Toast 提示“已裁剪 N 条旧事件”。
- 提供“暂停自动滚动”以便查看历史。

#### 清空事件

- 快捷键/按钮触发二次确认或 5 秒撤销 Toast。
- 清空后重置选中与详情。

### 12.2 断点工作流（Breakpoint）

#### 规则模型

`BreakpointRule` 字段：

- 名称、启用状态
- 方法匹配（GET/POST/... 或正则）
- URL 正则
- 状态码匹配
- Header 匹配（key/value）
- Body 包含（字符串/bytes）

#### 创建/编辑

- 表单分组：基本信息、请求匹配、响应匹配、高级。
- 提供“测试匹配”按钮：对当前选中事件试运行，显示是否命中。
- 正则输入实时校验，错误时红色边框 + 提示。
- 保存后规则立即生效；禁用规则灰色显示。

#### 暂停流程

1. 请求命中断点规则。
2. `InterceptEngine::pause(session_id)` 创建 `PausePoint`。
3. UI 收到 `paused-session` 事件，暂停队列新增条目并高亮。
4. 捕获继续，但该会话挂起；状态栏显示“N 个会话已暂停”。
5. 用户在暂停队列选择会话 → 详情显示原始请求/响应 → 可修改。
6. 释放：`release` 或 `modify` 后继续处理。
7. 超时：默认 30 秒自动释放，Toast 警告“断点超时已自动释放”。

#### 修改对话框

- 分标签：请求、响应。
- 字段：Method、URL、Headers（键值对编辑器）、Body（文本/JSON/Hex）。
- 提供“应用重写规则”选项（先应用自动规则再手动修改）。
- 提供“恢复原始”按钮。
- 校验：URL 格式、Header 名称、JSON 语法；错误阻止释放。
- 释放前预览 diff（原始 vs 修改）。

#### 并发与死锁防护

- 暂停会话数上限（默认 50）；超过时新命中自动释放并记录警告。
- 超时自动释放。
- UI 显示每个暂停会话的剩余时间。

### 12.3 重写工作流（Rewrite）

#### 规则模型

`RewriteRule` 字段：

- 名称、启用、优先级
- 匹配条件：method_regex、url_regex、header_match、body_contains
- 动作列表：SetUrl、SetMethod、SetHeader、AddHeader、RemoveHeader、SetBody

#### 管理

- 规则列表支持拖拽调整优先级或数字输入。
- 启用/禁用开关即时生效。
- 规则命中时流量表行显示“RW”徽章，详情显示命中的规则与 diff。
- 支持导入/导出 JSON 规则文件。
- 提供“试运行”：选择一个历史请求，预览修改结果，不发送。

#### 应用顺序

1. 按优先级升序。
2. 每个规则匹配后应用动作，生成 `ModifiedRequest`。
3. 若某规则动作失败，记录警告并继续下一规则（不中断）。
4. 最终请求进入网络层。

#### 冲突处理

- 同一 header 被多个规则设置：后执行覆盖，详情显示最终值。
- Body 重写后更新 Content-Length（自动或提示）。
- URL 重写后更新 Host（可选自动）。

### 12.4 CA 安装工作流

#### 桌面 CA 生命周期

1. 检查状态：`check_ca_status("FlowLens CA")`。
2. 若未安装：显示“安装 CA”按钮与风险说明。
3. 安装：可能需要管理员权限；失败时显示 UAC/权限指引。
4. 安装成功：更新状态卡片，显示指纹与安装时间。
5. 导出 PEM：复制或保存文件。
6. 卸载：二次确认；卸载后状态更新。
7. 生成新 CA：仅在未安装或用户明确重置时允许；警告旧证书失效。

#### 状态展示

- 已安装：绿色徽章 + 指纹（SHA-256 前 12 位）+ 安装时间。
- 未安装：黄色徽章 + “HTTPS 流量无法解密”提示。
- 未知/错误：红色徽章 + 错误详情。

#### 风险与提示

- CA 安装会信任 FlowLens 解密所有 HTTPS，仅在可信环境使用。
- 不导出私钥。
- 移动端信任需要额外步骤。


---

## 13. 状态设计：空、加载、错误

### 13.1 空状态

| 场景 | 文案 | CTA |
|---|---|---|
| 无流量 | “暂无网络流量” | 开始捕获 |
| 搜索无结果 | “未匹配到事件” | 清除搜索 |
| 无设备 | “未检测到捕获设备” | 安装 Npcap / 使用 Mock |
| 无暂停会话 | “当前没有暂停的会话” | 创建断点规则 |
| 无重写规则 | “尚未创建重写规则” | 新建规则 |
| 无历史会话 | “暂无历史会话” | 开始捕获并导出 |
| CA 未安装 | “CA 未安装，HTTPS 无法解密” | 安装 CA |

空状态应包含图标、简短说明、下一步按钮；避免空白面板。

### 13.2 加载状态

- 初始加载：Skeleton（表格行、卡片）。
- 命令执行：按钮内 spinner + 禁用。
- 大 body：进度条 + “正在加载…”。
- 统计轮询：不显示全屏 loading，仅数字更新。
- 设备加载：下拉框内 spinner。

### 13.3 错误状态

- 错误 Banner：图标 + 标题 + 详情 + 操作（重试/复制错误/查看文档）。
- Toast：短暂错误（3-5 秒），可点击查看详情。
- 行内错误：表单字段红色边框 + 文本。
- 网络/权限错误：提供具体指引（管理员权限、Npcap、防火墙）。
- TLS 解密失败：在详情面板显示“解密失败：原因”，不阻断其他流量。

### 13.4 部分失败

- 设备列表加载失败但可使用 Mock。
- 统计轮询失败静默重试，不打断用户。
- 导出失败保留已生成文件路径并提示。
- 断点超时自动释放并记录。

---

## 14. 移动 CA 信任工作流

### 14.1 目标

让用户在 iOS/Android 设备上信任 FlowLens CA，以便解密移动端 HTTPS 流量。桌面 UI 必须提供：

- CA PEM 展示与复制
- QR 码（编码 PEM）
- 分步指引（iOS/Android）
- 状态验证（捕获到移动端流量且 TLS 解密成功）
- 已知限制说明

### 14.2 桌面端 CA 面板

组件：`CaStatusCard` + `CaQrPanel` + `MobileTrustGuide`。

状态：

1. **未生成 CA**：提示生成 CA（或自动）。
2. **已生成未安装**：显示 PEM/QR，提示“移动端信任前，建议先在桌面安装 CA”。
3. **已安装**：显示指纹、安装时间、PEM/QR。
4. **PEM 已导出**：显示保存路径与 QR。

操作：

- 生成/重新生成 CA（确认）
- 安装到 Windows（管理员）
- 卸载
- 复制 PEM
- 保存 PEM 文件
- 显示/隐藏 QR
- 刷新状态

### 14.3 iOS 工作流（UI 文案）

1. 扫描 QR 或通过邮件打开 PEM。
2. 设置 → 通用 → VPN 与设备管理 → 安装描述文件。
3. 设置 → 通用 → 关于本机 → 证书信任设置 → 开启 FlowLens CA 完全信任。
4. 返回 FlowLens，开始捕获，验证 iPhone 流量解密。

UI 提供折叠步骤与“我已完成”确认按钮；确认后触发一次 CA 状态检查与流量验证提示。

### 14.4 Android 工作流（UI 文案）

1. 扫描 QR 或下载 PEM。
2. 设置 → 安全 → 加密与凭据 → CA 证书/VPN 与应用证书。
3. 选择 FlowLens CA，启用 VPN 与应用。
4. 返回 FlowLens，开始捕获，验证 App 流量解密。

注意：Android 12+ 可能限制系统级 CA 信任，UI 必须提示“部分 App 可能仍不信任用户 CA；如失败，需使用可调试 App 或 root 设备”。

### 14.5 验证与反馈

- 移动端流量出现且 TLS 解密成功：显示绿色“移动端解密正常”。
- 出现 TLS 握手但无 HTTP：提示“CA 未被信任或 App 忽略用户 CA”。
- 无移动端流量：提示“确保设备与 FlowLens 主机在同一网络 / 代理配置”（若使用代理模式）。

### 14.6 已知限制

- Android 7+ 默认不信任用户 CA（部分 App）。
- iOS 需要“完全信任”开关。
- 企业 MDM 可自动推送 CA（P3）。
- macOS 信任流程未覆盖（P3）。

---

## 15. 数据与类型契约（UI 相关）

### 15.1 UiEvent 扩展建议

当前 `UiEvent` 仅含基础字段。建议扩展（保持向后兼容，新增字段 optional）：

- `session_id?: number`
- `protocol?: "http1" | "http2" | "websocket" | "sse" | "grpc" | "unknown"`
- `process_name?: string`
- `pid?: number`
- `match_quality?: "exact" | "heuristic" | "unspecified"`
- `tls_version?: string`
- `sni?: string`
- `cipher_suite?: string`
- `request_headers?: Record<string,string>`
- `response_headers?: Record<string,string>`
- `request_body?: string` (base64 + mime)
- `response_body?: string`
- `paused?: boolean`
- `rewrite_applied?: string[]` (rule ids)
- `timestamp_micros?: number`

### 15.2 分页与查询

- Sessions 查询返回 `{ items, total, page, page_size }`。
- 默认 page_size 50，最大 200。
- 排序字段白名单，防止注入。

### 15.3 二进制数据

- body 以 base64 传输，附带 `content_type` 与 `size`。
- 前端按 MIME 决定渲染：JSON/XML/HTML/文本/图片/hex。
- 大 body 提供“下载/保存”而非内联。

---

## 16. 性能与容量基线

| 指标 | 目标 |
|---|---|
| 10,000 行表格滚动 | ≥30fps |
| 事件从捕获到显示 | <100ms |
| 统计面板刷新 | 1s，不阻塞主线程 |
| 详情面板打开 | <50ms（元数据），大 body 异步 |
| 内存占用 | <500MB（10k 事件 + 详情） |
| 断点暂停处理 | <50ms |
| CA 状态检查 | <200ms |

---

## 17. 安全与隐私

1. CA 私钥永远不发送到前端；仅证书 PEM、指纹、元数据。
2. 请求/响应 body 默认可遮罩（密码、token）；提供“显示敏感数据”开关。
3. 导出 HAR 时提示可能包含敏感数据。
4. 日志不记录 body 或 header 中的 Authorization/Cookie（可配置）。
5. 卸载 CA、清空事件、删除会话需要确认。
6. 断点修改请求前显示 diff，避免误操作。


---

## 18. 验收标准（Definition of Done）

每个功能交付必须满足：

1. 功能完整且可交互验证。
2. TypeScript strict 无类型错误；Rust clippy 无警告（或已说明）。
3. 深色/浅色主题下颜色与对比度正常。
4. 窗口最小 1000×600 时核心功能可用。
5. 性能基线达标。
6. Tauri 命令异常时前端有用户可见提示。
7. 文档更新（本规范 + 用户指南）。
8. 可访问性：键盘可完成核心流程，屏幕阅读器可读状态。

---

## 19. 优先级与路线图（UX 视角）

### P0（必须，2-3 周）

- Live 工作台三栏布局、虚拟表格、排序、选择。
- 过滤/搜索绑定与快速过滤。
- 统计轮询与 Header 状态。
- 设备选择与 Mock 状态。
- CA 状态卡片与安装/卸载。
- 空/加载/错误状态。

### P1（高，3-4 周）

- 详情面板标签分化与复制。
- 断点规则编辑器与暂停队列。
- 重写规则编辑器与列表。
- TLS 详情与进程详情。
- 移动 CA 信任指引与 QR。

### P2（中，3-4 周）

- Sessions 历史与分页。
- Replay 与 Compare。
- Stats 仪表盘与图表。
- 命令面板与快捷键配置。
- Hex/原始 body 查看。

### P3（低）

- Fuzzer UI。
- MCP 状态面板。
- 国际化。
- 远程捕获。

---

## 20. 附录

### 20.1 推荐目录结构（前端）

```text
ui/src/
├─ components/
│  ├─ ui/                 # 基础原语
│  ├─ layout/             # AppShell/Header/Nav
│  ├─ live/               # TrafficTable/DetailPanel
│  ├─ intercept/          # Rules/PausedQueue
│  ├─ tls/                # CaStatus/Qr/MobileGuide
│  ├─ sessions/
│  └─ stats/
├─ lib/
│  ├─ api/                # Tauri 命令封装
│  ├─ stores/             # Svelte 5 runes stores
│  ├─ types.ts
│  └─ utils/
└─ styles/
   ├─ global.css
   └─ tokens.css
```

### 20.2 关键状态文案（中文）

- 捕获中：`捕获中`
- 未捕获：`未捕获`
- CA 已安装：`CA 已安装`
- CA 未安装：`CA 未安装，HTTPS 无法解密`
- 断点暂停：`已暂停 (N)`
- 重写命中：`已应用重写规则`
- TLS 已解密：`TLS 已解密`
- TLS 未解密：`TLS 未解密`

### 20.3 术语表

| 英文 | 中文 |
|---|---|
| Capture | 捕获 |
| Breakpoint | 断点 |
| Rewrite Rule | 重写规则 |
| Replay | 重放 |
| Diff / Compare | 对比 |
| CA | 证书颁发机构 / 根证书 |
| PEM | PEM 证书文本 |
| SNI | 服务器名称指示 |
| Process Attribution | 进程归属 |
| Match Quality | 匹配质量 |

---

*本文档为 FlowLens UX/UI 设计规范 v1.0。随实现与后端 API 演进同步更新。*

