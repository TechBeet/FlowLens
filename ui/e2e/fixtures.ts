import type { Page } from '@playwright/test';

/**
 * FlowLens runs inside Tauri in production and calls backend commands via
 * `invoke(...)`. A plain Vite dev-server page has no Tauri runtime, so without
 * a stub every backend call throws synchronously at import time and the app
 * white-screens.
 *
 * The stub below installs the minimal `window.__TAURI_INTERNALS__` surface the
 * app depends on:
 *   - `invoke` rejects (graceful "backend unavailable"), matching how api.ts
 *     handles a missing backend (try/catch + fallbacks everywhere).
 *   - `transformCallback` returns a plain callback id so `listen(...)` works
 *     without a real IPC channel.
 */
export async function stubTauri(page: Page): Promise<void> {
  await page.addInitScript(() => {
    // The event system (used by App.svelte via `listen(...)`) needs a callback
    // registry. A simple incrementing id is enough for the webview.
    let cbId = 0;

    const internals = {
      // `invoke` rejects gracefully with a clear message ("backend unavailable").
      // Every caller tolerates this: api.ts wraps calls in try/catch and
      // App.svelte guards its `await listen(...)`, so the UI stays interactive.
      invoke: (cmd: string, _args?: unknown): Promise<never> =>
        Promise.reject(
          new Error(`[E2E] Tauri invoke("${cmd}") is not available in the browser test runtime`),
        ),

      // `listen()` resolves this to a stable id and later uses it to unregister.
      // Returning an id keeps `onMount` happy without opening a real channel.
      transformCallback: (callback: unknown): number => {
        cbId += 1;
        // Keep a reference so the callback is never GC'd before use.
        (globalThis as Record<string, unknown>)[`__tauri_cb_${cbId}`] = callback;
        return cbId;
      },
    };

    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = internals;
  });
}