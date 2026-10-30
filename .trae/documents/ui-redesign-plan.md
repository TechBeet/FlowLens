# FlowLens UI Redesign Plan

## Summary
Fix the broken FlowLens Tauri/Svelte UI by converting Svelte 4 components to Svelte 5 runes, ensuring CSS variable theming, and verifying the build. The project has two conflicting UIs (egui GUI vs Tauri/Svelte shell), but the plan focuses on fixing the Tauri/Svelte shell to work correctly.

## Current State Analysis

### Files Explored (10+ files):
- `ui/src/App.svelte` - Already uses Svelte 5 runes correctly
- `ui/src/main.ts` - Svelte mount point, imports global.css
- `ui/src/lib/api.ts` - Tauri command bindings (start/stop capture, get stats, list devices, export HAR, theme management)
- `ui/src/lib/types.ts` - TrafficEvent, TrafficStats interfaces matching Rust models
- `ui/src/styles/global.css` - Already has CSS variables for dark/light themes (--bg, --surface, --panel, --fg, --fg-dim, --accent, --border, --ok, --warn, --err, --hover, --selected, --row-alt)
- `ui/src/components/Header.svelte` - Already uses Svelte 5 runes, uses CSS variables
- `ui/src/components/Sidebar.svelte` - **BROKEN**: Uses Svelte 4 syntax (`export let`), hardcoded colors (#12121e, #222, #1a1a2a, etc.)
- `ui/src/components/TrafficTable.svelte` - **BROKEN**: Uses Svelte 4 syntax, hardcoded colors (#151520, #222, #1a1a2e, etc.)
- `ui/src/components/RightPanel.svelte` - **BROKEN**: Uses Svelte 4 syntax, hardcoded colors (#151520, #222, etc.)
- `ui/src-tauri/src/commands.rs` - Tauri commands with event emission (`traffic-event`)
- `ui/src-tauri/src/state.rs` - AppState with event_bus, stats, running flag
- `crates/flowlens-ui/Cargo.toml` - Removed egui/eframe dependencies
- `crates/flowlens-ui/src/lib.rs` - Removed GuiFrontend, kept TuiFrontend

### Key Issues Found:

| Component | Problem |
|-----------|---------|
| `Sidebar.svelte` | Uses Svelte 4 `export let events`, hardcoded hex colors, no CSS variables |
| `TrafficTable.svelte` | Uses Svelte 4 `export let events`, hardcoded hex colors, no CSS variables |
| `RightPanel.svelte` | Uses Svelte 4 `export let event`, hardcoded hex colors, no CSS variables |
| `App.svelte` | Already correct Svelte 5 syntax, properly wires components |
| `global.css` | Already has complete CSS variable system for dark/light themes |
| `api.ts` | Already has correct Tauri invoke commands |

### Data Flow:
1. Tauri backend (`commands.rs`) emits `traffic-event` with `UiEvent` data
2. `App.svelte` listens for `traffic-event` and updates `$state(events)`
3. `Sidebar`, `TrafficTable`, `RightPanel` receive events via `$props()`
4. CSS variables in `global.css` provide theming

## Proposed Changes

### Phase 1: Fix Svelte Components (already done in previous session)
1. **Sidebar.svelte** - Convert to Svelte 5 `$props()` and `$bindable`, use CSS variables
2. **TrafficTable.svelte** - Convert to Svelte 5 `$props()` and `$bindable`, use CSS variables
3. **RightPanel.svelte** - Convert to Svelte 5 `$props()` and `$bindable`, add tab switching, use CSS variables
4. **App.svelte** - Already correct, keep as-is

### Phase 2: Verify Build
1. `cargo check --workspace` - Verify Rust compilation
2. `npm run build` - Verify Svelte/Vite compilation
3. `npm run tauri dev` - Verify Tauri application starts

### Files to Modify:
- `ui/src/components/Sidebar.svelte` - Already fixed
- `ui/src/components/TrafficTable.svelte` - Already fixed
- `ui/src/components/RightPanel.svelte` - Already fixed
- `ui/src/styles/global.css` - Already has CSS variables
- No changes needed to App.svelte, api.ts, main.ts

### Assumptions:
- The Tauri backend already emits `traffic-event` events correctly (verified by successful compilation)
- The `App.svelte` component already properly wires up event listeners (verified by code structure)
- CSS variables in `global.css` are already defined and used by Header.svelte

### Verification Steps:
1. `cargo check --workspace` succeeds
2. `npm run build` succeeds
3. `npm run tauri dev` starts the application
4. UI displays traffic data from mock producer
5. No console errors in browser/devtools
6. Theme toggle works (dark/light)
7. All Svelte components use proper Svelte 5 runes

## Implementation History
- Sidebar.svelte: Converted to Svelte 5 `$props()`/`$bindable`, CSS variables added
- TrafficTable.svelte: Converted to Svelte 5 `$props()`/`$bindable`, CSS variables added
- RightPanel.svelte: Converted to Svelte 5 `$props()`/`$state`, tab switching added, CSS variables added
- egui GUI: Removed (deleted gui.rs, fonts.rs, updated Cargo.toml, lib.rs, main.rs)
- Build verified: cargo check + npm run build both pass

## Next Steps (if needed)
- Start Tauri dev server and verify UI displays traffic events
- Test theme toggle (dark/light)
- Test capture start/stop functionality
- Test filtering functionality