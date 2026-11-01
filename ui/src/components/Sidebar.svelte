<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  
  let { events = $bindable<TrafficEvent[]>([]), selectedEvent = $bindable<TrafficEvent | null>(null) } = $props();

  let searchQuery = $state("");
  
  function selectEvent(event: TrafficEvent) {
    selectedEvent = event;
  }
  
  function clearEvents() {
    events = [];
    selectedEvent = null;
  }

  let filteredEvents = $derived(
    searchQuery.trim() === ""
      ? events
      : events.filter((e) => {
          const q = searchQuery.toLowerCase();
          return (
            (e.method?.toLowerCase().includes(q) ?? false) ||
            (e.url?.toLowerCase().includes(q) ?? false) ||
            (e.info?.toLowerCase().includes(q) ?? false) ||
            (e.status?.toLowerCase().includes(q) ?? false)
          );
        })
  );
  
  export function dispatch(e: CustomEvent) {
    // Forward custom events if needed
  }
</script>

<div class="sidebar">
  <div class="search-bar">
    <input type="text" bind:value={searchQuery} placeholder="搜索事件..." />
  </div>
  <div class="event-list" role="list">
    {#each filteredEvents as event (event.id)}
      <button
        type="button"
        class="event-item"
        class:selected={selectedEvent?.id === event.id}
        onclick={() => selectEvent(event)}
      >
        <span class="event-method">{event.method || '—'}</span>
        <span class="event-url">{event.url || event.info || ''}</span>
        {#if event.status}
          <span class="event-status">{event.status}</span>
        {/if}
      </button>
    {/each}
    {#if filteredEvents.length === 0}
      <div class="empty-state">
        {#if searchQuery.trim()}
          未匹配到事件
        {:else}
          暂无网络流量
        {/if}
      </div>
    {/if}
  </div>
  <button class="clear-btn" onclick={clearEvents}>清除全部</button>
</div>

<style>
  .sidebar {
    width: 320px;
    min-width: 200px;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-right: 1px solid var(--border);
    overflow: hidden;
  }

  .search-bar {
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }

  .search-bar input {
    width: 100%;
    padding: 6px 10px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 13px;
    color: var(--fg);
  }

  .event-list {
    flex: 1;
    overflow: auto;
  }

  .event-item {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    cursor: pointer;
    transition: background 0.15s;
    font-size: 13px;
    text-align: left;
    width: 100%;
  }

  .event-item:hover {
    background: var(--hover);
  }

  .event-item.selected {
    background: var(--selected);
  }

  .event-item:focus {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .event-method {
    font-weight: 600;
    color: var(--accent);
  }

  .event-url {
    color: var(--fg-dim);
    font-size: 12px;
    margin-top: 2px;
  }

  .event-status {
    font-size: 11px;
    color: var(--fg-dim);
    margin-top: 2px;
    align-self: flex-end;
  }

  .empty-state {
    padding: 20px;
    text-align: center;
    color: var(--fg-dim);
    font-size: 13px;
  }

  .clear-btn {
    margin: 10px;
    padding: 8px 12px;
    background: var(--err);
    border-color: var(--err);
    color: white;
    font-size: 12px;
  }

  .clear-btn:hover {
    filter: brightness(0.9);
  }
</style>
