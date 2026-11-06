<script lang="ts">
  import type { UiEvent } from "../lib/types";

  interface Props {
    event?: UiEvent | null;
  }

  let { event = null }: Props = $props();

  let activeTab = $state(0);
  const tabs = ["Headers", "Request Body", "Response Body", "Timing", "TLS", "Process"];

  function formatHeaders(headers: Record<string, string> | undefined): string {
    if (!headers || Object.keys(headers).length === 0) {
      return "(无头信息)";
    }
    return Object.entries(headers)
      .map(([k, v]) => `${k}: ${v}`)
      .join("\n");
  }

  function formatBody(body: string | undefined): string {
    if (!body) return "(无内容)";
    try {
      return JSON.stringify(JSON.parse(body), null, 2);
    } catch {
      return body;
    }
  }

  function formatTiming(ev: UiEvent): string {
    const lines: string[] = [];
    if (ev.timestamp_micros != null) lines.push(`timestamp_micros: ${ev.timestamp_micros}`);
    if (ev.time) lines.push(`time: ${ev.time}`);
    if (ev.size != null) lines.push(`size: ${ev.size} bytes`);
    if (ev.session_id != null) lines.push(`session_id: ${ev.session_id}`);
    if (ev.paused != null) lines.push(`paused: ${ev.paused}`);
    if (ev.rewrite_applied?.length) lines.push(`rewrite_applied: ${ev.rewrite_applied.join(", ")}`);
    return lines.length ? lines.join("\n") : "(无 timing 信息)";
  }

  function formatTLS(ev: UiEvent): string {
    const lines: string[] = [];
    if (ev.tls_version) lines.push(`version: ${ev.tls_version}`);
    if (ev.sni) lines.push(`sni: ${ev.sni}`);
    if (ev.cipher_suite) lines.push(`cipher: ${ev.cipher_suite}`);
    if (ev.src_addr) lines.push(`src: ${ev.src_addr}`);
    if (ev.dst_addr) lines.push(`dst: ${ev.dst_addr}`);
    return lines.length ? lines.join("\n") : "(非 TLS 事件)";
  }

  function formatProcess(ev: UiEvent): string {
    const lines: string[] = [];
    if (ev.process_name) lines.push(`process: ${ev.process_name}`);
    if (ev.pid != null) lines.push(`pid: ${ev.pid}`);
    if (ev.match_quality) lines.push(`match_quality: ${ev.match_quality}`);
    if (ev.protocol) lines.push(`protocol: ${ev.protocol}`);
    if (ev.kind) lines.push(`kind: ${ev.kind}`);
    return lines.length ? lines.join("\n") : "(无进程信息)";
  }

  function formatHeadersCombined(ev: UiEvent): string {
    return `=== Request Headers ===\n${formatHeaders(ev.requestHeaders)}\n\n=== Response Headers ===\n${formatHeaders(ev.responseHeaders)}`;
  }
</script>

{#if event}
  <div class="detail-panel">
    <div class="detail-header">
      <span class="detail-method">{event.method || "—"}</span>
      <span class="detail-url">{event.url || event.info || "—"}</span>
      {#if event.status}
        <span class="detail-status">{event.status}</span>
      {/if}
    </div>

    <div class="detail-tabs" role="tablist">
      {#each tabs as tab, i}
        <button
          class="tab-btn"
          class:active={activeTab === i}
          role="tab"
          aria-selected={activeTab === i}
          onclick={() => (activeTab = i)}
        >
          {tab}
        </button>
      {/each}
    </div>

    <div class="detail-content" role="tabpanel">
      {#if activeTab === 0}
        <pre class="event-json">{formatHeadersCombined(event)}</pre>
      {:else if activeTab === 1}
        <pre class="event-json">{formatBody(event.requestBody)}</pre>
      {:else if activeTab === 2}
        <pre class="event-json">{formatBody(event.responseBody)}</pre>
      {:else if activeTab === 3}
        <pre class="event-json">{formatTiming(event)}</pre>
      {:else if activeTab === 4}
        <pre class="event-json">{formatTLS(event)}</pre>
      {:else if activeTab === 5}
        <pre class="event-json">{formatProcess(event)}</pre>
      {/if}
    </div>
  </div>
{:else}
  <div class="detail-panel">
    <div class="empty-detail">请从列表选择一个事件查看详情</div>
  </div>
{/if}

<style>
  .detail-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--surface);
  }

  .detail-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .detail-method {
    font-weight: 600;
    color: var(--accent);
    font-size: 14px;
  }

  .detail-url {
    display: block;
    color: var(--fg-dim);
    font-size: 12px;
    margin-top: 4px;
    word-break: break-all;
  }

  .detail-status {
    display: inline-block;
    margin-top: 6px;
    font-size: 11px;
    color: var(--fg);
    padding: 2px 6px;
    background: var(--bg);
    border-radius: 4px;
  }

  .detail-tabs {
    display: flex;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
  }

  .tab-btn {
    padding: 10px 16px;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .tab-btn.active,
  .tab-btn:hover {
    color: var(--fg);
    background: var(--hover);
  }

  .tab-btn.active {
    color: var(--accent);
    border-bottom: 2px solid var(--accent);
  }

  .detail-content {
    flex: 1;
    padding: 12px;
    overflow: auto;
  }

  .event-json {
    font-size: 12px;
    line-height: 1.5;
    color: var(--fg);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .empty-detail {
    color: var(--fg-dim);
    text-align: center;
    padding: 40px;
    font-size: 14px;
  }
</style>
