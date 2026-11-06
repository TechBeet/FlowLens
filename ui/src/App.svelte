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
      addLog(`加载设备列表失败�?{e}`);
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
      const path = window.prompt("请输入导出路�?(例如: C:\\\\output.har):");
      if (path) {
        await api.exportHAR(path);
        addLog(`已导�?HAR: ${path}`);
      }
    } catch (e) {
      addLog(`导出 HAR 失败�?{e}`);
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
      error = `停止失败�?{e}`;
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
      addLog(`${p.kind} - ${p.info}`);
    });
  });

  // Sync running state via capture lifecycle events emitted by the backend.
  onMount(async () => {
    const u1 = await listen("capture-started", () => {
      running = true;
      addLog("后端已开始捕获网络");
    });
    const u2 = await listen("capture-stopped", () => {
      running = false;
      addLog("后端已停止捕获网络");
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
      设备�?
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
    <button class="btn" onclick={() => (showSettings = true)}>⚙ 设置</button>
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

  <SettingsView
    open={showSettings}
    theme={currentTheme}
    onclose={() => (showSettings = false)}
    ontoggletheme={toggleThemeBtn}
  />

  <footer class="status-bar">
    <span class="status-item">
      事件 <strong>{events.length}</strong> ?
    </span>
    <span class="status-sep">�?/span>
    <span class="status-item status-{running ? "running" : "stopped"}">
      捕获�?strong>{running ? "进行�? : "已停�?}</strong>
    </span>
    <span class="status-sep">�?/span>
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

  PLACEHOLDER
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(2px);
  }

  .modal-content {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    width: 90%;
    max-width: 600px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 18px;
    color: var(--fg);
  }

  .close-btn {
    width: 28px;
    height: 28px;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    font-size: 18px;
    cursor: pointer;
    border-radius: 4px;
  }

  .close-btn:hover {
    background: var(--hover);
    color: var(--fg);
  }

  .modal-tabs {
    display: flex;
    padding: 0 20px;
    border-bottom: 1px solid var(--border);
  }

  .tab-btn {
    padding: 10px 16px;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    font-size: 13px;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.15s;
  }

  .tab-btn:hover {
    color: var(--fg);
  }

  .tab-btn.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .settings-group {
    margin-bottom: 24px;
  }

  .settings-group h3 {
    margin: 0 0 12px 0;
    font-size: 14px;
    color: var(--fg);
    border-bottom: 1px solid var(--border);
    padding-bottom: 6px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
    gap: 12px;
  }

  .setting-row label {
    color: var(--fg);
    font-size: 13px;
  }

  .setting-select,
  .setting-input {
    padding: 6px 10px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 13px;
    min-width: 120px;
  }

  .shortcuts-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .shortcut-key {
    font-family: "Consolas", "Monaco", monospace;
    font-size: 12px;
    color: var(--accent);
    background: var(--bg);
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
  }

  .shortcut-desc {
    color: var(--fg-dim);
    font-size: 13px;
  }

  .about-section {
    text-align: center;
    padding: 20px;
  }

  .about-section h3 {
    margin: 0 0 8px 0;
    font-size: 20px;
    color: var(--accent);
  }

  .about-section p {
    margin: 4px 0;
    color: var(--fg-dim);
    font-size: 13px;
  }

  .license {
    margin-top: 12px !important;
    font-size: 11px !important;
    color: var(--fg-dim) !important;
  }

  .modal-footer {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding: 16px 20px;
    border-top: 1px solid var(--border);
  }

  .btn {
    padding: 6px 16px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 13px;
    cursor: pointer;
  }

  .btn:hover {
    background: var(--hover);
  }

  .btn-primary {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: var(--accent);
  }

  .btn-primary:hover {
    filter: brightness(1.1);
  }

  .status-error {
  margin-left: auto;
  color: var(--err);
  font-weight: 600;
}
</style>
