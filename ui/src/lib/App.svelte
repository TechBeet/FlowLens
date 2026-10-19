<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  export let running = $state(false);
  export let events = $state<any[]>([]);
  export let selectedEvent = $state<any>(null);
  export let stats = $state({ connections: 0, http: 0, tls: 0, sse: 0, websocket: 0, total: 0 });
  export let logEntries = $state<string[]>([]);
  export let currentEvent: any = $state(null);

  async function startCapture() {
    try {
      await invoke("start_capture");
      running = true;
      addLog("开始捕获");
    } catch (e) {
      addLog(`启动失败: ${e}`);
    }
  }

  async function stopCapture() {
    try {
      await invoke("stop_capture");
      running = false;
      addLog("停止捕获");
    } catch (e) {
      addLog(`停止失败: ${e}`);
    }
  }

  function addLog(msg: string) {
    logEntries = [...logEntries.slice(-100), `[${new Date().toLocaleTimeString()}] ${msg}`];
  }

  function clearEvents() {
    events = [];
    selectedEvent = null;
    addLog("已清除");
  }

  onMount(async () => {
    const unlisten = await listen("traffic-event", (event: any) => {
      events = [...events.slice(-500), event.payload];
      stats = event.payload.stats || stats;
      addLog(`${event.payload.kind}: ${event.payload.info}`);
    });
    return () => unlisten();
  });
</script>

<main class="container">
  <!-- Header -->
  <header class="header">
    <div class="logo">FlowLens</div>
    <div class="window-controls">
      <button class="control-btn" title="最小化">
        <span class="symbol">_</span>
      </button>
      <button class="control-btn" title="最大化">
        <span class="symbol">□</span>
      </button>
      <button class="control-btn close" title="关闭">
        <span class="symbol">✕</span>
      </button>
    </div>
  </header>

  <!-- Toolbar -->
  <div class="toolbar">
    <div class="left-group">
      <button
        class="btn"
        class:active={running}
        onclick={running ? stopCapture : startCapture}
      >
        {running ? "⏚ 停止捕获" : "● 开始捕获"}
      </button>
      <button class="btn" onclick={clearEvents}>清除</button>
      <button class="btn">导出</button>
      <button class="btn">设置</button>
    </div>
    <div class="right-group">
      <span class="stats-badge">连接: {stats.connections}</span>
      <span class="stats-badge">HTTP: {stats.http}</span>
      <span class="stats-badge">TLS: {stats.tls}</span>
      <span class="stats-badge">SSE: {stats.sse}</span>
      <span class="stats-badge">WS: {stats.websocket}</span>
      <span class="stats-badge">全部: {stats.total}</span>
    </div>
  </div>

  <!-- Main Content -->
  <div class="main-content">
    <!-- Left: Event List -->
    <div class="sidebar">
      <div class="search-bar">
        <input type="text" placeholder="搜索事件..." />
      </div>
      <div class="event-list">
        {#each events as event (event.id)}
          <div
            class="event-item"
            class:selected={selectedEvent?.id === event.id}
            onclick={() => {
              selectedEvent = event;
              currentEvent = event;
            }}
          >
            <span class="event-method">{event.method || "—"}</span>
            <span class="event-url">{event.url || event.info || ""}</span>
            <span class="event-status">{event.status || ""}</span>
          </div>
        {/each}
        {#if events.length === 0}
          <div class="empty-state">暂无事件</div>
        {/if}
      </div>
    </div>

    <!-- Right: Detail + Log -->
    <div class="detail-pane">
      <div class="detail-tabs">
        <button class="tab-btn active">详情</button>
        <button class="tab-btn">请求</button>
        <button class="tab-btn">响应</button>
        <button class="tab-btn">日志</button>
      </div>
      <div class="detail-content">
        {#if currentEvent}
          <pre class="event-json">{JSON.stringify(currentEvent, null, 2)}</pre>
        {:else}
          <div class="empty-detail">请选择一个事件查看详情</div>
        {/if}
      </div>
    </div>

    <!-- 日志面板 -->
    <div class="log-pane">
      <div class="log-header">日志</div>
      <div class="log-list">
        {#each logEntries as line (line)}
          <div class="log-line">{line}</div>
        {/each}
      </div>
    </div>
  </div>
</main>

<style>
  :global(html, body, #svelte) {
    margin: 0;
    padding: 0;
    height: 100%;
    font-family: "Segoe UI", "Inter", system-ui, sans-serif;
    background: #0f0f1a;
    color: #e0e0e0;
  }

  .container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #0f0f1a;
    color: #e0e0e0;
  }

  /* Header */
  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 12px;
    height: 36px;
    background: #151520;
    border-bottom: 1px solid #222;
    -webkit-app-region: drag;
  }

  .logo {
    font-size: 15px;
    font-weight: 600;
    color: #7c5af3;
  }

  .window-controls {
    display: flex;
    gap: 4px;
    -webkit-app-region: no-drag;
  }

  .control-btn {
    width: 32px;
    height: 24px;
    border: none;
    background: transparent;
    color: #aaa;
    font-size: 11px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s;
    cursor: pointer;
  }

  .control-btn:hover {
    background: #2a2a3a;
    color: #eee;
  }

  .control-btn.close:hover {
    background: #e74c3c;
    color: #fff;
  }

  /* Toolbar */
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    background: #151520;
    border-bottom: 1px solid #222;
    -webkit-app-region: drag;
  }

  .left-group,
  .right-group {
    display: flex;
    gap: 8px;
    align-items: center;
    -webkit-app-region: no-drag;
  }

  .btn {
    padding: 6px 14px;
    border: 1px solid #333;
    background: #1e1e2e;
    color: #ddd;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn:hover {
    background: #2a2a3e;
    border-color: #555;
  }

  .btn.active {
    background: #7c5af3;
    border-color: #7c5af3;
    color: #fff;
  }

  .stats-badge {
    font-size: 12px;
    padding: 2px 8px;
    background: #1e1e2e;
    border-radius: 10px;
    color: #bbb;
  }

  /* Main Content */
  .main-content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .sidebar {
    width: 320px;
    min-width: 200px;
    display: flex;
    flex-direction: column;
    background: #12121e;
    border-right: 1px solid #222;
    overflow: hidden;
  }

  .search-bar {
    padding: 10px;
    border-bottom: 1px solid #222;
  }

  .search-bar input {
    width: 100%;
    padding: 6px 10px;
    background: #1a1a2a;
    border: 1px solid #333;
    border-radius: 4px;
    font-size: 13px;
    color: #e0e0e0;
  }

  .event-list {
    flex: 1;
    overflow: auto;
  }

  .event-item {
    padding: 8px 12px;
    border-bottom: 1px solid #1a1a2a;
    display: flex;
    flex-direction: column;
    cursor: pointer;
    transition: background 0.15s;
    font-size: 13px;
  }

  .event-item:hover {
    background: #1a1a2e;
  }

  .event-item.selected {
    background: #201f33;
  }

  .event-method {
    font-weight: 600;
    color: #7c5af3;
  }

  .event-url {
    color: #aaa;
    font-size: 12px;
    margin-top: 2px;
  }

  .event-status {
    font-size: 11px;
    color: #666;
    margin-top: 2px;
    align-self: flex-end;
  }

  .empty-state {
    padding: 20px;
    text-align: center;
    color: #555;
    font-size: 13px;
  }

  .detail-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .detail-tabs {
    display: flex;
    background: #151520;
    border-bottom: 1px solid #222;
  }

  .tab-btn {
    padding: 10px 16px;
    background: transparent;
    border: none;
    color: #888;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .tab-btn.active,
  .tab-btn:hover {
    color: #eee;
    background: #1e1e2e;
  }

  .tab-btn.active {
    color: #7c5af3;
    border-bottom: 2px solid #7c5af3;
  }

  .detail-content {
    flex: 1;
    padding: 12px;
    overflow: auto;
  }

  .event-json {
    font-size: 12px;
    line-height: 1.5;
    color: #bbb;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .empty-detail {
    color: #555;
    text-align: center;
    padding: 40px;
    font-size: 14px;
  }

  .log-pane {
    height: 120px;
    background: #0d0d15;
    border-top: 1px solid #222;
    display: flex;
    flex-direction: column;
  }

  .log-header {
    padding: 6px 12px;
    font-size: 12px;
    color: #666;
    border-bottom: 1px solid #222;
  }

  .log-list {
    flex: 1;
    overflow: auto;
    font-size: 11px;
    padding: 4px 12px;
  }

  .log-line {
    padding: 2px 0;
    color: #66aaff;
  }
</style>
