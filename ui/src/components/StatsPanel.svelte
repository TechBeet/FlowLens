<script lang="ts">
  import type { TrafficStats } from "../lib/types";
  import type { EventStore } from "../lib/stores/eventStore";
  import { createEventStore } from "../lib/stores/eventStore";

  interface Props {
    store?: EventStore;
    stats?: TrafficStats;
  }

  let { store = createEventStore(), stats }: Props = $props();
</script>

<div class="stats-panel" aria-label="流量统计">
  <div class="stat">
    <span class="stat-label">总计</span>
    <span class="stat-value">{stats?.total ?? store.stats.total}</span>
  </div>
  <div class="stat">
    <span class="stat-label">HTTP</span>
    <span class="stat-value">{stats?.http ?? store.stats.http}</span>
  </div>
  <div class="stat">
    <span class="stat-label">TLS</span>
    <span class="stat-value">{stats?.tls ?? store.stats.tls}</span>
  </div>
  <div class="stat">
    <span class="stat-label">SSE</span>
    <span class="stat-value">{stats?.sse ?? store.stats.sse}</span>
  </div>
  <div class="stat">
    <span class="stat-label">WS</span>
    <span class="stat-value">{stats?.websocket ?? store.stats.websocket}</span>
  </div>
  <div class="stat stat-conn">
    <span class="stat-label">连接</span>
    <span class="stat-value">{stats?.connections ?? store.stats.connections}</span>
  </div>
</div>

<style>
  .stats-panel {
    display: flex;
    gap: 14px;
    padding: 6px 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .stat {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--fg-dim);
  }

  .stat.conn {
    background: var(--panel);
  }

  .stat-label {
    font-size: 11px;
    color: var(--fg-dim);
  }

  .stat-value {
    color: var(--fg);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
</style>
