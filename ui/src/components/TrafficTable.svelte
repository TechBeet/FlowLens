<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  
  let { events = $bindable<TrafficEvent[]>([]), selectedEvent = $bindable<TrafficEvent | null>(null) } = $props();
  
  function selectEvent(event: TrafficEvent) {
    selectedEvent = event;
  }

  type SortKey = "method" | "url" | "status" | "size" | "time";
  let sortKey = $state<SortKey>("time");
  let sortAsc = $state(true);

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      sortAsc = !sortAsc;
    } else {
      sortKey = key;
      sortAsc = true;
    }
  }

  let sortedEvents = $derived.by(() => {
    const arr = [...events];
    arr.sort((a, b) => {
      let va: string | number = "";
      let vb: string | number = "";
      switch (sortKey) {
        case "method":
          va = a.method ?? "";
          vb = b.method ?? "";
          break;
        case "url":
          va = a.url ?? a.info ?? "";
          vb = b.url ?? b.info ?? "";
          break;
        case "status":
          va = a.status ?? "";
          vb = b.status ?? "";
          break;
        case "size":
          va = a.size ?? 0;
          vb = b.size ?? 0;
          break;
        case "time":
          va = a.time ?? "";
          vb = b.time ?? "";
          break;
      }
      if (typeof va === "number" && typeof vb === "number") {
        return sortAsc ? va - vb : vb - va;
      }
      const cmp = String(va).localeCompare(String(vb));
      return sortAsc ? cmp : -cmp;
    });
    return arr;
  });
</script>

<div class="traffic-table">
  {#if events.length > 0}
    <table>
      <thead>
        <tr>
          <th onclick={() => toggleSort("method")} class="sortable">Method {sortKey === "method" ? (sortAsc ? "▲" : "▼") : ""}</th>
          <th onclick={() => toggleSort("url")} class="sortable">URL {sortKey === "url" ? (sortAsc ? "▲" : "▼") : ""}</th>
          <th onclick={() => toggleSort("status")} class="sortable">Status {sortKey === "status" ? (sortAsc ? "▲" : "▼") : ""}</th>
          <th onclick={() => toggleSort("size")} class="sortable">Size {sortKey === "size" ? (sortAsc ? "▲" : "▼") : ""}</th>
          <th onclick={() => toggleSort("time")} class="sortable">Time {sortKey === "time" ? (sortAsc ? "▲" : "▼") : ""}</th>
        </tr>
      </thead>
      <tbody>
        {#each sortedEvents as event (event.id)}
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
    cursor: pointer;
  }

  th:hover {
    background: var(--hover);
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
