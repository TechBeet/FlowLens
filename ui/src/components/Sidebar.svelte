<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { TrafficEvent } from '../lib/types';
  
  export let events: TrafficEvent[] = [];
  export let selectedEvent: TrafficEvent | null = null;
  
  const dispatch = createEventDispatcher();
  
  function selectEvent(event: TrafficEvent) {
    dispatch('select', event);
  }
  
  function clearEvents() {
    dispatch('clear');
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
</div>

<style>
  .sidebar {
    width: 320px;
    min-width: 200px;
    display: flex;
    flex-direction: column;
    background: #12121e;
    border-right: 1px solid #222;
    overflow: hidden;
  }

  .search-bar {
    padding: 10px;
    border-bottom: 1px solid #222;
  }

  .search-bar input {
    width: 100%;
    padding: 6px 10px;
    background: #1a1a2a;
    border: 1px solid #333;
    border-radius: 4px;
    font-size: 13px;
    color: #e0e0e0;
  }

  .event-list {
    flex: 1;
    overflow: auto;
  }

  .event-item {
    padding: 8px 12px;
    border-bottom: 1px solid #1a1a2a;
    display: flex;
    flex-direction: column;
    cursor: pointer;
    transition: background 0.15s;
    font-size: 13px;
  }

  .event-item:hover {
    background: #1a1a2e;
  }

  .event-item.selected {
    background: #201f33;
  }

  .event-item:focus {
    outline: 2px solid #7c5af3;
    outline-offset: -2px;
  }

  .event-method {
    font-weight: 600;
    color: #7c5af3;
  }

  .event-url {
    color: #aaa;
    font-size: 12px;
    margin-top: 2px;
  }

  .event-status {
    font-size: 11px;
    color: #666;
    margin-top: 2px;
    align-self: flex-end;
  }

  .empty-state {
    padding: 20px;
    text-align: center;
    color: #555;
    font-size: 13px;
  }
</style>