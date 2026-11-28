// FlowLens Phase 1 — Event store using Svelte 5 runes.
//
// NOTE: this module is `eventStore.svelte.ts` (not `.ts`) because it uses the
// `$state` / `$derived` runes, which are only compiled inside `.svelte.ts`
// modules or components.
import type { UiEvent, TrafficStats, FilterState, TrafficEvent } from "../types";
import * as api from "../api";
import {
  subscribeTrafficEvents,
  filterEvents,
  searchEvents,
  addBatchStats,
  type AggregatedStats,
} from "../composables";

const DEFAULT_FILTER: FilterState = { host: "", pid: "", process: "", method: "", status: "" };

/** Coalescing window for incoming events (P1: event batching). */
export const BATCH_FLUSH_MS = 50;

/**
 * Hard cap on events waiting to be flushed. When the backend produces faster
 * than the UI drains, the oldest pending events are dropped so memory stays
 * bounded (P1: backpressure). Dropped events are counted in `droppedEvents`.
 */
export const MAX_PENDING = 5000;

/**
 * Creates the live-capture workstation store.
 *
 * The store subscribes to the Tauri "traffic-event" stream emitted by the
 * capture pump in `commands.rs`, buffers events for `BATCH_FLUSH_MS`, then
 * commits one batch into the reactive `events` ring buffer and folds the batch
 * into `stats` in a single pass.
 */
export function createEventStore() {
  // ----- core reactive state -----
  let events = $state<UiEvent[]>([]);
  let selectedEvent = $state<UiEvent | null>(null);
  let stats = $state<TrafficStats>({
    connections: 0,
    http: 0,
    tls: 0,
    sse: 0,
    websocket: 0,
    total: 0,
  });
  let filterState = $state<FilterState>({ ...DEFAULT_FILTER });
  let searchQuery = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);
  let droppedEvents = $state(0);

  // ----- MAX_EVENTS initialization (once) -----
  let MAX_EVENTS = 10000; // default, refreshed from the backend below
  void (async () => {
    try {
      MAX_EVENTS = await api.getEventMax();
    } catch {
      MAX_EVENTS = 10000;
    }
  })();

  // ----- batching buffer -----
  let eventBuffer: TrafficEvent[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  let disposed = false;
  let unlisten: (() => void) | null = null;


  /** Commit everything currently buffered as one batch. Returns batch size. */
  function flush(): number {
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    if (eventBuffer.length === 0) {
      return 0;
    }

    const batch = eventBuffer;
    eventBuffer = [];

    // Ring-buffer bounded append: keep only the newest MAX_EVENTS entries.
    const overflow = events.length + batch.length - MAX_EVENTS;
    if (overflow > 0) {
      events.splice(0, Math.min(overflow, events.length));
    }
    events.push(...batch);

    // Per-batch stats aggregation (single pass over the batch).
    addBatchStats(stats as AggregatedStats, batch);
    return batch.length;
  }

  function scheduleFlush() {
    if (flushTimer !== null || disposed) return;
    flushTimer = setTimeout(() => {
      flushTimer = null;
      flush();
    }, BATCH_FLUSH_MS);
  }

  /**
   * Ingest one captured event.
   *
   * Applies backpressure by dropping the oldest pending events once the buffer
   * reaches `MAX_PENDING`, then schedules a coalesced flush.
   */
  function ingest(ev: TrafficEvent) {
    if (disposed) return;
    eventBuffer.push(ev);
    if (eventBuffer.length > MAX_PENDING) {
      const dropped = eventBuffer.length - MAX_PENDING;
      eventBuffer.splice(0, dropped);
      droppedEvents += dropped;
    }
    scheduleFlush();
  }

  // ----- subscribe to the Tauri event stream -----
  try {
    unlisten = subscribeTrafficEvents((ev) => ingest(ev as TrafficEvent));
  } catch (e) {
    setError(`订阅事件流失败：${e}`);
  }

  // ----- derived -----
  // Pipeline: structural filter (host/method/…) then free-text keyword search.
  const filteredEvents = $derived.by(() =>
    searchEvents(filterEvents(events, filterState), searchQuery),
  );
  const isEmpty = $derived(events.length === 0 && !loading);


  // ----- actions -----
  async function loadDevices() {
    try {
      return await api.listDevices();
    } catch (e) {
      setError(`加载设备列表失败：${e}`);
      return [];
    }
  }

  async function getStats() {
    try {
      const s = await api.getStats();
      stats.connections = s.connections;
      stats.http = s.http;
      stats.tls = s.tls;
      stats.sse = s.sse;
      stats.websocket = s.websocket;
      stats.total = s.total;
      return s;
    } catch {
      // ignore polling errors
    }
  }

  async function startCapture(device?: string) {
    loading = true;
    error = null;
    try {
      await api.startCapture(undefined, device);
    } catch (e) {
      error = `启动失败：${e}`;
      throw e;
    } finally {
      loading = false;
    }
  }

  async function stopCapture() {
    loading = true;
    error = null;
    try {
      await api.stopCapture();
    } catch (e) {
      error = `停止失败：${e}`;
      throw e;
    } finally {
      loading = false;
    }
  }

  async function clearEvents() {
    loading = true;
    error = null;
    try {
      await api.clearEvents();
      flush();
      events.length = 0;
      eventBuffer = [];
      selectedEvent = null;
    } catch (e) {
      error = `清除失败：${e}`;
      throw e;
    } finally {
      loading = false;
    }
  }

  async function exportHAR(path: string, options?: Partial<api.HarExportOptions>) {
    loading = true;
    error = null;
    try {
      await api.exportHAR(path, options);
    } catch (e) {
      error = `导出 HAR 失败：${e}`;
      throw e;
    } finally {
      loading = false;
    }
  }

  async function getCAPem() {
    return api.getCAPem();
  }

  async function pauseSession(sessionId: number) {
    await api.pauseSession(sessionId);
  }

  async function releaseSession(sessionId: number) {
    await api.releaseSession(sessionId);
  }

  function setFilter(patch: Partial<FilterState>) {
    Object.assign(filterState, patch);
  }

  function clearFilter() {
    Object.assign(filterState, { ...DEFAULT_FILTER });
  }

  function setSearch(query: string) {
    searchQuery = query;
  }

  function clearSearch() {
    searchQuery = "";
  }

  function select(ev: UiEvent | null) {
    selectedEvent = ev;
  }

  /** Append a single event immediately, bypassing the batching window. */
  function appendEvent(ev: UiEvent) {
    if (events.length >= MAX_EVENTS) {
      events.splice(0, events.length - MAX_EVENTS + 1);
    }
    events.push(ev);
    addBatchStats(stats as AggregatedStats, [ev]);
  }

  function setError(msg: string | null) {
    error = msg;
  }

  /** Tear down the event subscription and any pending flush timer. */
  function dispose() {
    disposed = true;
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    unlisten?.();
    unlisten = null;
    eventBuffer = [];
  }

  return {
    // state — exposed as getters so readers always see the current rune value
    // (a plain property would capture the value at store-creation time).
    get events() {
      return events;
    },
    get selectedEvent() {
      return selectedEvent;
    },
    set selectedEvent(ev: UiEvent | null) {
      selectedEvent = ev;
    },
    get stats() {
      return stats;
    },
    get filterState() {
      return filterState;
    },
    get searchQuery() {
      return searchQuery;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get droppedEvents() {
      return droppedEvents;
    },
    // derived
    get filteredEvents() {
      return filteredEvents;
    },
    get isEmpty() {
      return isEmpty;
    },
    // batching / backpressure
    ingest,
    flush,
    dispose,
    get pendingCount() {
      return eventBuffer.length;
    },
    // actions
    loadDevices,
    getStats,
    startCapture,
    stopCapture,
    clearEvents,
    exportHAR,
    getCAPem,
    pauseSession,
    releaseSession,
    setFilter,
    setSearch,
    clearSearch,
    clearFilter,
    select,
    appendEvent,
    setError,
  };
}

export type EventStore = ReturnType<typeof createEventStore>;

/**
 * Reusable composable to access the singleton event store.
 * Components can import this instead of calling createEventStore() directly.
 */
let _singleton: EventStore | null = null;
export function useEventStore(): EventStore {
  if (!_singleton) {
    _singleton = createEventStore();
  }
  return _singleton;
}
