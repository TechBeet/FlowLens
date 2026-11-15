// FlowLens — composable for filtering and sorting traffic events.
//
// Pure functions that can be reused by the eventStore, components, and tests.
// The filter shape mirrors the Rust `TrafficFilter` type but is intentionally
// string-based for the UI (users type free-form text into inputs).
import type { UiEvent, FilterState } from "../types";

export const DEFAULT_FILTER: FilterState = {
  host: "",
  pid: "",
  process: "",
  method: "",
  status: "",
};

/**
 * Apply a text-based filter to an event list.
 * Matches the semantics of the original eventStore `$derived.by` block.
 */
export function filterEvents(events: UiEvent[], filter: FilterState): UiEvent[] {
  const { host, pid, process, method, status } = filter;
  const h = (host || "").toLowerCase();
  const m = (method || "").toLowerCase();
  const s = (status || "").toLowerCase();
  const p = pid || "";
  const proc = (process || "").toLowerCase();

  // Fast path: no filter active.
  if (!h && !p && !proc && !m && !s) return events;

  return events.filter((e) => {
    const url = e.url || e.info || "";
    if (h && !url.toLowerCase().includes(h)) return false;
    if (p && String(e.pid ?? "") !== p) return false;
    if (proc && !(e.process_name ?? "").toLowerCase().includes(proc) && String(e.pid ?? "") !== process) {
      return false;
    }
    if (m && (e.method ?? "").toLowerCase() !== m) return false;
    if (s && (e.status ?? "").toLowerCase() !== s) return false;
    return true;
  });
}

/**
 * Case-insensitive keyword search across method/url/info/status fields.
 */
export function searchEvents(events: UiEvent[], query: string): UiEvent[] {
  const q = (query || "").trim().toLowerCase();
  if (!q) return events;
  return events.filter(
    (e) =>
      (e.method?.toLowerCase().includes(q) ?? false) ||
      (e.url?.toLowerCase().includes(q) ?? false) ||
      (e.info?.toLowerCase().includes(q) ?? false) ||
      (e.status?.toLowerCase().includes(q) ?? false),
  );
}

export type SortKey = "time" | "size" | "method" | "status";
export type SortDir = "asc" | "desc";

/**
 * Sort events by a given key. Returns a new array (does not mutate input).
 * Timestamps fall back to the numeric id when `timestamp_micros` is absent.
 */
export function sortEvents(
  events: UiEvent[],
  key: SortKey = "time",
  dir: SortDir = "asc",
): UiEvent[] {
  const sign = dir === "asc" ? 1 : -1;
  const copy = [...events];
  copy.sort((a, b) => {
    let cmp = 0;
    switch (key) {
      case "time": {
        const ta = a.timestamp_micros ?? (Number(a.id) || 0);
        const tb = b.timestamp_micros ?? (Number(b.id) || 0);
        cmp = ta - tb;
        break;
      }
      case "size":
        cmp = (a.size ?? 0) - (b.size ?? 0);
        break;
      case "method":
        cmp = (a.method ?? "").localeCompare(b.method ?? "");
        break;
      case "status":
        cmp = (a.status ?? "").localeCompare(b.status ?? "");
        break;
    }
    return cmp * sign;
  });
  return copy;
}

/**
 * Whether a filter is currently active (any field non-empty).
 */
export function isFilterActive(filter: FilterState): boolean {
  return Boolean(
    filter.host || filter.pid || filter.process || filter.method || filter.status,
  );
}
