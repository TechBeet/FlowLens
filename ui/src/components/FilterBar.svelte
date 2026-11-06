<script lang="ts">
  import type { UiEvent } from "../lib/types";
  import type { EventStore } from "../lib/stores/eventStore";
  import { createEventStore } from "../lib/stores/eventStore";

  interface Props {
    store?: EventStore;
    events?: UiEvent[];
  }

  let { store = createEventStore(), events = [] }: Props = $props();

  function handleClear() {
    store.clearFilter();
  }
</script>

<div class="filter-bar" role="search" aria-label="流量过滤">
  <label class="filter-label">主机</label>
  <input
    type="text"
    bind:value={store.filterState.host}
    placeholder="host (url 片段)"
    class="filter-input"
  />

  <label class="filter-label">PID</label>
  <input
    type="text"
    bind:value={store.filterState.pid}
    placeholder="进程 ID"
    class="filter-input"
  />

  <label class="filter-label">方法</label>
  <input
    type="text"
    bind:value={store.filterState.method}
    placeholder="GET/POST/..."
    class="filter-input"
  />

  <label class="filter-label">状态</label>
  <input
    type="text"
    bind:value={store.filterState.status}
    placeholder="200/404/..."
    class="filter-input"
  />

  <button class="btn btn-clear" onclick={handleClear} title="清除过滤器">
    清除
  </button>

  <span class="filter-count" aria-live="polite">
    {store.filteredEvents.length} / {store.events.length} 匹配
  </span>
</div>

<style>
  .filter-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .filter-label {
    font-size: 11px;
    color: var(--fg-dim);
    white-space: nowrap;
  }

  .filter-input {
    width: 120px;
    padding: 4px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 12px;
  }

  .filter-input:focus {
    border-color: var(--accent);
  }

  .btn-clear {
    padding: 4px 10px;
    font-size: 12px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    cursor: pointer;
  }

  .btn-clear:hover {
    background: var(--hover);
  }

  .filter-count {
    margin-left: auto;
    font-size: 11px;
    color: var(--fg-dim);
    white-space: nowrap;
  }
</style>
