<script lang="ts">
  import type { TrafficEvent } from '../lib/types';
  import { List } from 'svelte-virtual';
  
  let {
    events = $bindable<TrafficEvent[]>([]),
    selectedEvent = $bindable<TrafficEvent | null>(null),
    onreplay,
    onbatchreplay,
    onStartCapture,
  }: {
    events?: TrafficEvent[];
    selectedEvent?: TrafficEvent | null;
    onreplay?: (sessionId: number, delayMs: number, headers: Record<string, string>) => void;
    onbatchreplay?: (sessionIds: number[], delayMs: number) => void;
    onStartCapture?: () => void;
  } = $props();

  let selectedIds = $state<string[]>([]);
  let batchDelayMs = $state<number>(0);

  function toggleSelect(eventId: string) {
    if (selectedIds.includes(eventId)) {
      selectedIds = selectedIds.filter(id => id !== eventId);
    } else {
      selectedIds = [...selectedIds, eventId];
    }
  }

  function selectAll() {
    const allIds = sortedEvents.map(e => e.id);
    if (allIds.length && allIds.every(id => selectedIds.includes(id))) {
      selectedIds = [];
    } else {
      selectedIds = allIds;
    }
  }

  function getSelectedEvents() {
    return sortedEvents.filter(e => selectedIds.includes(e.id));
  }

  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let contextMenuEvent = $state<TrafficEvent | null>(null);
  let showContextMenu = $state(false);

  $effect(() => {
    if (showContextMenu) {
      document.addEventListener('click', closeContextMenu);
    } else {
      document.removeEventListener('click', closeContextMenu);
    }
  });

  function onRowContextMenu(event: MouseEvent, ev: TrafficEvent) {
    event.preventDefault();
    event.stopPropagation();
    contextMenuEvent = ev;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    showContextMenu = true;
  }

  function closeContextMenu() {
    showContextMenu = false;
    contextMenuEvent = null;
  }

  function handleContextReplay() {
    const ev = contextMenuEvent;
    if (!ev || !onreplay || ev.session_id == null) {
      closeContextMenu();
      return;
    }
    onreplay(ev.session_id, 0, {});
    closeContextMenu();
  }

  function handleBatch() {
    const sel = getSelectedEvents();
    const sessionIds = sel.map(e => e.session_id).filter((sid): sid is number => sid != null);
    if (!sessionIds.length || !onbatchreplay) {
      closeContextMenu();
      return;
    }
    onbatchreplay(sessionIds, batchDelayMs || 0);
    closeContextMenu();
  }
  
  // Column configuration
  type ColumnKey = "method" | "url" | "status" | "size" | "time" | "process";
  interface ColumnConfig {
    key: ColumnKey;
    label: string;
    visible: boolean;
    order: number;
  }
  
  const DEFAULT_COLUMNS: ColumnConfig[] = [
    { key: "method", label: "Method", visible: true, order: 0 },
    { key: "url", label: "URL", visible: true, order: 1 },
    { key: "status", label: "Status", visible: true, order: 2 },
    { key: "process", label: "Process", visible: true, order: 3 },
    { key: "size", label: "Size", visible: true, order: 4 },
    { key: "time", label: "Time", visible: true, order: 5 },
  ];
  
  const STORAGE_KEY = 'flowlens-column-config';
  
  function loadColumnConfig(): ColumnConfig[] {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored) as ColumnConfig[];
        return DEFAULT_COLUMNS.map(def => {
          const found = parsed.find(p => p.key === def.key);
          return found ? { ...def, ...found } : def;
        });
      }
    } catch {
      // Ignore parse errors
    }
    return DEFAULT_COLUMNS.map(c => ({ ...c }));
  }
  
  let columnConfig = $state<ColumnConfig[]>(loadColumnConfig());
  
  function saveColumnConfig() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(columnConfig));
    } catch {
      // Ignore write errors
    }
  }
  
  let visibleColumns = $derived.by(() => 
    columnConfig
      .filter(c => c.visible)
      .sort((a, b) => a.order - b.order)
  );
  
  function toggleColumn(key: ColumnKey) {
    const col = columnConfig.find(c => c.key === key);
    if (col) {
      col.visible = !col.visible;
      saveColumnConfig();
    }
  }
  
  function moveColumn(key: ColumnKey, direction: 'up' | 'down') {
    const visible = [...columnConfig].filter(c => c.visible).sort((a, b) => a.order - b.order);
    const idx = visible.findIndex(c => c.key === key);
    if (idx === -1) return;
    
    const targetIdx = direction === 'up' ? idx - 1 : idx + 1;
    if (targetIdx < 0 || targetIdx >= visible.length) return;
    
    const temp = visible[idx].order;
    visible[idx].order = visible[targetIdx].order;
    visible[targetIdx].order = temp;
    saveColumnConfig();
  }
  
  function resetColumns() {
    columnConfig = DEFAULT_COLUMNS.map(c => ({ ...c }));
    saveColumnConfig();
  }
  
  function selectEvent(event: TrafficEvent) {
    selectedEvent = event;
  }

  type SortKey = "method" | "url" | "status" | "size" | "time" | "process";
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
        case "process":
          va = a.process_name ?? "";
          vb = b.process_name ?? "";
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
// Row height for virtual list (matches CSS padding: 8px 12px + text + borders)
  const ROW_HEIGHT = 40;

  $effect(() => {
    const keyIsVisible = columnConfig.find(c => c.key === sortKey)?.visible;
    if (!keyIsVisible && visibleColumns.length > 0) {
      sortKey = visibleColumns[0].key as SortKey;
    }
  });
</script>

<div class="traffic-table">
  <div class="column-config">
    <h3>列配置</h3>
    {#each columnConfig as col}
      <label class="column-row">
        <input type="checkbox" checked={col.visible} onchange={() => toggleColumn(col.key)} />
        {col.label}
        <button class="move-btn" class:disabled={!col.visible} onclick={() => moveColumn(col.key, 'up')}>↑</button>
        <button class="move-btn" class:disabled={!col.visible} onclick={() => moveColumn(col.key, 'down')}>↓</button>
      </label>
    {/each}
    <button class="reset-btn" onclick={resetColumns}>重置列</button>
  </div>
  
  {#if events.length > 0}
    <div class="batch-toolbar">
      {#if selectedIds.length > 0}
        <span class="batch-count">选中 {selectedIds.length} 条</span>
        <label class="delay-field" title="批量重放的间隔延迟">间隔(ms)
          <input type="number" min="0" step="50" class="delay-input" bind:value={batchDelayMs} />
        </label>
        <button class="batch-btn" onclick={handleBatch} disabled={!onbatchreplay}>批量重放</button>
      {:else}
        <span class="batch-count" style="color:var(--fg-dim)">未选中任何事件</span>
      {/if}
      <button class="toggle-btn" onclick={selectAll}>{selectedIds.length === sortedEvents.length && sortedEvents.length > 0 ? '取消全选' : '全选'}</button>
    </div>
    <table>
      <thead>
        <tr>
          <th class="select-col">
            <input
              type="checkbox"
              checked={sortedEvents.length > 0 && sortedEvents.every(e => isSelected(e.id))}
              onchange={selectAll}
              title="全选/取消全选"
            />
          </th>
          {#each visibleColumns as col}
            <th onclick={() => toggleSort(col.key as SortKey)} class="sortable">
              {col.label} {sortKey === col.key ? (sortAsc ? "▲" : "▼") : ""}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        <List
          itemCount={sortedEvents.length}
          itemSize={ROW_HEIGHT}
          height="100%"
          let:row
        >
          {@const event = sortedEvents[row.index]}
          <tr
            class="event-row"
            class:selected={selectedEvent?.id === event.id}
            onclick={() => selectEvent(event)}
            oncontextmenu={(e) => onRowContextMenu(e, event)}
          >
            <td class="select-col" onclick={(e) => e.stopPropagation()}>
              <input
                type="checkbox"
                checked={isSelected(event.id)}
                onchange={() => toggleSelect(event.id)}
              />
            </td>
            {#each visibleColumns as col}
              <td class={col.key}>
                {col.key === "method" ? event.method || '—'
                 : col.key === "url" ? event.url || event.info || '—'
                 : col.key === "status" ? event.status || '—'
                 : col.key === "process" ? `◈ ${event.process_name || '—'} (PID: ${event.pid || '—'})` : 
                  col.key === "size" ? event.size || '—'
                  : event.time || '—'}
              </td>
            {/each}
          </tr>
        </List>
      </tbody>
    </table>
  {:else}
    <div class="empty-state">
      <span class="empty-icon">🌐</span>
      <p class="empty-title">No traffic events found</p>
      <p class="empty-desc">开始捕获网络流量后，这里将实时显示请求与响应。</p>
      <button class="start-capture-btn" onclick={onStartCapture}>开始捕获</button>
    </div>
  {/if}

  {#if showContextMenu && contextMenuEvent}
    <div class="context-menu" style="left:{contextMenuX}px; top:{contextMenuY}px">
      <button class="ctx-item" onclick={handleContextReplay} disabled={contextMenuEvent?.session_id == null || !onreplay}>
        ▶ 重放该事件
      </button>
      {#if onbatchreplay}
        <button class="ctx-item" onclick={handleBatch} disabled={!getSelectedEvents().length}>
          ↪ 批量重放选中项
        </button>
      {/if}
      <button class="ctx-item" onclick={closeContextMenu}>关闭</button>
    </div>
  {/if}
</div>

<style>
  .traffic-table {
    width: 100%;
    overflow: auto;
    flex: 1;
    position: relative;
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
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 64px 24px;
    text-align: center;
    color: var(--fg-dim);
  }

  .empty-state .empty-icon {
    font-size: 48px;
    line-height: 1;
    margin-bottom: 8px;
    opacity: 0.85;
  }

  .empty-state .empty-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--fg);
  }

  .empty-state .empty-desc {
    margin: 0;
    max-width: 360px;
    font-size: 12px;
    color: var(--fg-dim);
    line-height: 1.5;
  }

  .start-capture-btn {
    margin-top: 16px;
    padding: 8px 18px;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    transition: filter 0.15s ease;
  }

  .start-capture-btn:hover {
    filter: brightness(1.12);
  }

  .start-capture-btn:active {
    filter: brightness(0.9);
  }

  .start-capture-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .column-config {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .column-config h3 {
    margin: 0 0 8px 0;
    font-size: 13px;
    color: var(--fg);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .column-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 4px 0;
    font-size: 13px;
    color: var(--fg);
  }

  .column-row input {
    margin: 0;
  }

  .move-btn {
    width: 24px;
    height: 24px;
    font-size: 12px;
    margin-left: 6px;
    background: var(--panel);
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: 3px;
    cursor: pointer;
  }

  .move-btn:hover {
    background: var(--hover);
    color: var(--fg);
  }

  .move-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .reset-btn {
    margin-top: 8px;
    padding: 4px 10px;
    font-size: 12px;
    background: var(--panel);
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: 4px;
    cursor: pointer;
  }

  .reset-btn:hover {
    background: var(--hover);
  }

  .batch-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .batch-count {
    color: var(--fg);
    font-weight: 500;
  }

  .delay-field {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--fg-dim);
    font-size: 11px;
  }

  .delay-input {
    width: 70px;
    padding: 3px 6px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 12px;
  }

  .batch-btn {
    padding: 4px 12px;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 600;
    font-size: 12px;
  }

  .batch-btn:hover:not(:disabled) {
    filter: brightness(1.12);
  }

  .batch-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .toggle-btn {
    margin-left: auto;
    padding: 4px 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    color: var(--fg);
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }

  .toggle-btn:hover {
    background: var(--hover);
  }

  .select-col {
    width: 28px;
    text-align: center;
  }

  .context-menu {
    position: fixed;
    min-width: 180px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
    z-index: 100;
    padding: 4px 0;
  }

  .ctx-item {
    display: block;
    width: 100%;
    padding: 6px 12px;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--fg);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .ctx-item:hover:not(:disabled) {
    background: var(--hover);
  }

  .ctx-item:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .ctx-item + .ctx-item {
    border-top: 1px solid var(--border);
  }
</style>
