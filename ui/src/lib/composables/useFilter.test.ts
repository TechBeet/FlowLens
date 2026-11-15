// FlowLens — unit tests for the useFilter composable.
import { describe, it, expect } from "vitest";
import {
  filterEvents,
  searchEvents,
  sortEvents,
  isFilterActive,
  DEFAULT_FILTER,
} from "./useFilter";
import { sampleEvents, makeFilter } from "../__fixtures__/events";

describe("useFilter / filterEvents", () => {
  it("returns all events when no filter is set", () => {
    const result = filterEvents(sampleEvents, makeFilter());
    expect(result).toHaveLength(sampleEvents.length);
  });

  it("filters by host substring (case-insensitive)", () => {
    const result = filterEvents(sampleEvents, makeFilter({ host: "EXAMPLE.COM" }));
    // events 2,3,4,5 carry a URL whose host ends in example.com
    expect(result.map((e) => e.id).sort()).toEqual(["2", "3", "4", "5"]);
    for (const e of result) {
      expect((e.url ?? e.info).toLowerCase()).toContain("example.com");
    }
  });

  it("filters by pid (exact match)", () => {
    const result = filterEvents(sampleEvents, makeFilter({ pid: "1234" }));
    expect(result).toHaveLength(2);
    expect(result.every((e) => e.pid === 1234)).toBe(true);
  });

  it("filters by process name substring (case-insensitive)", () => {
    const result = filterEvents(sampleEvents, makeFilter({ process: "CHRO" }));
    expect(result).toHaveLength(2);
    expect(result.every((e) => (e.process_name ?? "").toLowerCase().includes("ch"))).toBe(true);
  });

  it("filters by method (case-insensitive, exact)", () => {
    const result = filterEvents(sampleEvents, makeFilter({ method: "get" }));
    expect(result).toHaveLength(3);
    expect(result.every((e) => (e.method ?? "").toLowerCase() === "get")).toBe(true);
  });

  it("filters by status (exact match)", () => {
    const result = filterEvents(sampleEvents, makeFilter({ status: "200" }));
    expect(result).toHaveLength(2);
    expect(result.every((e) => e.status === "200")).toBe(true);
  });

  it("combines multiple filter fields with AND semantics", () => {
    const result = filterEvents(sampleEvents, makeFilter({
      method: "GET",
      status: "200",
    }));
    // Events 2 and 4 are both GET + 200
    expect(result).toHaveLength(2);
    expect(result.map((e) => e.id).sort()).toEqual(["2", "4"]);
  });

  it("returns no events when filter matches nothing", () => {
    const result = filterEvents(sampleEvents, makeFilter({ method: "DELETE" }));
    expect(result).toHaveLength(0);
  });

  it("does not mutate the input array", () => {
    const before = [...sampleEvents];
    filterEvents(sampleEvents, makeFilter({ method: "GET" }));
    expect(sampleEvents).toEqual(before);
  });
});

describe("useFilter / searchEvents", () => {
  it("returns all events for empty query", () => {
    const result = searchEvents(sampleEvents, "");
    expect(result).toHaveLength(sampleEvents.length);
  });

  it("finds events by method keyword (case-insensitive)", () => {
    const result = searchEvents(sampleEvents, "get");
    expect(result).toHaveLength(3);
  });

  it("finds events by URL substring", () => {
    const result = searchEvents(sampleEvents, "login");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("3");
  });

  it("finds events by info substring", () => {
    // No events have a matching info, but empty-info events should not match
    const result = searchEvents(sampleEvents, "xyz");
    expect(result).toHaveLength(0);
  });

  it("trims surrounding whitespace from query", () => {
    const result = searchEvents(sampleEvents, "   get   ");
    expect(result).toHaveLength(3);
  });
});

describe("useFilter / sortEvents", () => {
  it("sorts by time ascending by default", () => {
    const sorted = sortEvents(sampleEvents);
    const ids = sorted.map((e) => e.id);
    expect(ids).toEqual(["1", "2", "3", "4", "5", "6"]);
  });

  it("sorts by time descending", () => {
    const sorted = sortEvents(sampleEvents, "time", "desc");
    const ids = sorted.map((e) => e.id);
    expect(ids).toEqual(["6", "5", "4", "3", "2", "1"]);
  });

  it("sorts by size ascending", () => {
    const sorted = sortEvents(sampleEvents, "size", "asc");
    expect(sorted.map((e) => e.size)).toEqual([0, 100, 200, 300, 400, 500]);
  });

  it("sorts by method alphabetically", () => {
    const sorted = sortEvents(sampleEvents, "method", "asc");
    const methods = sorted.map((e) => e.method ?? "");
    // empty strings sort first, then GET, POST
    expect(methods[0]).toBe("");
    expect(methods).toContain("GET");
    expect(methods).toContain("POST");
  });

  it("does not mutate the input array", () => {
    const before = sampleEvents.map((e) => e.id);
    sortEvents(sampleEvents, "size", "desc");
    expect(sampleEvents.map((e) => e.id)).toEqual(before);
  });

  it("falls back to numeric id for events without timestamp_micros", () => {
    const eventsWithoutTs = [
      { id: "10", time: "t1", kind: "http", info: "", size: 0 },
      { id: "2", time: "t2", kind: "http", info: "", size: 0 },
      { id: "33", time: "t3", kind: "http", info: "", size: 0 },
    ];
    const sorted = sortEvents(eventsWithoutTs, "time", "asc");
    // Should sort by id since timestamp_micros is missing
    expect(sorted.map((e) => e.id)).toEqual(["2", "10", "33"]);
  });
});

describe("useFilter / isFilterActive", () => {
  it("returns false for default filter", () => {
    expect(isFilterActive(DEFAULT_FILTER)).toBe(false);
  });

  it("returns true when any field is non-empty", () => {
    expect(isFilterActive(makeFilter({ host: "example.com" }))).toBe(true);
    expect(isFilterActive(makeFilter({ pid: "1234" }))).toBe(true);
    expect(isFilterActive(makeFilter({ method: "GET" }))).toBe(true);
  });

  it("returns false for empty string fields", () => {
    expect(isFilterActive({ host: "", pid: "", process: "", method: "", status: "" })).toBe(false);
  });
});