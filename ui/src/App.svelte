<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import * as api from "./lib/api";
  import type { TrafficEvent, TrafficStats } from "./lib/types";
  import Header from "./components/Header.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import RightPanel from "./components/RightPanel.svelte";
  import TrafficTable from "./components/TrafficTable.svelte";

  // State management
  let running = $state(false);
  let events = $state<TrafficEvent[]>([]);
  let selectedEvent = $state<TrafficEvent | null>(null);
  let stats = $state<TrafficStats>({ connections: 0, http: 0, tls: 0, sse: 0, websocket: 0, total: 0 });
  let currentTheme = $state<"dark" | "light">("dark");
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let logEntries = $state<string[]>([]);
  let unlisten: (() => void) | undefined;
  let devices = $state<string[]>([]);
  let selectedDevice = $state<string>("");
  let caStatus = $state<{ installed: boolean; cn: string | null }>({ installed: false, cn: null });
  let statsInterval: number | undefined;

  // Initialize theme
  onMount(() => {
    currentTheme = api.loadTheme();
    api.applyTheme(currentTheme);
  });

  // Load devices
  async function loadDevices() {
    try {
      const devs = await api.listDevices();
      devices = devs;
      if (devs.length > 0 && !selectedDevice) {
        selectedDevice = devs[0];
      }
    } catch (e) {
      addLog(`加载设备列表失败：${e}`);
    }
  }

  // Poll stats
  async function pollStats() {
    try {
      const s = await api.getStats();
      stats = s;
    } catch (e) {
      // Silently ignore polling errors
    }
  }

  // Check CA status
  async function checkCA() {
    try {
      const status = await api.checkCAStatus("FlowLens CA");
      caStatus = { installed: status.installed, cn: status.cn };
    } catch (e) {
      addLog(`CA 状态检查失败：${e}`);
    }
  }

  // Install CA
  async function installCA() {
    try {
      await api.installCA("FlowLens CA");
      addLog("CA 证书已安装");
      await checkCA();
    } catch (e) {
      addLog(`CA 安装失败：${e}`);
    }
  }

  // Uninstall CA
  async function uninstallCA() {
    try {
      await api.uninstallCA("FlowLens CA");
      addLog("CA 证书已卸载");
      await checkCA();
    } catch (e) {
      addLog(`CA 卸载失败：${e}`);
    }
  }

  // Export HAR
  async function exportHAR() {
    try {
      const path = window.prompt("请输入导出路径 (例如: C:\\\\output.har):");
      if (path) {
        await api.exportHAR(path);
        addLog(`已导出 HAR: ${path}`);
      }
    } catch (e) {
      addLog(`导出 HAR 失败：${e}`);
    }
  }

  // Start capture
  async function startCapture() {
    try {
      isLoading = true;
      error = null;
      await api.startCapture();
      running = true;
      addLog("开始捕获网络流量");
      // Start polling stats
      statsInterval = window.setInterval(pollStats, 1000);
      // Check CA status
      await checkCA();
    } catch (e) {
      error = `启动失败：${e}`;
      addLog(error);
    } finally {
      isLoading = false;
    }
  }

  // Stop capture
  async function stopCapture() {
    try {
      isLoading = true;
      error = null;
      await api.stopCapture();
      running = false;
      addLog("停止捕获网络流量");
      if (statsInterval) {
        clearInterval(statsInterval);
        statsInterval = undefined;
      }
    } catch (e) {
      error = `停止失败：${e}`;
      addLog(error);
    } finally {
      isLoading = false;
    }
  }

  // Toggle theme
  function toggleThemeBtn() {
    const newTheme = api.toggleTheme(currentTheme);
    currentTheme = newTheme;
    api.applyTheme(newTheme);
    api.saveTheme(newTheme);
    addLog(`主题已切换为${newTheme === "dark" ? "暗色" : "亮色"}模式`);
  }

  function addLog(msg: string) {
    const ts = new Date().toLocaleTimeString();
    logEntries = [...logEntries.slice(-80), `[${ts}] ${msg}`];
  }

  function clearEvents() {
    events = [];
    selectedEvent = null;
    addLog("已清除事件");
  }

  function selectEvent(ev: TrafficEvent) {
    selectedEvent = ev;
  }

  // Listen for traffic events from the backend
  onMount(async () => {
    unlisten = await listen<TrafficEvent>("traffic-event", (ev) => {
      const p = ev.payload;
      events = [...events.slice(-500), p];
      addLog(`${p.kind}：${p.info}`);
    });
  });

  // Sync running state via capture lifecycle events emitted by the backend.
  onMount(async () => {
    const u1 = await listen("capture-started", () => {
      running = true;
      addLog("后端已开始捕获");
    });
    const u2 = await listen("capture-stopped", () => {
      running = false;
      addLog("后端已停止捕获");
      if (statsInterval) {
        clearInterval(statsInterval);
        statsInterval = undefined;
      }
    });
    return () => {
      u1();
      u2();
    };
  });

  // Load devices on mount
  onMount(() => {
    loadDevices();
  });

  onDestroy(() => {
    unlisten?.();
    if (statsInterval) {
      clearInterval(statsInterval);
    }
  });
</script>

<div class="app-container">
  <Header
    {running}
    {stats}
    {isLoading}
    theme={currentTheme}
    onstart={startCapture}
    onstop={stopCapture}
    ontoggletheme={toggleThemeBtn}
  />

  <div class="toolbar">
    <label class="toolbar-label">
      设备：
      <select bind:value={selectedDevice} class="device-select">
        {#each devices as dev}
          <option value={dev}>{dev}</option>
        {/each}
      </select>
    </label>
    <button class="btn" onclick={exportHAR}>导出 HAR</button>
    <button class="btn" onclick={installCA}>安装 CA</button>
    <button class="btn" onclick={uninstallCA}>卸载 CA</button>
    <button class="btn" onclick={checkCA}>CA 状态</button>
    <span class="ca-status">
      CA: {caStatus.installed ? "已安装" : "未安装"}
    </span>
  </div>

  <div class="main-content">
    <Sidebar
      bind:events
      bind:selectedEvent
    />
    <TrafficTable
      bind:events
      bind:selectedEvent
    />
    <RightPanel event={selectedEvent} />
  </div>

  <footer class="status-bar">
    <span class="status-item">
      事件：<strong>{events.length}</strong> 条
    </span>
    <span class="status-sep">│</span>
    <span class="status-item status-{running ? "running" : "stopped"}">
      捕获：<strong>{running ? "进行中" : "已停止"}</strong>
    </span>
    <span class="status-sep">│</span>
    <span class="status-item status-hint">
      快捷键：F2 切换主题
    </span>
    {#if error}
      <span class="status-error">{error}</span>
    {/if}
  </footer>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: var(--bg);
    color: var(--fg);
    font-family: "Segoe UI", system-ui, sans-serif;
    overflow: hidden;
  }

  .app-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
  }

  .main-content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

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

  .ca-status {
    color: var(--fg-dim);
    margin-left: auto;
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    background: var(--surface);
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-dim);
    min-height: 26px;
  }

  .status-item strong {
    color: var(--fg);
    font-weight: 600;
  }

  .status-sep {
    color: var(--border);
  }

  .status-running { color: var(--ok); }
  .status-stopped { color: var(--fg-dim); }
  .status-hint { opacity: 0.7; }

  .status-error {
    margin-left: auto;
    color: var(--err);
    font-weight: 600;
  }
</style>
