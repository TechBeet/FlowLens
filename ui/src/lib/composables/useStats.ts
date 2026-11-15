// FlowLens — composable for aggregating traffic statistics.
//
// Computes per-protocol counters (connections, http, tls, sse, websocket)
// from a list of UiEvent/TrafficEvent items. Keeps stats-aggregation logic
// in one place so both the eventStore and components can reuse it.
import type { UiEvent } from "../types";

export interface AggregatedStats {
  connections: number;
  http: number;
  tls: number;
  sse: number;
  websocket: number;
  total: number;
}

export const EMPTY_STATS: AggregatedStats = {
  connections: 0,
  http: 0,
  tls: 0,
  sse: 0,
  websocket: 0,
  total: 0,
};

/**
 * Classify a single event into protocol buckets.
 * Mirrors the Rust `tally()` function in ui/src-tauri/src/commands.rs.
 */
function classifyKind(kind: string): keyof AggregatedStats | null {
  const k = (kind || "").toLowerCase();
  if (k === "connection" || k === "closed" || k === "dropped") return "connections";
  if (k === "http") return "http";
  if (k === "tls") return "tls";
  if (k === "sse") return "sse";
  if (k === "ws" || k === "websocket") return "websocket";
  return null;
}

/**
 * Aggregate stats from an array of events.
 */
export function aggregateStats(events: UiEvent[]): AggregatedStats {
  const stats: AggregatedStats = { ...EMPTY_STATS };
  for (const e of events) {
    stats.total += 1;
    const bucket = classifyKind(e.kind);
    if (bucket) stats[bucket] += 1;
  }
  return stats;
}

/**
 * Incrementally add a batch of events into an existing stats object.
 * Returns the mutated stats object (convenient for $state updates).
 */
export function addBatchStats(stats: AggregatedStats, batch: UiEvent[]): AggregatedStats {
  for (const e of batch) {
    stats.total += 1;
    const bucket = classifyKind(e.kind);
    if (bucket) stats[bucket] += 1;
  }
  return stats;
}
