# FlowLens UI 规格说明

## 设备列表错误横幅 (Q4)

### 行为描述

- **触发条件**：`api.listDevices()` 抛出异常时。
- **处理逻辑**：
  1. `loadDevices()` 捕获异常，设置 `error = \`加载设备列表失败：${e}\``。
  2. 调用 `addLog(error)` 将错误写入日志面板。
  3. `devices` 保持空数组（回退），不抛出异常。
- **恢复逻辑**：成功加载时设置 `error = null`，清空错误状态。
- **UI 呈现**：
  - 工具栏下方：`{#if error}<div style="color: var(--err); margin-top: 8px;">{error}</div>{/if}`
  - 底部状态栏：`{#if error}<span class="status-error">{error}</span>{/if}`
- **可见性**：仅在 `error` 非空时显示，成功时自动隐藏。

### 其他规格

- 设备选择：默认选中第一个设备。
- 捕获控制：启动/停止时重置 `error = null`。
- 主题：深/浅色切换并持久化到后端。