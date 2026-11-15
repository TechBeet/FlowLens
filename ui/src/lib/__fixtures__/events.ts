// FlowLens — test fixtures for the composables and eventStore.
import type { UiEvent, FilterState } from "../types";
import { DEFAULT_FILTER } from "../composables/useFilter";

export function makeEvent(partial: Partial<UiEvent> & { id: string }): UiEvent {
  return {
    time: "00:00:00.0",
    kind: "http",
    info: "",
    size: 0,
    ...partial,
  } as UiEvent;
}

export const sampleEvents: UiEvent[] = [
  makeEvent({
    id: "1",
    kind: "connection",
    size: 100,
    src_addr: "10.0.0.1:50000",
    dst_addr: "10.0.0.2:443",
    time: "00:00:00.0",
  }),
  makeEvent({
    id: "2",
    kind: "http",
    size: 200,
    method: "GET",
    url: "https://example.com/api",
    status: "200",
    process_name: "chrome",
    pid: 1234,
    time: "00:00:00.1",
    timestamp_micros: 100_000,
  }),
  makeEvent({
    id: "3",
    kind: "tls",
    size: 300,
    method: "POST",
    url: "https://secure.example.com/login",
    status: "302",
    process_name: "firefox",
    pid: 5678,
    time: "00:00:00.2",
    timestamp_micros: 200_000,
    sni: "secure.example.com",
  }),
  makeEvent({
    id: "4",
    kind: "sse",
    size: 400,
    method: "GET",
    url: "https://events.example.com/stream",
    status: "200",
    process_name: "curl",
    pid: 9999,
    time: "00:00:00.3",
    timestamp_micros: 300_000,
  }),
  makeEvent({
    id: "5",
    kind: "websocket",
    size: 500,
    method: "GET",
    url: "https://ws.example.com/socket",
    status: "101",
    process_name: "chrome",
    pid: 1234,
    time: "00:00:00.4",
    timestamp_micros: 400_000,
  }),
  makeEvent({
    id: "6",
    kind: "dropped",
    size: 0,
    time: "00:00:00.5",
    timestamp_micros: 500_000,
  }),
];

export function makeFilter(overrides: Partial<FilterState> = {}): FilterState {
  return { ...DEFAULT_FILTER, ...overrides };
}