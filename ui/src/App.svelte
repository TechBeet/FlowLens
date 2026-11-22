<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import * as api from "./lib/api";
  import type { TrafficEvent, TrafficStats } from "./lib/types";
  import { shortcutRegistry } from "./lib/shortcuts";
  import Header from "./components/Header.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import DetailPanel from "./components/DetailPanel.svelte";
  import ChartsPanel from "./components/ChartsPanel.svelte";
  import ReplayHistory from "./components/ReplayHistory.svelte";
  import TrafficTable from "./components/TrafficTable.svelte";
  import SniDomainSelector from "./components/SniDomainSelector.svelte";
  import FilterBar from "./components/FilterBar.svelte";
import SettingsView from "./components/SettingsView.svelte";
import { createEventStore } from "./lib/stores/eventStore.svelte";
import { filterEvents } from "./lib/composables";

  export interface ReplayEntry {
    id: number;
    sessionId: number;
    delayMs: number;
    headers: Record<string, string>;
    status: "success" | "error";
    result?: unknown;
    error?: string;
    time: string;
    label?: string;
  }

  // State management
  let running = $state(false);
  // Handle to the FilterBar component, so shortcuts can call its `focus()` API
  // instead of guessing at DOM selectors.
  let filterBar = $state<{ focus: () => void } | undefined>(undefined);
  let events = $state<TrafficEvent[]>([]);
  let replayHistory = $state<ReplayEntry[]>([]);
  let selectedEvent = $state<TrafficEvent | null>(null);
  let stats = $state<TrafficStats>({ connections: 0, http: 0, tls: 0, sse: 0, websocket: 0, total: 0 });
  let sharedStore = createEventStore();
  let filteredForTable = $derived(filterEvents(events, sharedStore.filterState));

  async function handleReplay(sessionId: number, delayMs: number, headers: Record<string, string>) {
    try {
      const result = await api.replayRequestWithDelay(sessionId, delayMs, headers);
      replayHistory = [
        {
          id: Date.now(),
          sessionId,
          delayMs,
          headers,
          status: "success",
          result,
          time: new Date().toLocaleTimeString(),
          label: `成功重放 #${sessionId}`,
        },
        ...replayHistory.slice(0, 49),
      ];
      addLog(`重放会话 #${sessionId} 成功`);
    } catch (e) {
      replayHistory = [
        {
          id: Date.now(),
          sessionId,
          delayMs,
          headers,
          status: "error",
          error: String(e),
          time: new Date().toLocaleTimeString(),
          label: `失败重放 #${sessionId}`,
        },
        ...replayHistory.slice(0, 49),
      ];
      addLog(`重放会话 #${sessionId} 失败：${e}`);
    }
  }

  async function handleBatchReplay(sessionIds: number[], delayMs: number) {
    for (const sid of sessionIds) {
      // Use empty headers for batch replay as per current implementation constraints
      await handleReplay(sid, delayMs, {});
    }
  }

  // ---- Keyboard shortcuts (resolved via shortcutRegistry) ----

  function handleKeyDown(event: KeyboardEvent): void {
    // Never intercept when the user is typing into an editable input/select.
    const target = event.target as HTMLElement | null;
    const editable =
      target?.matches("input, textarea, select") ||
      target?.isContentEditable;
    if (editable) return;

    const actionId = shortcutRegistry.match(event);
    if (!actionId) return;

    event.preventDefault();

    switch (actionId) {
      case "focus_filter":
        // Delegate to the FilterBar component API. Optional chaining keeps the
        // shortcut a no-op (never a crash) when the bar is not mounted.
        filterBar?.focus();
        break;
      case "toggle_capture":
        running ? stopCapture() : startCapture();
        break;
      case "export_har":
        exportHAR();
        break;
      case "clear_selection":
        selectedEvent = null;
        addLog("快捷键：已清除选择");
        break;
      case "toggle_theme":
        toggleThemeBtn();
        break;
      // Unknown action ids are intentionally ignored; a shortcut always maps
      // to a registered action.
    }
  }

  function clearReplayHistory() {
    replayHistory = [];
    addLog("已清空重放历史");
  }

  let currentTheme = $state<"dark" | "light">("dark");
  let showSettings = $state(false);
  let showCharts = $state(false);
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let logEntries = $state<string[]>([]);
  let unlisten: (() => void) | undefined;
  let devices = $state<string[]>([]);
  let selectedDevice = $state<string>("");
  let captureMode = $state<"pcap" | "windivert">("pcap");
  let caStatus = $state<{ installed: boolean; cn: string | null }>({ installed: false, cn: null });
  let statsInterval: number | undefined;

  // Initialize theme
  onMount(() => {
    api.loadTheme().then((t) => {
      currentTheme = t;
      api.applyTheme(t);
    });
  });

  // Load persisted shortcut overrides and attach the global key handler.
  // The handler resolves combos through `shortcutRegistry`, so overrides take
  // effect as soon as they are merged (no re-binding needed).
  onMount(() => {
    api.loadShortcuts().then((saved) => {
      shortcutRegistry.merge(saved);
    });
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
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
      addLog(`加载设备列表失败：{e}`);
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
      const path = window.prompt("请输入导出文件路径 (例如: trace.har):", "trace.har");
      if (!path) return;
      await api.exportHAR(path, {
        include_headers: true,
        include_bodies: true,
        include_timing: true,
      });
      addLog(`已导出 HAR: ${path}`);
    } catch (e) {
      addLog(`导出 HAR 失败：${e}`);
    }
  }

  // Start capture
  async function startCapture() {
    try {
      isLoading = true;
      error = null;
      await api.startCapture(captureMode, selectedDevice);
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
    try {
      unlisten = await listen<TrafficEvent>("traffic-event", (ev) => {
        const p = ev.payload;
        events = [...events.slice(-500), p];
        addLog(`${p.kind} - ${p.info}`);
      });
    } catch {
      // Backend unavailable (e.g. browser dev run without Tauri); no events.
    }
  });

  // Sync running state via capture lifecycle events emitted by the backend.
  onMount(async () => {
    let u1: (() => void) | undefined;
    let u2: (() => void) | undefined;
    try {
      u1 = await listen("capture-started", () => {
        running = true;
        addLog("后端已开始捕获网络");
      });
      u2 = await listen("capture-stopped", () => {
        running = false;
        addLog("后端已停止捕获网络");
        if (statsInterval) {
          clearInterval(statsInterval);
          statsInterval = undefined;
        }
      });
    } catch {
      // Backend unavailable (e.g. browser dev run without Tauri).
    }
    return () => {
      u1?.();
      u2?.();
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
    <div class="domain-controls">
      <SniDomainSelector />
    </div>
    <label class="toolbar-label">
      设备�?
      <select bind:value={selectedDevice} class="device-select">
        {#each devices as dev}
          <option value={dev}>{dev}</option>
        {/each}
      </select>
    </label>
    <label class="toolbar-label" title="WinDivert 模式无需代理即可解密 HTTPS">
      模式：
      <select bind:value={captureMode} class="device-select">
        <option value="pcap">Npcap（需安装）</option>
        <option value="windivert">WinDivert（无需代理）</option>
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

  <!-- The single FilterBar instance: Ctrl+F targets its exported focus() API. -->
  <FilterBar store={sharedStore} bind:this={filterBar} />

  <div class="main-content">
    <Sidebar
      bind:events
      bind:selectedEvent
    />
    <TrafficTable
      events={filteredForTable}
      bind:selectedEvent
      onreplay={(sid) => handleReplay(sid, 0, {})}
      onbatchreplay={handleBatchReplay}
      onStartCapture={startCapture}
    />
    <div class="right-column">
      <div class="right-tabs">
        <button class="right-tab" class:active={!showCharts} onclick={() => (showCharts = false)}>详情</button>
        <button class="right-tab" class:active={showCharts} onclick={() => (showCharts = true)}>图表</button>
      </div>
      {#if showCharts}
        <ChartsPanel events={events} />
      {:else}
        <DetailPanel event={selectedEvent} onreplay={handleReplay} />
        <ReplayHistory entries={replayHistory} onclear={clearReplayHistory} />
      {/if}
    </div>
  </div>

  <SettingsView
    open={showSettings}
    theme={currentTheme}
    onclose={() => (showSettings = false)}
    ontoggletheme={toggleThemeBtn}
  />

  <footer class="status-bar">
    <span class="status-item">
      事件 <strong>{events.length}</strong> 
    </span>
    <span class="status-sep">|</span>
    <span class="status-item status-{running ? "running" : "stopped"}">
      捕获 <strong>{running ? "进行中" : "已停止"}</strong>
    </span>
    <span class="status-sep">|</span>
    <span class="status-item status-hint">
      快捷键：可在「设置 → 快捷键」中自定义
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

  .btn:hover {
    background: var(--hover);
  }

  .status-error {
  margin-left: auto;
  color: var(--err);
  font-weight: 600;
}
  .right-tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .right-tab {
    flex: 1;
    padding: 8px;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    font-size: 12px;
    cursor: pointer;
  }
  .right-tab.active {
    color: var(--accent);
    border-bottom: 2px solid var(--accent);
  }
  .right-tab:hover {
    background: var(--hover);
  }
</style>
