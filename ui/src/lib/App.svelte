<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Header from "../components/Header.svelte";
  import Sidebar from "../components/Sidebar.svelte";
  import RightPanel from "../components/RightPanel.svelte";

  let running = $state(false);
  let events = $state<any[]>([]);
  let selectedEvent = $state<any>(null);
  let stats = $state({ connections: 0, http: 0, tls: 0, sse: 0, websocket: 0, total: 0 });
  let logEntries = $state<string[]>([]);

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

  function selectEvent(event: any) {
    selectedEvent = event;
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
  <Header {running} {stats} on:start={startCapture} on:stop={stopCapture} />

  <div class="main-content">
    <Sidebar {events} {selectedEvent} on:select={selectEvent} on:clear={clearEvents} />
    <RightPanel event={selectedEvent} />
  </div>

  <div class="log-pane">
    <div class="log-header">日志</div>
    <div class="log-list">
      {#each logEntries as entry}
        <div class="log-line">{entry}</div>
      {/each}
    </div>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: #0d0d15;
    color: #e0e0e0;
    font-family: "Segoe UI", Arial, sans-serif;
  }

  :global(html, body, #root) {
    height: 100%;
  }

  .container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #0d0d15;
  }

  .main-content {
    display: flex;
    flex: 1;
    overflow: hidden;
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