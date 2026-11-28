<script lang="ts">
  import type { UiEvent } from "../lib/types";
  import type { EventStore } from "../lib/stores/eventStore.svelte";
  import { createEventStore } from "../lib/stores/eventStore.svelte";

  interface Props {
    store?: EventStore;
    events?: UiEvent[];
  }

  let { store = createEventStore(), events = [] }: Props = $props();

  // Reference to the first (host) filter input, exposed via `bind:this`.
  let hostInput = $state<HTMLInputElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);

  function handleClear() {
    store.clearFilter();
    store.clearSearch();
  }

  /** Focus and select the host input, scrolling it into view when possible. */
  export function focus() {
    if (!hostInput) return;
    hostInput.focus({ preventScroll: false });
    try {
      hostInput.setSelectionRange(0, hostInput.value.length);
    } catch {
      // setSelectionRange can throw on some input types; focus is sufficient.
    }
    if (typeof hostInput.scrollIntoView === "function") {
      hostInput.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }
</script>

<div class="filter-bar" role="search" aria-label="流量过滤">
  <label class="filter-label" for="filter-search">搜索</label>
  <input
    type="search"
    id="filter-search"
    bind:this={searchInput}
    bind:value={store.searchQuery}
    oninput={(e) => store.setSearch((e.currentTarget as HTMLInputElement).value)}
    placeholder="关键字（method/url/状态）"
    class="filter-input filter-search"
  />

  <label class="filter-label" for="filter-host">主机</label>
  <input
    type="text"
    id="filter-host"
    bind:this={hostInput}
    bind:value={store.filterState.host}
    placeholder="host (url 片段)"
    class="filter-input"
  />

  <label class="filter-label" for="filter-process">进程</label>
  <input
    type="text"
    id="filter-process"
    bind:value={store.filterState.process}
    placeholder="进程名或 PID"
    class="filter-input"
  />

  <label class="filter-label" for="filter-pid">PID</label>
  <input
    type="text"
    id="filter-pid"
    bind:value={store.filterState.pid}
    placeholder="进程 ID"
    class="filter-input"
  />

  <label class="filter-label" for="filter-method">方法</label>
  <input
    type="text"
    id="filter-method"
    bind:value={store.filterState.method}
    placeholder="GET/POST/..."
    class="filter-input"
  />

  <label class="filter-label" for="filter-status">状态</label>
  <input
    type="text"
    id="filter-status"
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

  .filter-search {
    width: 200px;
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
