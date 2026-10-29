# FlowLens 路线图

## Phase 1: 用户态 MVP（2-3 周）

### 目标
实现基础抓包能力，使用 Npcap 作为捕获引擎，支持 HTTP/1.1 和 TLS 1.2 解密。

### 功能清单
- [x] **flowlens-cli**: 命令行入口、管理员权限检查、安装驱动
- [x] **flowlens-capture**: Npcap 用户态捕获引擎
- [x] **flowlens-attribution**: ETW + IP Helper 进程归属
- [x] **flowlens-tls**: rcgen CA + rustls MITM 解密
- [x] **flowlens-protocol**: HTTP/1.1 解析 (httparse)
- [x] **flowlens-storage**: SQLite 存储
- [x] **flowlens-ui**: ratatui TUI 界面

### 可验收标准
1. 启动 FlowLens 后能看到所有 HTTP 流量
2. HTTPS 流量被解密为明文
3. 每条流量能正确归属到 PID/进程名
4. TUI 能实时展示会话列表和请求详情
5. 按 PID/Host 过滤正常工作

### 风险与回退
- **Npcap 不可用**: 使用已安装的网络适配器信息回退
- **ETW 订阅失败**: 回退到 IP Helper TCP 表查询
- **驱动签名失败**: Phase 1 无需 WFP 驱动，完全用户态运行

---

## Phase 2: 多协议支持（2-3 周）

### 目标
扩展协议解析能力，支持 HTTP/2、WebSocket、SSE、gRPC，并增加导出功能。

### 功能清单
- [x] HTTP/2 解析 (h2 crate)
- [x] WebSocket 帧解析 (tungstenite)
- [x] SSE 事件流解析
- [x] gRPC 请求/响应聚合 (h2 + prost)
- [x] HAR 导出
- [x] SAZ 导入 (部分兼容)
- [x] 高级过滤器：时间范围、URL 包含、大小范围

### 可验收标准
1. 能正确解析 WebSocket 双向通信
2. SSE 事件能按时间线展示
3. HAR 文件可被 Chrome DevTools 导入
4. 过滤器组合工作正常

### 风险与回退
- **HTTP/2 复杂性**: 仅解析头部，body 按字节流处理
- **SAZ 兼容性**: 仅支持导入 metadata 和 basic HTTP，不保证完美兼容

---

## Phase 3: MCP 服务器（1-2 周）

### 目标
实现 MCP 协议服务器，允许 Claude Code 等 AI 代理查询流量数据。

### 功能清单
- [x] flowlens-mcp crate
- [x] `get_sessions` 工具
- [x] `get_request` 工具
- [x] `aggregate_by_pid` 工具
- [x] `aggregate_by_host` 工具
- [x] JSON-RPC 通信

### 可验收标准
1. Claude Code 可以调用 MCP 工具查询本机流量
2. 返回格式符合 MCP 规范

---

## Phase 4: WFP Callout 驱动（3-4 周）

### 目标
用 WFP Callout 驱动替换 Npcap，实现真正的内核态捕获。

### 功能清单
- [ ] C/C++ WFP Callout 驱动开发
- [ ] Rust 用户态 FFI 绑定
- [ ] IOCTL + 共享内存环形缓冲区
- [ ] 驱动签名 (EV 证书或自签名测试模式)
- [ ] 自动安装器

### 可验收标准
1. 替代 Npcap 完全捕获流量
2. 环形缓冲区零丢包处理
3. 1000+ 并发连接稳定性测试通过
4. 免安装模式（自释放驱动）

### 风险与回退
- **驱动签名**: EV 证书需要 ~$300/年；测试阶段使用自签名 + TestSigning 模式
- **内核稳定性**: 使用 `!` 宏验证所有指针；panic 不崩溃系统
- **回退机制**: WFP 加载失败时自动切换回 Npcap

---

## Phase 5: 性能优化（2-3 周）

### 目标
优化性能和内存占用，支持大规模流量捕获。

### 功能清单
- [ ] 环形缓冲区零拷贝
- [ ] 并发解密
- [ ] 内存上限控制
- [ ] 崩溃恢复机制
- [ ] 批量导出优化

### 可验收标准
1. CPU 占用 < 10%（中等负载）
2. 内存占用 < 500MB
3. 持续捕获 24 小时无内存泄漏

---

## 依赖关系图

```
Phase 1 ────────▶ Phase 2 ────────▶ Phase 3
    │                               │
    ▼                               ▼
Phase 4 ─────────────────────────▶ Phase 5
```

---

*Roadmap last updated: 2026/10/30*