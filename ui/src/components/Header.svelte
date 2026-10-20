<script lang="ts">
  export let running = false;
  export let stats: any = { connections: 0, http: 0, tls: 0, sse: 0, websocket: 0, total: 0 };
  
  export function startCapture() {
    // This is handled by the parent component
  }
  
  export function stopCapture() {
    // This is handled by the parent component
  }
</script>

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

<div class="toolbar">
  <div class="left-group">
    <button 
      class="btn" 
      class:active={running}
      on:click={() => running ? stopCapture() : startCapture()}
    >
      {running ? '⏚ 停止捕获' : '● 开始捕获'}
    </button>
    <button class="btn">清除</button>
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

<style>
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: #12121e;
    border-bottom: 1px solid #222;
  }

  .logo {
    font-size: 18px;
    font-weight: 600;
    color: #7c5af3;
  }

  .window-controls {
    display: flex;
    gap: 8px;
  }

  .control-btn {
    width: 28px;
    height: 28px;
    background: transparent;
    border: none;
    color: #888;
    cursor: pointer;
    border-radius: 4px;
    transition: all 0.15s;
  }

  .control-btn:hover {
    background: #1e1e2e;
    color: #eee;
  }

  .control-btn.close:hover {
    background: #e01e36;
    color: white;
  }

  .symbol {
    font-size: 14px;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    background: #151520;
    border-bottom: 1px solid #222;
  }

  .left-group, .right-group {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .btn {
    padding: 6px 12px;
    background: #1e1e2e;
    border: 1px solid #333;
    border-radius: 4px;
    color: #bbb;
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
</style>