# ADR 0003 — Event Bus and Batched UI Updates

**Status:** Accepted  
**Deciders:** FlowLens maintainers  
**Date:** 2024-01-01  

## Context

The capture backend emits traffic events continuously (one event per HTTP/TLS/packet/etc.) through Tauri `app_handle.emit("traffic-event", ...)` in `ui/src-tauri/src/commands.rs`. On the frontend, `ui/src/lib/composables/eventSubscriber.ts` subscribes to the `traffic-event` stream via Tauri `listen`.

If the UI committed each event to the DOM immediately, a busy capture would:

- Flood the frontend with individual reactive updates, causing dropped frames in the traffic table and stats panel.
- Exceed the ring-buffer cap and consume unbounded memory while the buffer grows.
- Starve the main thread with re-renders, making the UI unresponsive.

The decision concerns how to ingest, buffer, flush, and cap events between the Tauri event bus and the Svelte UI.

## Decision

**Ingest → coalesce buffer → timed flush → ring-buffer append → single stats pass**, with hard backpressure.

- **Ingest**: `subscribeTrafficEvents` (in `eventSubscriber.ts`) receives each `traffic-event` and hands it to `eventStore.ingest(ev)`.
- **Batching**: `createEventStore` (`ui/src/lib/stores/eventStore.svelte.ts`) buffers incoming events in `eventBuffer` and schedules a single flush after `BATCH_FLUSH_MS` (50 ms). All events arriving within the window are committed in one batch.
- **Commit**: `flush()` appends the whole batch to the reactive `events` ring buffer in one operation, then folds the batch into `stats` via `addBatchStats` in a single pass. This makes the UI update once per batch instead of once per event.
- **Ring buffer**: `events` is capped at `MAX_EVENTS` (default 10,000, persisted via `get_event_max`/`set_event_max` in `event_max.txt`); overflow drops the oldest entries.
- **Backpressure**: `MAX_PENDING` (5,000) caps the pre-flush buffer; once full, the oldest pending events are dropped and counted in `droppedEvents`.
- **Disposal**: `dispose()` clears the flush timer, unsubscribes from the Tauri event stream, and empties the buffer so no events leak after navigation or teardown.

## Consequences

### Positive

- **Bounded memory**: The ring buffer and pending buffer have hard caps, so long captures cannot grow memory without limit.
- **Stable UI frame rate**: One batched DOM commit per ~50 ms window keeps the traffic table and stats panel responsive under high event rates.
- **Backpressure safety**: `MAX_PENDING` with oldest-first dropping prevents buffer exhaustion when the backend produces faster than the UI drains.
- **Clean lifecycle**: `dispose()` guarantees listener cleanup, avoiding dangling Tauri subscriptions.

### Negative

- **Small latency**: Events are not shown the instant they arrive; they appear after up to `BATCH_FLUSH_MS` (50 ms). Acceptable for a capture/inspection tool.
- **Added complexity**: Buffering, flushing, ring-buffer overflow, and backpressure logic must be maintained and tested (`eventStore.svelte.test.ts` covers batching, coalescing, stats aggregation, flush, and disposal).
- **Potential event reorder within batch**: None in practice — events are appended in arrival order within a batch.

## Alternatives considered

| Alternative | Assessment |
|-------------|------------|
| **Commit each event immediately** | Rejected: causes per-event reactive updates and re-renders; too expensive at high capture rates. |
| **Larger fixed batch window** | Rejected: increases latency; 50 ms balances throughput and responsiveness. |
| **Unbounded buffer / no cap** | Rejected: risks unbounded memory growth during long captures. |
| **External state store (Redux/Pinia)** | Rejected: Svelte 5 runes + local `$state` suffice; an external store adds indirection without solving the batching problem. |
