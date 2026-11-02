# Phase B Task 3 — GUI Rebuild & SAZ Export: Analysis Report

> Scope: Read-only analysis. No `.rs` or `.svelte` source files were modified.

## 1. UI / Tauri shell status (`ui/`)

- `ui/package.json` (lines 4-7): scripts `dev`, `build`, `tauri`. Dependencies: `@tauri-apps/api ^2`, Svelte 5, Vite 6. Partially built web frontend.
- `ui/vite.config.mjs` (lines 1-21): standard Svelte+Vite config; port 3000; ignores `src-tauri/target`.
- `ui/src-tauri/Cargo.toml` (lines 1-22): Tauri v2 app `flowlens-tauri`; depends on `flowlens-core`, `flowlens-capture`, `flowlens-tls`, `flowlens-replay`, tokio, reqwest. Separate workspace (`[workspace]`).
- `ui/src-tauri/src/lib.rs` (lines 13-33): `invoke_handler` registers `start_capture`, `stop_capture`, `get_stats`, `list_devices`, `export_har`, `install_ca`, `uninstall_ca`, `check_ca_status`, `get_ca_pem`, `pause_session`, `release_session`, `modify_session`, `list_paused_sessions`, `add/remove/toggle/list_rewrite_rule`, `replay_request`, `compare_sessions`. **Note: `export_har` IS registered here** (line 18), contrary to `docs/ui_analysis_report.md` §3.1 which lists it as unregistered — the doc is stale; current code already wires it.
- `ui/src-tauri/src/commands.rs`: `export_har` (lines 372-454) builds a HAR archive from `TrafficEvent` rows and writes JSON to `output_path`. `get_stats` (lines 188-191 per report) returns `TrafficStats`. `list_devices` (lines 193-196) returns device list.
- Frontend:
  - `ui/src/App.svelte` (lines 45-53): `pollStats()` calls `api.getStats()` and updates `stats`. (lines 108-109): `statsInterval = window.setInterval(pollStats, 1000)` started on capture start. So stats polling IS wired in current code, but only while capturing; not on mount.
  - `ui/src/components/Sidebar.svelte` (lines 6, 17-29, 38): search box uses `bind:value={searchQuery}` and `filteredEvents` derived filter — search IS bound in current code. (Doc report is stale.)
  - `ui/src/components/RightPanel.svelte` (lines 9-58): tabs differentiate request/response headers and bodies via `formatHeaders`/`formatBody` using `event.requestHeaders` etc. — differentiated in current code.
  - `ui/src/components/Header.svelte` (lines 44-50): stats bar displays `stats.total/http/tls/websocket/sse`.
- Conclusion: Tauri shell is more complete than the analysis report suggests; remaining gaps: device selection not passed to backend, export format only HAR (no SAZ), no elevation/admin UX in UI, no mobile CA trust docs.

## 2. `crates/flowlens-ui/src/lib.rs` (lines 1-14)

- Only exports `TuiFrontend` (line 9) and `UiFrontend` trait (lines 11-14). egui GUI removed; README mentions "egui GUI removed". TUI (ratatui) is the functional native UI.

## 3. SAZ export status

- `crates/flowlens-export/src/saz.rs`: implements `SazImporter` (import only). No exporter. Lines 18-92 import raw `*_c.txt` / `*_s.txt` pairs from zip.
- `crates/flowlens-cli/src/lib.rs` (lines 66-77): `Export { format, query, out }` accepts `--format` (default `har`); comment says "har or saz".
- `src/main.rs` (lines 316-362): on `saz` format, currently writes HAR content to `out.with_extension("saz.har")` (lines 353-359) — **not a real SAZ archive**. This matches task description.
- `crates/flowlens-export/src/lib.rs`: only `HarExporter` and `SazImporter` traits; no `SazExporter` trait.

## 4. GUI status summary

- ratatui TUI: functional (live events, nav, filter, details).
- egui GUI: removed.
- Tauri/Svelte shell: largely wired (capture, stats polling, search, right-panel tabs, export_har registered); still partial (no SAZ export, no admin/elevation UX, mobile CA docs missing).

## 5. Remaining work & estimated effort

1. Real SAZ export: add `SazExporter` trait + implementation writing zip with `metadata.xml` + `raw/NN_c.txt`, `raw/NN_s.txt`, `raw/NN_m.xml` per Fiddler spec; wire into CLI `saz` branch and Tauri `export_saz` command. Effort: ~2-3 days.
2. Tauri wiring polish: pass selected device to start_capture, expose export format selector, stats polling on mount. Effort: ~1 day.
3. Admin/elevation UX: detect admin (already `is_admin`/`ensure_admin` in CLI), prompt UAC via `runas`/manifest; document Npcap prerequisite. Effort: ~1-2 days.
4. Mobile CA trust workflow docs + QR code generation for CA download URL. Effort: ~1 day.
5. README/docs sync. Effort: ~0.5 day.

Total estimated: ~5-7 days.

## Test counts

- `crates/flowlens-export`: 13 tests passing (har + saz import).
- No source files modified in this task.