// FlowLens — reusable composable for subscribing to the Tauri event stream.
//
// Use this instead of inline `listen("traffic-event", ...)` calls spread across
// components. Keeps event-subscription logic in one place and surfaces a clean,
// synchronous cleanup handle (safe to call before the async listen resolves).
import { listen } from "@tauri-apps/api/event";
import type { TrafficEvent } from "../types";

/**
 * Subscribe to the Tauri "traffic-event" stream.
 *
 * Returns a synchronous cleanup function. It is safe to call immediately
 * (even before `listen()` resolves under the hood) — the underlying listener
 * is registered asynchronously and torn down as soon as it becomes available.
 */
export function subscribeTrafficEvents(
  handler: (ev: TrafficEvent) => void,
): () => void {
  let unlisten: (() => void) | undefined;
  let disposed = false;

  void listen<TrafficEvent>("traffic-event", (ev) => {
    handler(ev.payload);
  })
    .then((u) => {
      if (disposed) {
        // Cleanup already requested; tear the listener down right away.
        u();
      } else {
        unlisten = u;
      }
    })
    .catch((e) => {
      // Registration failures are non-fatal; surface via console only.
      console.error("[FlowLens] failed to subscribe to traffic-event:", e);
    });

  return () => {
    disposed = true;
    unlisten?.();
    unlisten = undefined;
  };
}
