<script lang="ts">
  interface ReplayEntry {
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

  let { entries = [], onclear }: { entries: ReplayEntry[]; onclear?: () => void } = $props();

  function summary(entry: ReplayEntry): string {
    if (entry.status === "error") return entry.error ?? "未知错误";
    const r = entry.result;
    if (r && typeof r === "object") {
      const obj = r as Record<string, unknown>;
      const status = obj.status ?? obj.statusCode ?? obj.code;
      if (status != null) return `状态 ${status}`;
    }
    if (typeof r === "string") return r.slice(0, 60);
    return "成功";
  }
</script>

<div class="replay-history">
  <div class="history-head">
    <h3>重放历史</h3>
    {#if entries.length > 0}
      <span class="count">{entries.length}</span>
      {#if onclear}
        <button class="clear-btn" onclick={onclear} title="清空重放历史">清空</button>
      {/if}
    {/if}
  </div>
  {#if entries.length === 0}
    <p class="empty">暂无重放记录</p>
  {:else}
    <ul>
      {#each entries as entry (entry.id)}
        <li class:error={entry.status === "error"}>
          <span class="time">{entry.time}</span>
          <span class="label">{entry.label ?? `#${entry.sessionId}`}</span>
          {#if entry.delayMs > 0}
            <span class="meta" title="延迟">+{entry.delayMs}ms</span>
          {/if}
          {#if entry.headers && Object.keys(entry.headers).length > 0}
            <span class="meta" title={Object.entries(entry.headers).map(([k, v]) => `${k}: ${v}`).join("\n")}>
              {Object.keys(entry.headers).length} 头
            </span>
          {/if}
          <span class="detail" class:err-text={entry.status === "error"}>{summary(entry)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .replay-history {
    padding: 8px 12px;
    border-top: 1px solid var(--border);
    color: var(--fg);
    font-size: 12px;
    max-height: 220px;
    overflow-y: auto;
    background: var(--surface);
  }
  .history-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .replay-history h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
  }
  .count {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 6px;
    font-size: 10px;
    color: var(--fg-dim);
  }
  .clear-btn {
    margin-left: auto;
    padding: 1px 8px;
    font-size: 11px;
    background: var(--panel);
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: 4px;
    cursor: pointer;
  }
  .clear-btn:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .empty {
    color: var(--fg-dim);
    margin: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 2px 0;
    font-family: "Consolas", monospace;
    border-bottom: 1px dashed var(--border);
  }
  li:last-child {
    border-bottom: none;
  }
  li.error .label {
    color: var(--err);
  }
  .time {
    color: var(--fg-dim);
    flex-shrink: 0;
  }
  .label {
    flex-shrink: 0;
  }
  .meta {
    color: var(--fg-dim);
    font-size: 10px;
    flex-shrink: 0;
  }
  .detail {
    color: var(--fg-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail.err-text {
    color: var(--err);
  }
</style>