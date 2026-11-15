// FlowLens Phase 1 — Event store using Svelte 5 runes
import type { UiEvent, TrafficStats, FilterState, TrafficEvent } from "../types";
import * as api from "../api";
import {
  subscribeTrafficEvents,
  filterEvents,
  addBatchStats,
  type AggregatedStats,
} from "../composables";

const DEFAULT_FILTER: FilterState = { host: "", pid: "", process: "", method: "", status: "" };

/**
 * Captures the live-capture workstation state.
 * Exposes reactive state ($state) and derived values ($derived/$derived.by)
 * plus async actions wired to the Tauri API.
 *
 * The store subscribes to the Tauri "traffic-event" event stream
 * emitted by the capture pump in commands.rs (line 167).
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
  let loading = $state(false);
  let error = $state<string | null>(null);

  // ----- MAX_EVENTS initialization (once) -----
  let MAX_EVENTS = 10000; // default
  (async () => {
    try {
      const cap = await api.getEventMax();
      MAX_EVENTS = cap;
    } catch {
      MAX_EVENTS = 10000;
    }
  })();

  // ----- event batching buffer -----
  let eventBuffer: TrafficEvent[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const BATCH_FLUSH_MS = 50;

  function flushBuffer() {
    if (eventBuffer.length === 0) {
      flushTimer = null;
      return;
    }
    const batch = [...eventBuffer];
    eventBuffer.length = 0;
    // Ring-buffer bounded append for the main events array
    if (events.length >= MAX_EVENTS) {
      events.splice(0, events.length - MAX_EVENTS + 1);
    }
    events.push(...batch);
    // Update per-protocol stats via composable (mirrors Rust tally())
    addBatchStats(stats as AggregatedStats, batch);
    flushTimer = setTimeout(flushBuffer, BATCH_FLUSH_MS);
  }

  // ----- subscribe to Tauri event stream -----
  // Uses the reusable subscribeTrafficEvents composable.
  (async () => {
    try {
      const stop = subscribeTrafficEvents((ev) => {
        const p = ev as TrafficEvent;
        // Add to buffer instead of direct append
        eventBuffer.push(p);
        // Start flush timer if not already running
        if (!flushTimer) {
          flushTimer = setTimeout(flushBuffer, BATCH_FLUSH_MS);
        }
      });
      // keep handle alive for the store lifetime; no cleanup needed until destroy
      void stop;
    } catch (e) {
      setError(`订阅事件流失败：${e}`);
    }
  })();

  // ----- derived -----
  const filteredEvents = $derived.by(() => filterEvents(events, filterState));

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
      await api.startCapture(device);
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
      events.length = 0;
      selectedEvent = null;
    } catch (e) {
      error = `清除失败：${e}`;
      throw e;
    } finally {
      loading = false;
    }
  }

  async function exportHAR(
    path: string,
    options?: Partial<api.HarExportOptions>,
  ) {
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

  function select(ev: UiEvent | null) {
    selectedEvent = ev;
  }

  // Maximum number of events to retain in the ring buffer.
  // Initialized from Tauri getEventMax command, falls back to 10000.
  function appendEvent(ev: UiEvent) {
    // Keep the ring buffer bounded to avoid unbounded memory growth.
    if (events.length >= MAX_EVENTS) {
      events.splice(0, events.length - MAX_EVENTS + 1);
    }
    events.push(ev);
    addBatchStats(stats as AggregatedStats, [ev]);
  }

  function setError(msg: string | null) {
    error = msg;
  }

  return {
    // state
    events,
    selectedEvent,
    stats,
    filterState,
    loading,
    error,
    // derived
    filteredEvents,
    isEmpty,
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
