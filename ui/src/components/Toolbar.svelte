<script lang="ts">
  import type { UiEvent } from "../lib/types";
  import type { EventStore } from "../lib/stores/eventStore";
  import * as api from "../lib/api";

  interface Props {
    running?: boolean;
    isLoading?: boolean;
    theme?: "dark" | "light";
    devices?: string[];
    selectedDevice?: string;
    store?: EventStore;
    onstart?: () => void;
    onstop?: () => void;
    onclear?: () => void;
    ontoggletheme?: () => void;
    ondevicestart?: (device: string) => void;
  }

  let {
    running = false,
    isLoading = false,
    theme = "dark",
    devices = [],
    selectedDevice = $bindable(""),
    store,
    onstart,
    onstop,
    onclear,
    ontoggletheme,
    ondevicestart,
  }: Props = $props();

  function handleCapture() {
    if (running) {
      onstop?.();
    } else {
      onstart?.();
    }
  }

  function addLog(msg: string) {
    console.log("[CA]", msg);
  }
</script>

<div class="toolbar" role="toolbar" aria-label="捕获工具栏">
  <label class="toolbar-label">
    设备：
    <select bind:value={selectedDevice} class="device-select">
      {#each devices as dev}
        <option value={dev}>{dev}</option>
      {/each}
    </select>
  </label>

  <button
    class="btn primary"
    class:danger={running}
    disabled={isLoading}
    onclick={handleCapture}
  >
    {running ? "■ 停止捕获" : "● 开始捕获"}
  </button>

  <button class="btn" disabled={isLoading} onclick={() => onclear?.()}>
    清除
  </button>

  <button class="btn icon-btn" onclick={() => ontoggletheme?.()} title="切换主题">
    {theme === "dark" ? "☀" : "☾"}
  </button>

  <div class="ca-controls">
    <button
      class="btn btn-secondary"
      onclick={async () => {
        try {
          const pem = await api.generateCA("FlowLens CA");
          await api.installCA("FlowLens CA");
          addLog(`已安装 CA: ${pem}`);
        } catch (e) {
          addLog(`CA 安装失败：${e}`);
        }
      }}>生成根 CA（HTTPS 解密）</button>
    <button class="btn btn-danger" onclick={() => api.uninstallCA("FlowLens CA")} title="卸载根 CA">卸载根 CA</button>
    <button
      class="btn btn-primary"
      onclick={async () => {
        const status = await api.checkCAStatus("FlowLens CA");
        addLog(`CA 状态: installed=${status.installed}, cn=${status.cn}`);
      }}>检查 CA 状态</button>
  </div>

</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .toolbar-label {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg);
  }

  .device-select {
    padding: 4px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 12px;
  }

  .btn {
    padding: 4px 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 12px;
    cursor: pointer;
  }

  .btn:hover {
    background: var(--hover);
  }

  .btn.primary {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: var(--accent);
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.12);
  }

  .btn.primary.danger {
    background: var(--err);
    border-color: var(--err);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .icon-btn {
    width: 32px;
    height: 32px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
  }

  .mock-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg);
    cursor: pointer;
  }

  .mock-toggle input {
    cursor: pointer;
  }

  .ca-controls {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    flex-wrap: wrap;
  }

  .ca-status {
    color: var(--fg-dim);
    margin-left: auto;
  }
</style>
