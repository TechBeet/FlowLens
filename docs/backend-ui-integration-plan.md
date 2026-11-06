# FlowLens 后端-to-UI 集成计划

> 文档版本：1.0  
> 输出路径：`docs/backend-ui-integration-plan.md`  
> 适用对象：Rust/Tauri 集成x发、Svelte 前端x发、架构设计、QA  
> 语言：中文（技术术语保留英文）  
> 基线：`ui/src-tauri` Tauri v2 shell + Svelte 5 前端，后端 crate：`flowlens-core`、`flowlens-capture`、`flowlens-tls`、`flowlens-storage`、`flowlens-replay`、`flowlens-mcp`

---

## 1. 目标与范围
本计划用于把 FlowLens 现有 Rust 后端能力完整、稳定地暴露给 Tauri/Svelte UI，明确：
1. Tauri 命令清单、签名、返回值、前端调用状态。
2. 事件总线到前端事件映射。
3. 缺失命令与扩展 API 契约。
4. 数据流架构。
5. 类型同步规则。
6. API 契约、错误与版本兼容。

---

## 2. 当前基线与关键发现

### 2.1 后端桥接层
`ui/src-tauri/src/lib.rs` 注册命令、`ui/src-tauri/src/state.rs` AppState、`ui/src-tauri/src/commands.rs` 实现。

### 2.2 已注册 Tauri 命令（19 个）
`start_capture`, `stop_capture`, `get_stats`, `list_devices`, `export_har`, `install_ca`, `uninstall_ca`, `check_ca_status`, `get_ca_pem`, `pause_session`, `release_session`, `modify_session`, `list_paused_sessions`, `add_rewrite_rule`, `remove_rewrite_rule`, `toggle_rewrite_rule`, `list_rewrite_rules`, `replay_request`, `compare_sessions`。

### 2.3 事件流
Capture Engine -> BroadcastEventBus -> pump task -> TrafficRow::from_event -> UiEvent::from_row -> Tauri emit("traffic-event") -> Svelte listen。

生命周期事件：capture-started / capture-stopped。

### 2.4 已识别缺陷
参数名不匹配（export_har output_path, install_ca pem vs cn），get_ca_pem 未实现，stats 未推送，设备选择未暴露，UiEvent 信息不足，二进制序列化，compare_sessions 占位，Intercept/Rewrite 未接入管线，Storage 未接入，TLS 解密 stub，broadcast Lagged 未处理，事件上限。

---

## 3. Tauri 命令清单
见第 3.1 节总表。关键问题：
- export_har 前端传 path，后端参数 output_path。
- CA API 语义不匹配。
- 前端只封装 12 个命令，intercept/rewrite/replay/compare 未封装。
- 修改建议详见第 7 节。

---

## 4. 事件总线映射
BroadcastEventBus 发布 TrafficEvent，订阅过滤后在 pump task 处理。当前 UiEvent 仅含 id/time/kind/info/size/src_addr/dst_addr/method/url/status。

建议扩展字段：session_id, protocol, process, tls, headers, body 等。

建议新增事件：stats-updated, paused-session, rewrite-rule-applied, replay-result, ca-status-changed, capture-error。

处理 broadcast Lagged，避免 pump task 终止。

---

## 5. 数据流架构
捕获、事件、命令双向流。AppState 建议扩展存储、SessionManager、CaManager、selected_device、capture_engine_kind。

捕获、断点、重写、Replay、CA、存储流程概述。详见文档。

---

## 6. 类型同步
规则：Serde -> JSON -> TS，snake_case 与 camelCase 映射，Option -> nullable，Vec<u8> -> base64，时间戳微秒，枚举字符串。

核心模型映射：ConnectionKey、ProcessInfo、TlsSession、HttpRequest/Response、TrafficEvent、TrafficRow/UiEvent、TrafficStats、TrafficFilter、BreakpointRule、PausePoint、RewriteRule/Action/Match、DiffResult、ReplayResult、CaInfo、StorageQuery、FuzzResult。

建议引入 ts-rs 生成类型，暂保持手动同步并增加 checklist。

---

## 7. 缺失命令与扩展
P0：start_capture 支持设备参数、get_ca_pem 实现、clear_events、stats 推送。
P1：会话详情、列表、查询、删除、TLS/CA 生成/设置、断点 CRUD、get_paused_request/response。
P2：重写预览、Replay 增强、compare_sessions 实现、import HAR/SAZ、聚合统计。
P3：MCP 控制、诊断。

每个命令提供签名、返回类型、前后端映射。

---

## 8. API 契约
Tauri invoke 参数名严格匹配，错误以 String 返回，建议格式 `<module>: <op> failed: <detail>`。事件 payload 必须可序列化。

分页、查询、时间戳、二进制、枚举、版本兼容规范。

---

## 9. 路线图
Phase0 修复 wiring 与类型，Phase1 实时工作台，Phase2 TLS/CA，Phase3 断点重写，Phase4 Replay/对比/导入导出，Phase5 统计/MCP/设置。

---

## 10. 验收标准
编译通过、类型检查、实时流量、设备选择、HAR 导出、CA 流程、断点/重写/Replay/对比可用、10k 事件流畅、错误提示明确。

---

*本文档为集成基线，变更模型或命令时需同步更新本文档与 ui/src/lib/types.ts。*
## 附录 A：Tauri 命令详细清单（补充）

### A.1 命令总表

| # | 命令 | Rust 函数 | 输入 | 输出 | 前端 API | 状态 | 备注 |
|---|---|---|---|---|---|---|---|
| 1 | `start_capture` | `commands::start_capture` | `device?: string`（建议扩展） | `()` | `startCapture(device?)` | ✅ | 启动 Npcap/mock，spawn pump，emit `capture-started` |
| 2 | `stop_capture` | `commands::stop_capture` | 无 | `()` | `stopCapture()` | ✅ | 停止 pump，emit `capture-stopped` |
| 3 | `get_stats` | `commands::get_stats` | 无 | `TrafficStats` | `getStats()` | ✅ | pump task 维护 |
| 4 | `list_devices` | `commands::list_devices` | 无 | `string[]` | `listDevices()` | ✅ | Npcap 设备列表 |
| 5 | `export_har` | `commands::export_har` | `output_path: string` | `()` | `exportHAR(path)` | ⚠️ | 参数名需统一为 `output_path` |
| 6 | `install_ca` | `commands::install_ca` | `pem: string` | `()` | `installCA(pem)` | ⚠️ | 前端当前传 `cn`，需改为 PEM |
| 7 | `uninstall_ca` | `commands::uninstall_ca` | `pem: string` | `()` | `uninstallCA(pem)` | ⚠️ | 同上 |
| 8 | `check_ca_status` | `commands::check_ca_status` | `pem: string` | `boolean` 或 `CaStatus` | `checkCAStatus(pem)` | ⚠️ | 建议返回 `CaStatus` |
| 9 | `get_ca_pem` | `commands::get_ca_pem` | 无 | `Option<string>` | `getCAPem()` | ❌ | 当前固定 `None`，需接入 InMemoryTlsEngine |
| 10 | `pause_session` | `commands::pause_session` | `session_id: u64` | `()` | 封装建议 | ⚠️ | 未传 request/response/rule_id |
| 11 | `release_session` | `commands::release_session` | `session_id: u64` | `()` | 封装建议 | ✅ | - |
| 12 | `modify_session` | `commands::modify_session` | `session_id: u64, new_data` | `()` | 封装建议 | ✅ | 支持 request/response |
| 13 | `list_paused_sessions` | `commands::list_paused_sessions` | 无 | `PausePoint[]` | 封装建议 | ✅ | - |
| 14 | `add_rewrite_rule` | `commands::add_rewrite_rule` | `rule: JsonValue` | `()` | 封装建议 | ✅ | 反序列化为 RewriteRule |
| 15 | `remove_rewrite_rule` | `commands::remove_rewrite_rule` | `id: string` | `boolean` | 封装建议 | ✅ | - |
| 16 | `toggle_rewrite_rule` | `commands::toggle_rewrite_rule` | `id: string, enabled: bool` | `boolean` | 封装建议 | ✅ | - |
| 17 | `list_rewrite_rules` | `commands::list_rewrite_rules` | 无 | `JsonValue[]` | 封装建议 | ✅ | - |
| 18 | `replay_request` | `commands::replay_request` | `session_id: u64` | `HttpResponse` | 封装建议 | ✅ | 仅从 InterceptEngine 获取 request |
| 19 | `compare_sessions` | `commands::compare_sessions` | `id_a: u64, id_b: u64` | `JsonValue` | 封装建议 | ❌ | 占位，需接入 diff_sessions |

### A.2 命令实现细节

`start_capture`：重置 stats，spawn pump task，订阅 TrafficFilter::default()，启动 Engine::auto()；Pcap 路径同时运行 mock producer。

`stop_capture`：设置 running=false，取出 pump task 并在 2 秒内等待结束，emit capture-stopped。

`export_har`：从 events_store 读取 TrafficEvent，匹配 HttpRequest/HttpResponse 生成 HAR 1.2，body 使用 base64。

`pause_session`：调用 InterceptEngine::pause(session_id, None, None, None)。

`modify_session`：从 JSON 提取 request/response，调用 InterceptEngine::modify。

`replay_request`：从 InterceptEngine::get_request 获取 request，调用 ReplayClient::replay。

`compare_sessions`：当前仅返回占位 JSON。

### A.3 命令注册

`invoke_handler` 已注册全部 19 个命令。前端 api.ts 仅封装其中 12 个（捕获 4 + CA 4 + 导出 1 + 无 intercept/rewrite/replay/compare）。


## ��¼ B���¼�����ӳ��

### B.1 BroadcastEventBus
publish(TrafficEvent) / subscribe(TrafficFilter) -> Receiver���ڲ����� dummy receiver����ֹ publish ʧ�ܡ�recent Vec<TrafficRow> ���գ�evt_cnt ͳ�ơ�

### B.2 TrafficEvent -> UiEvent ӳ��
ConnectionEstablished->connection, PacketReceived->packet, TlsHandshake->tls, HttpRequest->http_request, HttpResponse->http_response, WebSocketFrame->websocket, SseEvent->sse, GrpcCall->grpc, ConnectionClosed->closed, CaptureDropped->dropped.

### B.3 UiEvent ��ǰ�ֶ�
id, time, kind, info, size, src_addr?, dst_addr?, method?, url?, status?��������չ session_id, protocol, process, headers, body, tls_version, cipher_suite, sni �ȡ�

### B.4 ��ѹ����
recv() ���� Lagged Ӧ��¼������������������ Closed ���˳���

### B.5 �����¼��б�
stats-updated, events-cleared, paused-session, paused-session-released, rewrite-rule-applied, replay-result, ca-status-changed, capture-error��

### B.6 �������
������� 50k��ǰ��������� 5k-10k������������ storage��


## ��¼ C���������ܹ������䣩

�������� -> BroadcastEventBus -> pump task -> TrafficRow::from_event -> UiEvent -> Tauri emit -> Svelte store -> �����
��������ǰ�� invoke -> commands.rs -> AppState / ���� crate -> Result<T,String>��

AppState ������չ��storage, session_manager, ca_manager, selected_device, capture_engine_kind��RewriteEngine ӦΪ Arc<Mutex<...>> �Ա�����乲���

�������̣�start_capture(device?) -> running=true -> reset stats/events -> spawn pump -> Engine::auto() / new_pcap(device) -> spawn_engine_pump / mock_producer -> emit capture-started��

�ϵ����̣�HttpRequest -> ƥ��ϵ���� -> InterceptEngine::pause -> emit paused-session -> �ȴ� release/modify -> ������
��д���̣�HttpRequest -> RewriteEngine::apply_rules -> ModifiedRequest -> �������� -> emit rewrite-rule-applied��
Replay ���̣�replay_request(session_id) -> InterceptEngine::get_request �� StorageEngine::get_session().http_request -> ReplayClient::replay -> ������Ӧ��
�Ա����̣�compare_sessions(id_a,id_b) -> StorageEngine ��ȡ�Ự -> diff_sessions -> DiffResult��
CA ���̣�generate_ca(cn) -> CaInfo -> get_ca_pem() / install_ca(pem) / check_ca_status(pem)��
�洢���̣�insert_session / get_session / list_sessions / query_sessions / delete_sessions��

