# ADR 0002 — Use Svelte 5 Runes

**Status:** Accepted  
**Deciders:** FlowLens maintainers  
**Date:** 2024-01-01  

## Context

FlowLens' frontend (`ui/`) is a Svelte app built with Vite. The existing codebase is written for Svelte 5 with `svelte: ^5` and `@sveltejs/vite-plugin-svelte: ^5` in `ui/package.json`. The UI manages:

- A live, high-frequency traffic event stream (`traffic-event` from Tauri).
- A ring-buffer event store with batching/backpressure (`ui/src/lib/stores/eventStore.svelte.ts`).
- Filter state (host/PID/process/method/status), capture running state, stats, theme, HAR export, rewrite rules, shortcuts, and SNI decrypt domains.

The decision was whether to write reactive UI state with **Svelte 5 runes** (`$state`, `$derived`, `$effect`, `$props`) versus the older Svelte 4 `export let` / reactive statements (`$:`) style.

## Decision

Use **Svelte 5 runes** throughout the frontend.

- Component props use `$props()` (typed `interface Props`).
- Reactive state uses `$state(...)` (e.g. `running`, `events`, `filterState`, `stats`, `selectedEvent`, `replayHistory`, `harPath`).
- Derived values use `$derived(...)` where needed (e.g. `filteredEvents`, `isEmpty`).
- Side effects use `$effect(...)` (e.g. subscribing to Tauri events, listening for keyboard shortcuts).
- Stores that need runes are named `*.svelte.ts` so the Vite Svelte plugin compiles the runes transform (see `eventStore.svelte.ts` and its `*.svelte.test.ts`).

## Consequences

### Positive

- **Fine-grained reactivity**: Only the DOM that actually depends on changed state re-renders, which matters for the high-frequency traffic table and live stats.
- **Less boilerplate**: No `export let` declarations, no `$: ` reactive statements, and no external writable/readable stores for local UI state.
- **TypeScript-friendly**: Runes work naturally with `lang="ts"` and typed props, matching `ui/tsconfig.json` and `npm run check` (`tsc --noEmit`).
- **Consistent codebase**: All components (`App.svelte`, `DetailPanel.svelte`, `SettingsView.svelte`, `FilterBar.svelte`, `TrafficTable.svelte`, etc.) use the same reactivity model, reducing cognitive load.

### Negative

- **Svelte 5 learning curve**: Team members familiar only with Svelte 4 need to learn runes syntax (`$state`, `$derived`, `$props`).
- **`.svelte.ts` naming convention**: Runes-enabled modules must use the `.svelte.ts` suffix, which is non-obvious and must be documented for new modules (see `eventStore.svelte.ts`).
- **Test configuration**: Vitest must compile `.svelte.ts` modules with the Svelte plugin (`vite-plugin-svelte`) so runes transform correctly in tests (see `ui/vitest.config.mts`).

## Alternatives considered

| Alternative | Assessment |
|-------------|------------|
| **Svelte 4 (`export let` / `$:`)** | Rejected: older reactivity model; the project already targets Svelte 5 in `package.json`, and runes provide better performance for the live event table. |
| **Svelte 5 legacy mode** | Rejected: mixing `export let` with runes creates inconsistency; legacy mode is a compatibility bridge, not a target state. |
| **React / Vue / Solid** | Rejected: would discard the existing Svelte codebase, components, and Vite plugin setup; no benefit justifying a rewrite. |
