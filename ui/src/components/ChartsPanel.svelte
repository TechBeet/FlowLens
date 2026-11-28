<script lang="ts">
  import type { UiEvent } from "../lib/types";

  interface Props {
    events?: UiEvent[];
  }

  let { events = [] }: Props = $props();

  type KindCounts = Record<string, number>;

  // Count events per protocol kind, derived from the live event stream.
  let protocolCounts = $derived.by<KindCounts>(() => {
    const counts: KindCounts = { http: 0, tls: 0, ws: 0, sse: 0, grpc: 0 };
    for (const ev of events) {
      if (ev.kind in counts) counts[ev.kind] = (counts[ev.kind] ?? 0) + 1;
    }
    return counts;
  });

  let totalEvents = $derived(events.length);

  const rows: Array<[string, string]> = [
    ["HTTP", "http"],
    ["HTTPS/TLS", "tls"],
    ["WebSocket", "ws"],
    ["SSE", "sse"],
    ["gRPC", "grpc"],
  ];

  function pct(n: number): number {
    return totalEvents > 0 ? Math.round((n / totalEvents) * 100) : 0;
  }
</script>

<div class="charts-panel" aria-label="流量统计图表">
  <div class="chart-title">协议分布</div>
  <div class="protocol-breakdown">
    {#each rows as [label, key]}
      <div class="protocol-row">
        <span class="protocol-name">{label}</span>
        <span class="protocol-track">
          <span class="protocol-bar" style="width:{pct(protocolCounts[key] ?? 0)}%"></span>
        </span>
        <span class="protocol-count">{protocolCounts[key] ?? 0}</span>
      </div>
    {/each}
  </div>
  <div class="chart-total">总计 {totalEvents} 事件</div>
</div>

<style>
  .charts-panel {
    padding: 12px;
    background: var(--surface);
    flex: 1;
    overflow: auto;
  }

  .chart-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg);
    margin-bottom: 12px;
  }

  .protocol-breakdown {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .protocol-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .protocol-name {
    font-size: 12px;
    color: var(--fg-dim);
    min-width: 76px;
  }

  .protocol-track {
    flex: 1;
    height: 10px;
    background: var(--hover);
    border-radius: 5px;
    overflow: hidden;
  }

  .protocol-bar {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: 5px;
    transition: width 0.3s ease;
  }

  .protocol-count {
    font-size: 12px;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
    min-width: 32px;
    text-align: right;
  }

  .chart-total {
    margin-top: 16px;
    font-size: 12px;
    color: var(--fg-dim);
  }
</style>