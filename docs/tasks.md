# FlowLens 任务拆解

## Phase 1: 用户态 MVP

### 任务 1: 项目骨架
- **目标**: 创建工作区、基础 Cargo.toml、README
- **输入**: 无
- **输出**: workspace Cargo.toml、Crate 目录结构
- **依赖**: 无
- **验收标准**: `cargo build --workspace` 成功
- **复杂度**: ★☆☆☆☆

### 任务 2: flowlens-core 基础模型
- **目标**: 实现 ConnectionKey、ProcessInfo、TrafficEvent 枚举
- **输入**: 无
- **输出**: flowlens-core/src/models.rs
- **依赖**: 无
- **验收标准**: 单元测试覆盖 80%+
- **复杂度**: ★★☆☆☆

### 任务 3: CLI 框架
- **目标**: 基础命令行参数解析、管理员检查
- **输入**: 项目骨架
- **输出**: flowlens-cli/src/main.rs
- **依赖**: clap
- **验收标准**: `flowlens --help` 显示帮助
- **复杂度**: ★★☆☆☆

### 任务 4: Npcap 捕获引擎
- **目标**: 基础 pcap 捕获、过滤表达式
- **输入**: 无
- **输出**: flowlens-capture/src/npcap_engine.rs
- **依赖**: pcap crate, tokio
- **验收标准**: 能捕获 loopback HTTP 流量
- **复杂度**: ★★★★☆

### 任务 5: ETW 归属引擎
- **目标**: ETW 进程创建/退出事件订阅
- **输入**: 无
- **输出**: flowlens-attribution/src/etw_engine.rs
- **依赖**: windows-sys crate
- **验收标准**: 能获取 calc.exe 的 PID
- **复杂度**: ★★★★☆

### 任务 6: CA 生成和 MITM
- **目标**: rcgen 生成 CA、rustls TLS 中断
- **输入**: 无
- **输出**: flowlens-tls/src/mitm.rs
- **依赖**: rcgen, rustls
- **验收标准**: 能解密 google.com HTTPS
- **复杂度**: ★★★★★

### 任务 7: HTTP/1.1 解析
- **目标**: httparse 解析请求/响应
- **输入**: TLS MITM 解密输出
- **输出**: flowlens-protocol/src/http1.rs
- **依赖**: httparse
- **验收标准**: 正确解析 GET / HTTP/1.1
- **复杂度**: ★★★☆☆

### 任务 8: SQLite 存储
- **目标**: 创建会话表、插入、查询
- **输入**: Core 模型
- **输出**: flowlens-storage/src/db.rs
- **依赖**: sqlx, tokio-postgres (for SQLite)
- **验收标准**: 插入 1000 条记录查询 < 10ms
- **复杂度**: ★★★☆☆

### 任务 9: TUI 基础界面
- **目标**: ratatui 会话列表、请求详情视图
- **输入**: 事件总线
- **输出**: flowlens-ui/src/tui.rs
- **依赖**: ratatui, crossterm
- **验收标准**: 按键响应 < 100ms
- **复杂度**: ★★★☆☆

### 任务 10: 事件总线集成
- **目标**: 将所有模块连接到广播系统
- **输入**: 所有子任务完成
- **输出**: flowlens-core/src/event_bus.rs
- **依赖**: tokio::sync::broadcast
- **验收标准**: 捕获 -> 归属 -> TLS -> 解析 -> 存储 -> UI 全链路
- **复杂度**: ★★★★☆

---

## Phase 2: 多协议支持

### 任务 11: HTTP/2 解析
- **目标**: h2 crate 解析帧、头部压缩
- **输入**: TLS 解密流
- **输出**: flowlens-protocol/src/http2.rs
- **依赖**: h2, hpack
- **验收标准**: 解析 h2spec 基础测试
- **复杂度**: ★★★★☆

### 任务 12: WebSocket 支持
- **目标**: tungstenite 帧解析、掩码处理
- **输入**: HTTP/1.1 Upgrade 或 HTTP/2
- **输出**: flowlens-protocol/src/websocket.rs
- **依赖**: tungstenite
- **验收标准**: 正确解析 echo.websocket.org
- **复杂度**: ★★★☆☆

### 任务 13: SSE 解析
- **目标**: text/event-stream 流式解析
- **输入**: HTTP 响应体
- **输出**: flowlens-protocol/src/sse.rs
- **依赖**: 无
- **验收标准**: 解析 GitHub Events API
- **复杂度**: ★★☆☆☆

### 任务 14: gRPC 支持
- **目标**: h2 + prost 解析二进制帧
- **输入**: HTTP/2 TLS 流
- **输出**: flowlens-protocol/src/grpc.rs
- **依赖**: h2, prost
- **验收标准**: 解析 grpc.io 示例服务
- **复杂度**: ★★★★★

### 任务 15: HAR 导出
- **目标**: JSON 格式导出符合 HAR 1.2
- **输入**: TrafficSession 集合
- **输出**: flowlens-export/src/har.rs
- **依赖**: serde_json
- **验收标准**: 导出文件可被 Chrome 导入
- **复杂度**: ★★☆☆☆

### 任务 16: SAZ 导入
- **目标**: ZIP 解包 + 基本 HTTP 重建
- **输入**: .saz 文件路径
- **输出**: flowlens-export/src/saz.rs
- **依赖**: zip crate
- **验收标准**: 导入 Fiddler 基础会话
- **复杂度**: ★★★★☆

### 任务 17: 高级过滤器
- **目标**: 时间范围、URL 包含、大小过滤
- **输入**: TrafficFilter 结构
- **输出**: flowlens-storage/src/filter.rs
- **依赖**: 无
- **验收标准**: 组合过滤返回正确结果
- **复杂度**: ★★☆☆☆

---

## Phase 3: MCP 服务器

### 任务 18: MCP 基础设施
- **目标**: JSON-RPC 2.0 实现、工具注册
- **输入**: 无
- **输出**: flowlens-mcp/src/server.rs
- **依赖**: jsonrpc-core, tokio
- **验收标准**: 本地连接成功响应 ping
- **复杂度**: ★★★☆☆

### 任务 19: 查询会话工具
- **目标**: get_sessions, get_request 实现
- **输入**: StorageEngine
- **输出**: flowlens-mcp/src/tools/query.rs
- **依赖**: flowlens-core 模型
- **验收标准**: Claude Code 调用返回分页数据
- **复杂度**: ★★★☆☆

### 任务 20: 聚合工具
- **目标**: aggregate_by_pid, aggregate_by_host
- **输入**: 会话存储
- **输出**: flowlens-mcp/src/tools/aggregate.rs
- **依赖**: 同上
- **验收标准**: 返回统计数据 < 500ms
- **复杂度**: ★★★☆☆

---

## Phase 4: WFP Callout 驱动

### 任务 21: 驱动框架
- **目标**: 基础 DriverEntry、卸载例程
- **输入**: 无
- **输出**: driver/src/driver.c
- **依赖**: WDK
- **验收标准**: 在测试 VM 中加载成功
- **复杂度**: ★★★★★

### 任务 22: WFP Callout 注册
- **目标**: ALE AUTH CONNECT/V4/V6 层注册
- **输入**: 驱动框架
- **输出**: driver/src/callout.c
- **依赖**: FWPSK 动态链接库
- **验收标准**: 能捕获 SYN 包
- **复杂度**: ★★★★★

### 任务 23: IOCTL 通信
- **目标**: 用户态 ↔ 内核态数据交换
- **输入**: 共享内存设计
- **输出**: driver/src/ioctl.c + flowlens-capture/wfp.rs
- **依赖**: Windows Driver Model
- **验收标准**: 用户态能读取捕获的包
- **复杂度**: ★★★★★

### 任务 24: 共享内存环形缓冲区
- **目标**: 高效零拷贝数据传输
- **输入**: 无
- **输出**: driver/src/shared_mem.rs
- **依赖**: Windows 内存管理
- **验收标准**: 10Gbps 流量无丢包
- **复杂度**: ★★★★★

### 任务 25: 驱动签名和安装器
- **目标**: EV 证书签名或测试模式
- **输入**: 完成的驱动二进制
- **输出**: flowlens-installer/src/sign.rs
- **依赖**: signtool, wix
- **验收标准**: 在干净 Windows 上免提示安装
- **复杂度**: ★★★★☆

### 任务 26: Npcap 自动回退
- **目标**: WFP 加载失败时切换回 Npcap
- **输入**: CaptureEngine trait
- **输出**: flowlens-capture/src/select.rs
- **依赖**: 无
- **验收标准**: 卸载驱动后仍能工作
- **复杂度**: ★★☆☆☆

---

## Phase 5: 性能优化

### 任务 27: 零拷贝环形缓冲区
- **目标**: 使用 mmap 和 内存屏障
- **输入**: 现有环形缓冲区
- **输出**: driver/src/zero_copy.rs
- **依赖**: 无
- **验收标准**: CPU 使用率降低 30%
- **复杂度**: ★★★★★

### 任务 28: 并发解密
- **目标**: 使用 rayon 并行处理 TLS 会话
- **输入**: MITM 上下文池
- **输出**: flowlens-tls/src/concurrent.rs
- **依赖**: rayon
- **验收标准**: 4核 CPU 提升 3x 吞吐量
- **复杂度**: ★★★★☆

### 任务 29: 内存上限控制
- **目标**: LRU 淘汰旧会话、强制写入磁盘
- **输入**: StorageEngine
- **输出**: flowlens-storage/src/lru_cache.rs
- **依赖**: 无
- **验收标准**: 内存占用稳定在 200MB 以下
- **复杂度**: ★★★☆☆

### 任务 30: 崩溃恢复机制
- **目标**: WAL 日志 + 检查点恢复
- **输入**: SQLite 配置
- **输出**: flowlens-storage/src/recovery.rs
- **依赖**: 无
- **验收标准**: 强制断电后数据无丢失
- **复杂度**: ★★★☆☆

---

## 依赖图

```
Phase 1 Tasks:
  1 → 2,3
  2 → 6,7,8
  3 → 10
  4 → 10
  5 → 10
  6,7 → 10
  8,9,10 → 完成 Phase 1

Phase 2 Tasks:
  11,12,13,14 → 15,16,17
  15,16,17 → 完成 Phase 2

Phase 3 Tasks:
  18 → 19,20
  19,20 → 完成 Phase 3

Phase 4 Tasks:
  21 → 22,23,24,25
  22,23,24,25 → 26
  26 → 完成 Phase 4 (替换 Phase 1 的 4 任务)

Phase 5 Tasks:
  27,28,29,30 → 完成 Phase 5
```

---

*Tasks last updated: 2026/10/14*