<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  
  let { events = $bindable<TrafficEvent[]>([]), selectedEvent = $bindable<TrafficEvent | null>(null) } = $props();
  
  function selectEvent(event: TrafficEvent) {
    selectedEvent = event;
  }
  
  function clearEvents() {
    events = [];
    selectedEvent = null;
  }
  
  export function dispatch(e: CustomEvent) {
    // Forward custom events if needed
  }
</script>

<div class="sidebar">
  <div class="search-bar">
    <input type="text" placeholder="搜索事件..." />
  </div>
  <div class="event-list" role="list">
    {#each events as event (event.id)}
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
    {#if events.length === 0}
      <div class="empty-state">暂无网络流量</div>
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
