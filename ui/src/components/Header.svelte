<script lang="ts">
  import type { TrafficStats } from "../lib/types";

  interface Props {
    running?: boolean;
    stats?: TrafficStats;
    isLoading?: boolean;
    theme?: "dark" | "light";
    onstart?: () => void;
    onstop?: () => void;
    ontoggletheme?: () => void;
  }

  let {
    running = false,
    stats = { connections: 0, http: 0, tls: 0, sse: 0, websocket: 0, total: 0 },
    isLoading = false,
    theme = "dark",
    onstart,
    onstop,
    ontoggletheme,
  }: Props = $props();

  function handleCapture() {
    if (running) {
      onstop?.();
    } else {
      onstart?.();
    }
  }
</script>

<header class="header">
  <div class="brand">
    <span class="logo-icon">&#9670;</span>
    <span class="logo-text">流光 FlowLens</span>
  </div>

  <div class="capture-indicator" class:active={running}>
    <span class="status-dot"></span>
    <span class="status-label">{running ? "捕获中" : "未捕获"}</span>
  </div>

  <div class="actions">
    <button
      class="btn primary"
      class:danger={running}
      disabled={isLoading}
      onclick={handleCapture}
    >
      {running ? "■ 停止捕获" : "● 开始捕获"}
    </button>

    <button
      class="icon-btn"
      onclick={() => ontoggletheme?.()}
      title="切换主题 (F2)"
      aria-label="切换主题"
    >
      {theme === "dark" ? "☀" : "☾"}
    </button>
  </div>
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    font-weight: 600;
    color: var(--fg);
    white-space: nowrap;
  }

  .logo-icon {
    color: var(--accent);
    font-size: 18px;
  }

  .logo-text {
    letter-spacing: 0.4px;
  }

  .capture-indicator {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    font-size: 12px;
    color: var(--fg-dim);
  }

  .capture-indicator.active {
    border-color: var(--ok);
    color: var(--ok);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--fg-dim);
  }

  .capture-indicator.active .status-dot {
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 16px;
    font-size: 13px;
    font-weight: 500;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--fg);
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
    white-space: nowrap;
  }

  .btn:hover:not(:disabled) {
    background: var(--hover);
    border-color: var(--accent);
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
    line-height: 1;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--fg);
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .icon-btn:hover {
    background: var(--hover);
    border-color: var(--accent);
  }
</style>