// FlowLens — unit tests for the runes-based event store.
//
// The store talks to Tauri (event stream + commands), so both `../api` and the
// `subscribeTrafficEvents` composable are mocked. Everything else (filtering,
// stats aggregation) runs for real, which is what we want to verify.
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

vi.mock("../composables", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../composables")>();
  const handlers: Array<(ev: unknown) => void> = [];
  const unlistenSpy = vi.fn();
  return {
    ...actual,
    subscribeTrafficEvents: (handler: (ev: unknown) => void) => {
      handlers.push(handler);
      return unlistenSpy;
    },
    __handlers: handlers,
    __unlisten: unlistenSpy,
  };
});

vi.mock("../api", () => ({
  getEventMax: async () => 10000,
  listDevices: async () => ["eth0"],
  getStats: async () => ({
    connections: 0,
    http: 0,
    tls: 0,
    sse: 0,
    websocket: 0,
    total: 0,
  }),
  startCapture: async () => {},
  stopCapture: async () => {},
  clearEvents: async () => {},
  exportHAR: async () => {},
  getCAPem: async () => "PEM",
  pauseSession: async () => {},
  releaseSession: async () => {},
}));

import { createEventStore, BATCH_FLUSH_MS, MAX_PENDING } from "./eventStore.svelte";
import { makeEvent, sampleEvents } from "../__fixtures__/events";
import type { TrafficEvent } from "../types";
import * as composables from "../composables";

const handlers = (composables as unknown as { __handlers: Array<(ev: unknown) => void> }).__handlers;
const unlistenSpy = (composables as unknown as { __unlisten: ReturnType<typeof vi.fn> }).__unlisten;

/** Push an event through the captured subscription handler. */
function emit(ev: unknown) {
  for (const h of handlers) h(ev);
}

describe("eventStore / subscription & batching", () => {
  let store: ReturnType<typeof createEventStore>;

  beforeEach(() => {
    handlers.length = 0;
    unlistenSpy.mockClear();
    vi.useFakeTimers();
    store = createEventStore();
  });

  afterEach(() => {
    store.dispose();
    vi.useRealTimers();
  });

  it("subscribes to the traffic-event stream on creation", () => {
    expect(handlers).toHaveLength(1);
  });

  it("does not commit events before the batching window elapses", () => {
    emit(sampleEvents[1] as TrafficEvent);
    expect(store.events).toHaveLength(0);
    expect(store.pendingCount).toBe(1);
  });

  it("commits buffered events once the 50ms window elapses", () => {
    emit(sampleEvents[1] as TrafficEvent);
    emit(sampleEvents[2] as TrafficEvent);

    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    expect(BATCH_FLUSH_MS).toBe(50);
    expect(store.events).toHaveLength(2);
    expect(store.pendingCount).toBe(0);
  });

  it("coalesces a burst of events into a single batch", () => {
    for (const ev of sampleEvents) emit(ev as TrafficEvent);
    expect(store.pendingCount).toBe(sampleEvents.length);

    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    expect(store.events).toHaveLength(sampleEvents.length);
    expect(store.events.map((e) => e.id)).toEqual(sampleEvents.map((e) => e.id));
  });

  it("aggregates stats per batch, matching the event kinds", () => {
    for (const ev of sampleEvents) emit(ev as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    // sampleEvents: 1 connection, 1 http, 1 tls, 1 sse, 1 websocket, 1 dropped
    expect(store.stats.total).toBe(6);
    expect(store.stats.connections).toBe(2); // "connection" + "dropped"
    expect(store.stats.http).toBe(1);
    expect(store.stats.tls).toBe(1);
    expect(store.stats.sse).toBe(1);
    expect(store.stats.websocket).toBe(1);
  });

  it("accumulates stats across successive batches", () => {
    emit(sampleEvents[1] as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);
    emit(sampleEvents[1] as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    expect(store.stats.total).toBe(2);
    expect(store.stats.http).toBe(2);
    expect(store.events).toHaveLength(2);
  });

  it("flush() commits immediately and reports the batch size", () => {
    emit(sampleEvents[3] as TrafficEvent);
    expect(store.flush()).toBe(1);
    expect(store.events).toHaveLength(1);
    expect(store.flush()).toBe(0);
  });
});

describe("eventStore / backpressure, filtering & lifecycle", () => {
  let store: ReturnType<typeof createEventStore>;

  beforeEach(() => {
    handlers.length = 0;
    unlistenSpy.mockClear();
    vi.useFakeTimers();
    store = createEventStore();
  });

  afterEach(() => {
    store.dispose();
    vi.useRealTimers();
  });

  it("drops the oldest pending events once MAX_PENDING is exceeded", () => {
    const extra = 25;
    for (let i = 0; i < MAX_PENDING + extra; i++) {
      emit(makeEvent({ id: String(i), kind: "http" }) as TrafficEvent);
    }

    expect(store.pendingCount).toBe(MAX_PENDING);
    expect(store.droppedEvents).toBe(extra);

    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    // The surviving events are the newest ones.
    expect(store.events).toHaveLength(MAX_PENDING);
    expect(store.events[0].id).toBe(String(extra));
    expect(store.events[store.events.length - 1].id).toBe(String(MAX_PENDING + extra - 1));
  });

  it("exposes derived filtered events that react to filter changes", () => {
    for (const ev of sampleEvents) emit(ev as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    expect(store.filteredEvents).toHaveLength(sampleEvents.length);

    store.setFilter({ method: "GET" });
    expect(store.filteredEvents.map((e) => e.id).sort()).toEqual(["2", "4", "5"]);

    store.setFilter({ process: "chrome" });
    expect(store.filteredEvents.map((e) => e.id).sort()).toEqual(["2", "5"]);

    store.clearFilter();
    expect(store.filteredEvents).toHaveLength(sampleEvents.length);
  });

  it("ignores events after dispose() and unsubscribes", () => {
    store.dispose();
    emit(sampleEvents[1] as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    expect(unlistenSpy).toHaveBeenCalled();
    expect(store.events).toHaveLength(0);
    expect(store.pendingCount).toBe(0);
  });

  it("appendEvent bypasses the batching window", () => {
    store.appendEvent(makeEvent({ id: "direct", kind: "tls" }));
    expect(store.events).toHaveLength(1);
    expect(store.stats.tls).toBe(1);
    expect(store.stats.total).toBe(1);
  });

  it("clearEvents empties the ring buffer and clears the selection", async () => {
    emit(sampleEvents[1] as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);
    store.select(store.events[0]);

    await store.clearEvents();

    expect(store.events).toHaveLength(0);
    expect(store.selectedEvent).toBeNull();
  });

  it("select() updates the selected event", () => {
    emit(sampleEvents[1] as TrafficEvent);
    vi.advanceTimersByTime(BATCH_FLUSH_MS);

    store.select(store.events[0]);
    expect(store.selectedEvent?.id).toBe("2");

    store.select(null);
    expect(store.selectedEvent).toBeNull();
  });
});
