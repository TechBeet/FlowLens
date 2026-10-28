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

  // Initialize theme
  onMount(() => {
    currentTheme = api.loadTheme();
    api.applyTheme(currentTheme);
  });

  // Start capture
  async function startCapture() {
    try {
      isLoading = true;
      error = null;
      await api.startCapture();
      running = true;
      addLog("开始捕获网络流量");
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

  onDestroy(() => {
    unlisten?.();
  });
</script>

<div class="app-container">
  <Header
    {running}
    {stats}
    {isLoading}
    onstart={startCapture}
    onstop={stopCapture}
    ontoggle-theme={toggleThemeBtn}
    theme={currentTheme}
  />

  <div class="main-content">
    <Sidebar
      {events}
      {selectedEvent}
      onselect={selectEvent}
      onclear={clearEvents}
      {stats}
    />
    <TrafficTable
      {events}
      {selectedEvent}
      onselect={selectEvent}
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
