// FlowLens Phase 1 — Event store using Svelte 5 runes
import type { UiEvent, TrafficStats, FilterState } from "../types";
import * as api from "../api";

const DEFAULT_FILTER: FilterState = { host: "", pid: "", method: "", status: "" };

/**
 * Captures the live-capture workstation state.
 * Exposes reactive state ($state) and derived values ($derived/$derived.by)
 * plus async actions wired to the Tauri API.
 */
export function createEventStore() {
  // ----- core reactive state -----
  const events = $state<UiEvent[]>([]);
  const selectedEvent = $state<UiEvent | null>(null);
  const stats = $state<TrafficStats>({
    connections: 0,
    http: 0,
    tls: 0,
    sse: 0,
    websocket: 0,
    total: 0,
  });
  const filterState = $state<FilterState>({ ...DEFAULT_FILTER });
  const loading = $state(false);
  const error = $state<string | null>(null);

  // ----- derived -----
  const filteredEvents = $derived.by(() => {
    const { host, pid, method, status } = filterState;
    const h = host.toLowerCase();
    const m = method.toLowerCase();
    const s = status.toLowerCase();
    const p = pid;
    return events.filter((e) => {
      const url = e.url || e.info || "";
      if (h && !url.includes(h)) return false;
      if (p && String(e.pid ?? "") !== p) return false;
      if (m && (e.method ?? "").toLowerCase() !== m) return false;
      if (s && (e.status ?? "").toLowerCase() !== s) return false;
      return true;
    });
  });

  const isEmpty = $derived(events.length === 0 && !loading);

  // ----- actions -----
  async function loadDevices() {
    try {
      return await api.listDevices();
    } catch (e) {
      throw new Error(`加载设备列表失败：${e}`);
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

  async function exportHAR(path: string) {
    loading = true;
    error = null;
    try {
      await api.exportHAR(path);
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

  function appendEvent(ev: UiEvent) {
    // Keep the ring buffer bounded to avoid unbounded memory growth.
    const MAX = 5000;
    if (events.length >= MAX) {
      events.splice(0, events.length - MAX + 1);
    }
    events.push(ev);
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
