<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { TrafficEvent } from '../lib/types';
  
  export let events: TrafficEvent[] = [];
  export let selectedEvent: TrafficEvent | null = null;
  
  const dispatch = createEventDispatcher();
  
  function selectEvent(event: TrafficEvent) {
    dispatch('select', event);
  }
</script>

<div class="traffic-table">
  {#if events.length > 0}
    <table>
      <thead>
        <tr>
          <th>Method</th>
          <th>URL</th>
          <th>Status</th>
          <th>Size</th>
          <th>Time</th>
        </tr>
      </thead>
      <tbody>
        {#each events as event (event.id)}
          <tr
            class="event-row"
            class:selected={selectedEvent?.id === event.id}
            on:click={() => selectEvent(event)}
          >
            <td class="method">{event.method || '—'}</td>
            <td class="url">{event.url || event.info || '—'}</td>
            <td class="status">{event.status || '—'}</td>
            <td class="size">{event.size || '—'}</td>
            <td class="time">{event.time || '—'}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <div class="empty-state">No traffic events found</div>
  {/if}
</div>

<style>
  .traffic-table {
    width: 100%;
    overflow: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  th, td {
    padding: 8px 12px;
    text-align: left;
    border-bottom: 1px solid #222;
  }

  th {
    background: #151520;
    color: #bbb;
    font-weight: 500;
  }

  .event-row:hover {
    background: #1a1a2e;
  }

  .event-row.selected {
    background: #201f33;
  }

  .method {
    color: #7c5af3;
    font-weight: 600;
  }

  .url {
    color: #aaa;
    font-size: 12px;
  }

  .empty-state {
    padding: 20px;
    text-align: center;
    color: #555;
  }
</style>