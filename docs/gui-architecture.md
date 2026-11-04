# FlowLens Native GUI Architecture

**Status:** Draft ‚Ä?analysis and planning only. No implementation code.
**Project:** FlowLens (Windows native proxyless HTTP/HTTPS packet capture / debug tool)
**Scope:** Convert the existing ratatui TUI (`crates/flowlens-ui`) into a native window GUI application ("common application window style").
**Date:** Planning phase.

> **This document is design documentation only.** It contains no Rust implementation code. Any code-like blocks are explicitly marked as illustrative pseudo-code and are not compilable.

---

## 1. Executive Summary / Recommendation

**Recommended framework:** `eframe` / `egui` (immediate-mode, pure-Rust GUI), delivered as a new workspace crate `crates/flowlens-gui` behind the existing `UiFrontend` trait abstraction.

**Critical offline finding:** NONE of the candidate native GUI frameworks (`egui`/`eframe`, `iced`, `slint`/`slint-build`, `tauri`, `druid`, `winit`, `tiny-skia`, `glow`, `wgpu`, `wry`, `webview`) are present in the local Cargo registry cache (`.cargo-home/registry/src`). Only `ratatui` (0.26.3) + `crossterm` (0.27) ‚Ä?the existing TUI stack ‚Ä?plus a handful of utility crates are available offline.

Therefore:
- The recommended `egui`/`eframe` path requires the crates to be fetched into the registry before an offline build can succeed.
- Until `egui`/`eframe` (and their transitive deps such as `winit`, `wgpu`, `glow`, `pistachio`, `epaint`, etc.) are available offline, the only buildable UI remains the existing `ratatui` TUI.
- The architecture below keeps the `UiFrontend` trait so the TUI and a future `egui` GUI can coexist behind the same `start --gui` entry point.

---

## 2. Current State Investigation

### 2.1 Entry point (`src/main.rs`) ‚Ä?`start --gui` path

The root binary crate parses `flowlens_cli::Cli` via clap. The `Start` command carries a `--gui`/`-g` flag (currently deprecated).

Relevant wiring (summarized, not quoted verbatim):

1. A single `BroadcastEventBus` (from `flowlens_core`) is created: `let bus = Arc::new(BroadcastEventBus::default());`.
2. Capture is started: real Npcap via `flowlens_capture::Engine::auto()`, falling back to `flowlens_capture::spawn_mock_producer(bus.clone())` when Npcap is unavailable or `--mock` is passed. The engine pump feeds packets into the bus (`spawn_engine_pump` / `pump_engine_into_bus`).
3. Frontend selection (lines ~160-171):
   - `if gui { ... } else { ... }` ‚Ä?both branches currently construct `flowlens_ui::TuiFrontend::new()` and call `tui.run(bus.clone() as Arc<dyn EventBus>)?;`.
   - The `--gui` path prints a deprecation warning ("the egui frontend was removed") and launches the TUI.

The `UiFrontend` trait (`crates/flowlens-ui/src/lib.rs`):

```rust
// Illustrative pseudo-code (not compilable implementation)
pub trait UiFrontend: Send + Sync {
    fn run(&self, bus: Arc<dyn EventBus>) -> anyhow::Result<()>;
    fn stop(&self) -> anyhow::Result<()>;
}
pub use tui::TuiFrontend;
```

`TuiFrontend` (`crates/flowlens-ui/src/tui.rs`) subscribes to the bus on a background thread (`blocking_recv`), accumulates `TrafficRow`s into a mutex-guarded `Vec`, and runs a `ratatui` render loop with `crossterm` raw mode / alternate screen. Keyboard handling (`q`, `j/k`, `f`, `d`, etc.) is inline in the render loop.

### 2.2 Core data the GUI consumes (`crates/flowlens-core`)

| Module | Public surface relevant to GUI |
|--------|-------------------------------|
## 3. Offline Crate Availability Findings

Local registry cache inspected: `d:\github\FlowLens\.cargo-home\registry\src\index.crates.io-1949cf8c6b5b557f\`

### 3.1 Crates present offline (relevant subset)

| Crate | Version | Notes |
|-------|---------|-------|
| ratatui | 0.26.3 | Existing TUI stack; features `all-widgets` |
| crossterm | 0.27.0 | Existing TUI terminal backend |
| bytemuck | 1.25.2 | GPU/buffer casting |
| ttf-parser | 0.25.1 | Font parsing |
| parking_lot | 0.12.5 | Locks |
| webbrowser | 1.2.4 | Open URLs |
| khronos-egl | 6.0.0 | EGL bindings (partial GL ecosystem) |
| libloading | 0.8.9 | Dynamic lib loading |
| serde / serde_json | 1 / 1.0.154 | Serialization |
| tokio | 1 (in workspace deps, not registry listing) | Async runtime |

Full offline registry listing: `anyhow`, `arrayvec`, `autocfg`, `bitflags`, `bit-vec`, `bytemuck`, `cfg_aliases`, `cfg-if`, `crc32fast`, `crossterm`, `crossterm_winapi`, `cursor-icon`, `document-features`, `either`, `equivalent`, `foldhash`, `hashbrown`, `immutable-chunkmap`, `indexmap`, `itertools`, `itoa`, `khronos-egl`, `libloading`, `litrs`, `log`, `memchr`, `moxcms`, `once_cell`, `parking_lot`, `pkg-config`, `proc-macro2`, `quote`, `ratatui`, `rustc-hash`, `rustversion`, `ryu`, `scopeguard`, `serde_json`, `stable_deref_trait`, `ttf-parser`, `unicode-ident`, `webbrowser`, `winapi-util`, `zmij`.

### 3.2 Candidate GUI frameworks ‚Ä?OFFLINE availability

| Framework | Crate(s) | Offline in registry? | Verdict |
|-----------|----------|----------------------|---------|
| egui / eframe | `egui`, `eframe`, `epaint`, `pistachio`, `winit`, `wgpu`, `glow`, ... | **NO** | Requires network fetch |
| iced | `iced`, `iced_graphics`, `iced_winit`, `iced_wgpu`, ... | **NO** | Requires network fetch |
| slint | `slint`, `slint-build`, `slint-platform`, ... | **NO** | Requires network fetch + build script |
| Tauri | `tauri`, `tauri-build`, `wry`, `webview2-com`, ... | **NO** (only in `ui/src-tauri/target`, not registry) | Requires network + Node toolchain |
| druid | `druid`, `druid-shell`, ... | **NO** | Requires network fetch |
| winit (standalone) | `winit` | **NO** | Requires network fetch |
| tiny-skia / glow / wgpu | `tiny_skia`, `glow`, `wgpu` | **NO** | Requires network fetch |
| native-windows-gui / winapi | `native-windows-gui`, `winapi`, `windows-sys` | **NO** (`winapi-util` only) | Requires network fetch |

**Conclusion:** With strict offline access, **no native-window GUI framework can be compiled**. The only UI-capable crates offline are `ratatui` + `crossterm` (the current TUI). A native GUI requires adding `egui`/`eframe` (recommended) and its transitive dependencies to the local registry cache once network access is available.

---
| `event_bus.rs` | `EventBus` trait (`publish`, `subscribe(filter) -> broadcast::Receiver<TrafficEvent>`, `subscriber_count`, `has_subscribers`, `is_closed`); `BroadcastEventBus` impl; `EventBusDiagnostics` (`event_count`, `latest_events(limit)`, `filter_list`). |
| `models.rs` | `TrafficEvent` enum (ConnectionEstablished/Closed, HttpRequest/Response, TlsHandshake, WebSocketFrame, SseEvent, GrpcCall, CaptureDropped, PacketReceived, ...); `TrafficRow` (flattened view with `from_event`); `TrafficFilter` (PIDs, hosts, ports, methods, paths, status ranges, protocols); `ConnectionKey`, `ProcessInfo`, `TrafficSession`, `SessionId`, `TrafficStats`, `ProtocolKind`, `CaptureSource`, `TlsVersion`, `HttpVersion`. |
| `filter.rs` | `FilterExpr` DSL (Method, Host, StatusCodeRange, PathGlob, And/Or/Not), `parse_filter`, `eval_filter(row, expr)`. |
| `session.rs` | `SessionManager` trait + `InMemorySessionManager` (create/update/get/persist/list sessions). |
| `intercept.rs` | `InterceptEngine` (breakpoint pause/release/modify, `PausePoint`, `BreakpointRule`). |

The GUI consumes events through `EventBus::subscribe(TrafficFilter)` and reads snapshots through `EventBusDiagnostics`. Session/intercept control flows through `SessionManager` and `InterceptEngine`.

### 2.3 Existing `ui/` folder (Svelte + Tauri shell) ‚Ä?reusable?

The `ui/` folder is a **separate** Svelte 5 + Tauri v2 + Vite web UI with its own Rust shell (`ui/src-tauri/`) that depends on `flowlens-core`, `flowlens-capture`, `flowlens-tls`, `flowlens-replay`, `tauri = "2"`, `tauri-plugin-shell`, `reqwest`, etc.
## 5. Public API Surface the GUI Consumes

From `flowlens-core` / `flowlens-capture` (unchanged; GUI is a consumer):

**Event ingestion**
- `flowlens_core::EventBus::subscribe(TrafficFilter) -> broadcast::Receiver<TrafficEvent>` ‚Ä?subscribe to live event stream.
- `flowlens_core::EventBus::publish(...)`, `subscriber_count()`, `has_subscribers()`.
- `flowlens_core::EventBusDiagnostics::latest_events(limit)`, `event_count()`, `filter_list()` ‚Ä?snapshot reads for initial paint / history panel.
- `flowlens_core::models::TrafficFilter` ‚Ä?PID, host, port, method, path, status-range, protocol predicates.
- `flowlens_core::models::TrafficEvent` / `TrafficRow` ‚Ä?domain events and flattened row view.

**Session / capture control**
- `flowlens_capture::Engine` (`start`, `stop`, `is_running`, `drain_packets`), `Engine::auto()`, `Engine::new_mock()`.
- `flowlens_capture::spawn_mock_producer(bus)`, `spawn_engine_pump(engine, bus)`.
- `flowlens_core::session::SessionManager` (`create_session`, `update_session`, `get_session`, `list_sessions`).
- `flowlens_core::intercept::InterceptEngine` (`pause`, `release`, `modify`, `add_rule`, `list_rules`, `is_paused`).
- `flowlens_core::models::TrafficStats` ‚Ä?per-protocol counters.

**GUI-side responsibilities (new crate only, not implemented here):**
- Subscribe on a background thread/task; convert `TrafficEvent` ‚Ü?GUI row model; apply `TrafficFilter`/`FilterExpr`; drive session/intercept commands; own window lifecycle.

---

## 6. Running Mock Data into the GUI

The existing mock path requires no Npcap and no admin:

1. `flowlens start --gui --mock` (or `--gui` with Npcap absent) ‚Ü?`spawn_mock_producer(bus.clone())` begins publishing synthetic `TrafficEvent`s to the bus.
2. GUI subscribes via `bus.subscribe(TrafficFilter::default())` and renders rows as they arrive.
3. For deterministic demos, a test helper can push canned `TrafficEvent::HttpRequest` / `HttpResponse` / `ConnectionEstablished` events directly onto a `BroadcastEventBus` and hand it to `GuiFrontend::run`.

This lets the GUI be developed and demonstrated without Npcap, admin rights, or real network traffic.

---

## 7. New Crate Layout (`crates/flowlens-gui/`)

Proposed addition to workspace members:

```
crates/flowlens-gui/
‚îú‚îÄ‚îÄ Cargo.toml              # name = "flowlens-gui"; deps: flowlens-core, flowlens-capture, tokio, tracing, eframe/egui (when fetched)
‚îú‚îÄ‚îÄ src/
‚î?  ‚îú‚îÄ‚îÄ lib.rs              # GuiFrontend struct + impl UiFrontend; re-exports
‚î?  ‚îú‚îÄ‚îÄ app.rs              # GuiApp state (event buffer, filters, selection, session/intercept commands)
‚î?  ‚îú‚îÄ‚îÄ view_events.rs      # Illustrative: event list/table view
‚î?  ‚îú‚îÄ‚îÄ view_session.rs     # Illustrative: session detail / inspector
‚î?  ‚îú‚îÄ‚îÄ view_controls.rs    # Illustrative: start/stop/device/CA/toolbar controls
## 9. Migration Strategy from ratatui TUI

1. **Keep `crates/flowlens-ui` + `TuiFrontend` intact.** It remains buildable offline (`ratatui`/`crossterm` are in cache). It is the fallback when `--gui` is requested but `egui`/`eframe` crates are not yet available offline.
2. **Add `crates/flowlens-gui`** implementing `UiFrontend` with `egui`/`eframe`. Do not delete `TuiFrontend`.
3. **Update `src/main.rs`** `start --gui` branch to construct `GuiFrontend::new()` and call `gui.run(bus)`; keep the `else`/TUI branch for non-GUI mode. Remove the deprecation warning once the egui frontend is wired.
4. **Add `flowlens-gui` to workspace `members`** in root `Cargo.toml`; add `eframe`/`egui` to its `Cargo.toml` once crates are present in the offline registry.
5. **Feature parity:** port TUI behaviors to GUI equivalents ‚Ä?real-time event list (`Table`), selection/inspector panel (`RightPanel`-like), filter input bar (`FilterExpr` parse + `eval_filter`), clear (`f`), debug/raw detail (`d`), session stats, start/stop, device selection, CA install/uninstall, export HAR, pause/release/modify, rewrite rules, replay.
6. **Deprecate/remove TUI only after** `egui` GUI reaches feature parity and is verified offline-capable.

---

## 10. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| No GUI crate available offline | **Blocks compilation** of any native GUI until `egui`/`eframe` + deps are added to registry cache | Confirm offline availability first; keep `ratatui` TUI as the working UI; treat native GUI as a post-network-fetch phase |
| egui immediate-mode full redraw on high event rates | CPU churn at thousands of events/sec | Cap visible rows (`MAX_EVENTS` pattern), prune buffer, virtualize/limit scrolling, coalesce rapid updates |
| `broadcast::Receiver` lag drops events under backpressure | Missing events in GUI | Size channel generously (`BroadcastEventBus::default()` capacity), read on dedicated background thread, keep UI read non-blocking |
| `EventBus` trait returns concrete `broadcast::Receiver<TrafficEvent>` | Tight coupling to tokio broadcast | Keep using the existing trait as-is; GUI depends on `flowlens-core` only |
| `eframe`/`winit` requires system OpenGL/DirectX on Windows | Rendering failures on headless/VM | Prefer `wgpu` default backend; document graphics driver requirement; offer TUI fallback |
| Tauri `ui/` shell drifts from new GUI | Two UIs diverge in features | Treat `ui/` as reference only; do not extend it; converge feature list in the new `flowlens-gui` |
| Admin/Npcap requirement for real capture | GUI unusable without elevation | Default to `--mock` for demo/development; surface capture-source status in the GUI status bar |

---

## 11. Phase Breakdown

**Phase 0 ‚Ä?Offline baseline (no network, no new code)**
- Document current TUI wiring and core API surface (this analysis). Confirm no GUI crate is offline-buildable. Keep `ratatui` TUI as the only working UI.

**Phase 1 ‚Ä?Register GUI crates offline (network required once)**
- Fetch `eframe`, `egui`, and transitive deps (`winit`, `wgpu`, `glow`, `epaint`, `pistachio`, `raw-window-handle`, etc.) into `.cargo-home/registry` so `cargo build --offline` succeeds.
- Add `crates/flowlens-gui` to workspace `members` and to root `Cargo.toml`.

**Phase 2 ‚Ä?`flowlens-gui` crate skeleton (documented only)**
- `GuiFrontend` implements `UiFrontend`; `eframe` native window; background thread subscribes to `bus`; renders event list from buffered rows. No implementation code in this planning artifact.

**Phase 3 ‚Ä?Feature parity**
- Filter bar, selection/inspector, stats, controls (start/stop/device/CA/export), session & intercept commands, mock-data demo path.

**Phase 4 ‚Ä?Wire `start --gui` & retire deprecation**
- `src/main.rs` launches `GuiFrontend` on `--gui`; TUI retained as fallback/offline option.

**Phase 5 ‚Ä?Deprecate TUI (optional)**
- Remove `TuiFrontend` only after GUI parity verified and `egui` confirmed offline-capable.

---

‚î?  ‚îú‚îÄ‚îÄ view_filter.rs      # Illustrative: filter input bar (parses filter.rs DSL)
‚î?  ‚îú‚îÄ‚îÄ event_adapter.rs    # Illustrative: TrafficEvent -> GUI row conversion
‚î?  ‚îî‚îÄ‚îÄ mock.rs             # #[cfg(test)] or demo helper to feed canned events
```

The crate registers a new `GuiFrontend` implementing `UiFrontend`, selected from `src/main.rs` when `--gui` is passed (replacing the current deprecation-fallback-to-TUI behavior).

---

## 8. Threading / Async Model

- **UI thread:** `eframe` runs its own main loop (native window). All widget drawing and input handling happen on the main/UI thread.
- **Event ingestion:** a background thread (or tokio task) calls `bus.subscribe(...)` and `blocking_recv` / `recv`, pushing flattened rows into a `Mutex`/`RwLock`-protected buffer owned by `GuiApp`. This mirrors the existing TUI pattern (`std::thread::spawn` + `blocking_recv`).
- **Capture runtime:** `tokio` multi-thread runtime (already used by `src/main.rs`) drives `Engine` pump / mock producer; the GUI does not need its own tokio runtime ‚Ä?it shares the one created in `main`.
- **Session/intercept commands:** invoked from UI button handlers; `InterceptEngine` methods are `async`, so they are awaited on a spawned tokio task or via `block_in_place` from the egui event handler.
- **No shared mutable egui state across threads:** egui `Context` is not `Sync`; input events are marshalled to the UI thread via `ctx.request_repaint()` and the buffer is read under lock during `update`.

---

Assessment:
- **Not directly reusable** for a pure-Rust native GUI: it requires a WebView2 webview, the Tauri runtime, Node.js/Vite toolchain, and a JS/TS frontend. It is not part of the `flowlens` workspace `members` list and is not wired into `src/main.rs`.
- **Reusable as design reference only:** the component layout (Header / Sidebar / TrafficTable / RightPanel / status bar), the `UiEvent` flattened shape, and the Tauri command set (`start_capture`, `stop_capture`, `get_stats`, `list_devices`, `export_har`, CA install/uninstall, session pause/release/modify, rewrite rules, replay, compare) capture the intended feature set. These map cleanly onto `egui` views.
- **Not available offline:** the `ui/src-tauri/target/release/deps` contains compiled `libtauri`, `libwry`, `libwebview2_com` rlibs, proving Tauri was built previously (with network), but the crates are **not** in `.cargo-home/registry/src`, so an offline `cargo build` of the Tauri shell would fail.

### 2.4 Workspace `Cargo.toml`

Workspace members:
`flowlens-core`, `flowlens-capture`, `flowlens-attribution`, `flowlens-tls`, `flowlens-protocol`, `flowlens-storage`, `flowlens-export`, `flowlens-mcp`, `flowlens-ui`, `flowlens-cli`, `flowlens-driver`, `flowlens-installer`, `flowlens-integration`, `flowlens-replay`.

Root `flowlens` binary depends on all of these plus `clap`, `tokio` (full), `tracing`, `tracing-subscriber`, `anyhow`.

---
---
## 4. Framework Comparison & Rationale

| Criterion | egui / eframe | iced | slint | Tauri (Svelte) | Win32/native-windows-gui |
|-----------|--------------|------|-------|----------------|--------------------------|
| Language | Pure Rust | Pure Rust | Rust + .slint markup | Rust + TS/Svelte | Rust + raw Win32 |
| Runtime / window | `winit` + `wgpu`/`glow` | `winit` + `wgpu` | Own platform abstraction | WebView2 + Node | Raw Win32 API |
| Offline build today | ‚ú?(not in cache) | ‚ú?| ‚ú?| ‚ú?| ‚ú?|
| Single binary, no Node | ‚ú?| ‚ú?| ‚ú?| ‚ù?(Node/Vite) | ‚ú?|
| Immediate vs retained | Immediate (egui) | Retained-ish | Declarative retained | Retained (web) | Imperative Win32 |
| Ease of table/list views | Moderate (egui `Table`/`ScrollArea`) | Good | Good | Excellent (web) | Hard (raw Win32) |
| Hot reload / iteration | Good | Moderate | Moderate | Good (Vite HMR) | Poor |
| Windows native look | Custom-drawn (not native themed) | Custom-drawn | Native-ish | Web-styled | True native |
| Integration with existing `UiFrontend` trait | Natural (new impl) | Natural | Natural | Requires separate binary | Natural |

**Rationale for egui/eframe:**
- Pure Rust, single native window, no Node.js, no WebView, no separate frontend repo ‚Ä?consistent with FlowLens being a "Windows native" tool.
- Immediate-mode maps well to a traffic-event stream: each frame redraws the table from the current in-memory event buffer, mirroring the existing TUI mental model.
- Minimal API surface to implement `UiFrontend::run` / `stop`.
- Most likely to be acceptable offline once fetched (no C toolchain beyond what Rust/MSVC already has).

**Rationale against Tauri for this task:** Tauri is already present in `ui/` but is a separate web stack requiring Node/Vite/WebView2; converting the TUI to Tauri would duplicate the existing shell rather than extend the Rust workspace cleanly, and it is not buildable offline.

---
