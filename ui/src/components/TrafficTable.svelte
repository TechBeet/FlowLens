<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  
  let { events = $bindable<TrafficEvent[]>([]), selectedEvent = $bindable<TrafficEvent | null>(null) } = $props();
  
  function selectEvent(event: TrafficEvent) {
    selectedEvent = event;
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
            onclick={() => selectEvent(event)}
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
    flex: 1;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  th, td {
    padding: 8px 12px;
    text-align: left;
    border-bottom: 1px solid var(--border);
  }

  th {
    background: var(--panel);
    color: var(--fg);
    font-weight: 500;
    position: sticky;
    top: 0;
  }

  tr:nth-child(even) {
    background: var(--row-alt);
  }

  .event-row:hover {
    background: var(--hover);
  }

  .event-row.selected {
    background: var(--selected);
  }

  .method {
    color: var(--accent);
    font-weight: 600;
  }

  .url {
    color: var(--fg-dim);
    font-size: 12px;
  }

  .empty-state {
    padding: 20px;
    text-align: center;
    color: var(--fg-dim);
  }
</style>
