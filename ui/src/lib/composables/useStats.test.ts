// FlowLens — unit tests for the useStats composable.
import { describe, it, expect } from "vitest";
import { aggregateStats, addBatchStats, EMPTY_STATS } from "./useStats";
import { sampleEvents, makeEvent } from "../__fixtures__/events";

describe("useStats / aggregateStats", () => {
  it("returns EMPTY_STATS for empty input", () => {
    const result = aggregateStats([]);
    expect(result).toEqual(EMPTY_STATS);
  });

  it("counts every event in `total`", () => {
    const result = aggregateStats(sampleEvents);
    expect(result.total).toBe(6);
    expect(result.total).toBe(sampleEvents.length);
  });

  it("buckets events by protocol kind", () => {
    const result = aggregateStats(sampleEvents);
    // Fixture: 1 connection, 1 http, 1 tls, 1 sse, 1 websocket, 1 dropped
    expect(result.connections).toBe(2); // connection + dropped
    expect(result.http).toBe(1);
    expect(result.tls).toBe(1);
    expect(result.sse).toBe(1);
    expect(result.websocket).toBe(1);
  });

  it("buckets protocol counters sum to total minus unclassified", () => {
    const result = aggregateStats(sampleEvents);
    const sum =
      result.connections + result.http + result.tls + result.sse + result.websocket;
    // Every fixture kind is classified, so sum === total.
    expect(sum).toBe(result.total);
  });

  it("classifies 'ws' alias as websocket", () => {
    const result = aggregateStats([makeEvent({ id: "a", kind: "ws" })]);
    expect(result.websocket).toBe(1);
  });

  it("classifies 'closed' as a connection", () => {
    const result = aggregateStats([makeEvent({ id: "b", kind: "closed" })]);
    expect(result.connections).toBe(1);
  });

  it("ignores unknown kinds for per-protocol buckets but still counts total", () => {
    const result = aggregateStats([makeEvent({ id: "c", kind: "packet" })]);
    expect(result.total).toBe(1);
    expect(
      result.connections + result.http + result.tls + result.sse + result.websocket,
    ).toBe(0);
  });

  it("treats kind case-insensitively", () => {
    const result = aggregateStats([makeEvent({ id: "d", kind: "HTTP" })]);
    expect(result.http).toBe(1);
  });

  it("does not mutate the input array", () => {
    const before = sampleEvents.map((e) => e.id);
    aggregateStats(sampleEvents);
    expect(sampleEvents.map((e) => e.id)).toEqual(before);
  });
});

describe("useStats / addBatchStats", () => {
  it("accumulates counters across successive batches", () => {
    const stats = { ...EMPTY_STATS };
    const first = addBatchStats(stats, sampleEvents.slice(0, 3));
    expect(first.total).toBe(3);
    expect(first.connections).toBe(1);
    expect(first.http).toBe(1);
    expect(first.tls).toBe(1);

    const second = addBatchStats(first, sampleEvents.slice(3));
    expect(second.total).toBe(6);
    expect(second.sse).toBe(1);
    expect(second.websocket).toBe(1);
    // Counter from the first batch is preserved.
    expect(second.http).toBe(1);
  });

  it("handles an empty batch as a no-op", () => {
    const stats = { ...EMPTY_STATS, total: 5 };
    const result = addBatchStats(stats, []);
    expect(result.total).toBe(5);
  });

  it("returns the same object instance it mutated", () => {
    const stats = { ...EMPTY_STATS };
    const returned = addBatchStats(stats, [makeEvent({ id: "e", kind: "http" })]);
    expect(returned).toBe(stats);
  });
});