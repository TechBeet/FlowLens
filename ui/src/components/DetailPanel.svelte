<script lang="ts">
  import type { UiEvent } from "../lib/types";

  interface Props {
    event?: UiEvent | null;
    onreplay?: (sessionId: number, delayMs: number, headers: Record<string, string>) => void;
  }

  let { event = null, onreplay }: Props = $props();

  let activeTab = $state(0);
  const tabs = ["Headers", "Request Body", "Response Body", "Timing", "TLS", "Process"];

  // --- Q7: session replay configuration ---
  let replayDelayMs = $state<number>(0);
  let replayHeaderText = $state<string>("");

  function currentSessionId(): number | null {
    const sid = event?.session_id;
    return typeof sid === "number" ? sid : null;
  }

  function parseReplayHeaders(): Record<string, string> {
    const text = replayHeaderText.trim();
    if (!text) return {};
    const out: Record<string, string> = {};
    for (const line of text.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      const idx = trimmed.indexOf(":");
      if (idx <= 0) continue;
      const key = trimmed.slice(0, idx).trim();
      const value = trimmed.slice(idx + 1).trim();
      if (key) out[key] = value;
    }
    return out;
  }

  function handleReplay() {
    const sid = currentSessionId();
    if (sid == null) return;
    onreplay?.(sid, replayDelayMs, parseReplayHeaders());
  }

  // Lazy-loaded body content — only populated once its tab is first opened.
  let lazyBodies = $state<{ request?: string | null; response?: string | null }>({});

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
    if (ev.rewrite_applied) lines.push(`rewrite_applied: ${ev.rewrite_applied}`);
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

  function ensureLazyBodies(ev: UiEvent, tabIndex: number) {
    if (tabIndex === 1 && !lazyBodies.request) {
      lazyBodies.request = formatBody(ev.requestBody);
    }
    if (tabIndex === 2 && !lazyBodies.response) {
      lazyBodies.response = formatBody(ev.responseBody);
    }
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
      {#if event.process_name || event.pid != null}
        <span class="detail-process" title="进程归属">
          <span class="process-icon">◈</span>
          {event.process_name || "未知进程"}
          {#if event.pid != null}<span class="process-pid">PID {event.pid}</span>{/if}
        </span>
      {/if}
      <button
        class="header-replay-btn"
        disabled={currentSessionId() == null}
        onclick={handleReplay}
        title={currentSessionId() == null ? "该事件没有会话 ID，无法重放" : "重放此请求"}
      >▶ 重放</button>
    </div>

    <!-- Q7: session replay controls -->
    <div class="replay-bar">
      <label class="replay-field">
        延迟(ms)
        <input type="number" min="0" step="50" bind:value={replayDelayMs} class="replay-input" />
      </label>
      <label class="replay-field grow">
        自定义请求头 (每行 "Key: Value")
        <input
          type="text"
          bind:value={replayHeaderText}
          placeholder="Authorization: Bearer xxx"
          class="replay-input"
        />
      </label>
      <button
        class="replay-btn"
        disabled={currentSessionId() == null}
        onclick={handleReplay}
        title={currentSessionId() == null ? "该事件没有会话 ID，无法重放" : "重放此请求"}
      >
        ▶ 重放
      </button>
    </div>

    <div class="detail-tabs" role="tablist">
      {#each tabs as tab, i}
        <button
          class="tab-btn"
          class:active={activeTab === i}
          role="tab"
          aria-selected={activeTab === i}
          onclick={() => { activeTab = i; if (event) ensureLazyBodies(event, i); }}
        >
          {tab}
        </button>
      {/each}
    </div>

    <div class="detail-content" role="tabpanel">
      {#if activeTab === 0}
        <pre class="event-json">{formatHeadersCombined(event)}</pre>
      {:else if activeTab === 1}
        <pre class="event-json">{lazyBodies.request ?? formatBody(event.requestBody)}</pre>
      {:else if activeTab === 2}
        <pre class="event-json">{lazyBodies.response ?? formatBody(event.responseBody)}</pre>
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

  .detail-process {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: 8px;
    font-size: 11px;
    color: var(--accent);
    padding: 2px 8px;
    background: var(--bg);
    border-radius: 4px;
  }

  .process-icon {
    color: var(--ok);
  }

  .process-pid {
    color: var(--fg-dim);
    margin-left: 2px;
  }

  /* Detail header replay button */
  .header-replay-btn {
    margin-left: auto;
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 600;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .header-replay-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .header-replay-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ---- Q7: replay bar ---- */
  .replay-bar {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 10px 16px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .replay-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
    color: var(--fg-dim);
  }

  .replay-field.grow {
    flex: 1;
    min-width: 200px;
  }

  .replay-input {
    padding: 4px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 12px;
    font-family: inherit;
  }

  .replay-btn {
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 600;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: filter 0.15s ease, opacity 0.15s ease;
  }

  .replay-btn:hover:not(:disabled) {
    filter: brightness(1.12);
  }

  .replay-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
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
