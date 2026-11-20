# 检查清单

## 任务 1: 无需代理的 HTTPS 解密
- [x] WinDivert 网络层捕获引擎已实现（`crates/flowlens-capture/src/windivert.rs`）
- [x] `WinDivertEngine` 支持 `start/stop/recv_packet/recv_batch/send_packet`
- [x] `extract_connection_info_from` 正确解析 IPv4/TCP 头部
- [x] `spawn_capture` 使用 `recv_wait_ex` 批量接收（64 包/系统调用）
- [x] `create_https_engine` / `create_all_ip_engine` 工厂函数可用
- [x] 通过 `--features windivert` 可选启用，默认关闭避免 CRT 冲突
- [x] `cargo test -p flowlens-capture --features windivert` 22 个测试通过

## 任务 2: MIT 许可证
- [x] `LICENSE` 文件存在，内容为 MIT 协议
- [x] `README.md` 包含许可证声明

## 任务 3: UI/UX 增强
- [x] `CaptureMode` 枚举（`Pcap` / `WinDivert`）已添加到 `commands.rs`
- [x] `start_capture` 命令支持 `mode` 参数
- [x] `ui/src-tauri/Cargo.toml` 新增 `windivert` feature
- [x] `ui/src/App.svelte` 工具栏新增"模式"下拉框
- [x] `ui/src/lib/api.ts` `startCapture(mode?, device?)` API 签名更新

## 任务 4: 性能优化
- [x] `spawn_capture` 改用 `recv_wait_ex` 批量接收
- [x] 事件存储已有批处理（50ms）和背压（MAX_PENDING=5000）机制

## 任务 5: 文档
- [x] `docs/USER_GUIDE.md` 更新环境要求，添加 WinDivert 说明
- [x] `README.md` 包含完整项目文档

## 构建验证
- [x] `cargo check --workspace` 无警告
- [x] `cargo check --workspace --features windivert` 无警告
- [x] `cargo test -p flowlens-capture` 14 个测试通过
- [x] `cargo test -p flowlens-capture --features windivert` 22 个测试通过
- [x] `cargo fmt --all --check` 格式检查通过