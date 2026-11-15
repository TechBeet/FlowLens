# ADR 0001 — Use Tauri over Electron

**Status:** Accepted  
**Deciders:** FlowLens maintainers  
**Date:** 2024-01-01  

## Context

FlowLens is a desktop HTTP/HTTPS packet-capture and inspection tool (Tauri 2 + Svelte 5 + Rust workspace). The shell must:

- Run a lightweight webview UI while keeping native code (packet capture, TLS MITM, certificate store, Windows Npcap/Wpd) in Rust.
- Minimize runtime memory and CPU overhead, because capture pipelines are throughput-sensitive.
- Ship as a single small Windows installer with fast startup.
- Call Rust backend commands from Svelte via Tauri `invoke`, and emit backend events to the frontend via Tauri events (`traffic-event`, `capture-started`, `capture-stopped`).

The candidate desktop shells were **Electron** (bundles Chromium + Node.js) and **Tauri** (bundles the OS webview + a Rust core). The UX layer is Svelte 5 either way; the decision concerned only the shell / native bridge.

## Decision

Use **Tauri 2** as the desktop shell and application framework.

- The UI (`ui/`) is a Svelte 5 + Vite app served into the system webview.
- The Rust core lives in `crates/*` and `ui/src-tauri/`, with Tauri commands exposed via `tauri::generate_handler!` (see `ui/src-tauri/src/commands.rs`).
- Frontend ↔ backend communication uses Tauri `invoke` for commands and Tauri `listen`/`emit` for streaming traffic events.
- Distribution is handled by Tauri's bundler (`cargo tauri build`), producing a small Windows installer.

## Consequences

### Positive

- **Small installer / low memory**: The system webview (WebView2 on Windows) is shared with Edge/Chrome already present on most Windows machines, so FlowLens ships without bundling a full Chromium — far smaller than Electron and with a lower baseline memory footprint, important when holding a large event ring buffer.
- **Native Rust access**: Packet capture (Npcap/Wpd via `flowlens-capture`), TLS MITM (`flowlens-tls`), and Windows certificate-store operations (`cert_store.rs`) run natively in Rust with zero bridging tax. Tauri commands map cleanly to these.
- **Strong security defaults**: Tauri's CSP (see `tauri.conf.json`) restricts scripts/styles to `'self'`, reducing XSS surface versus Electron's default Node integration model.
- **Tauri v2 feature set**: Matches the existing `src-tauri` crate dependencies (`tauri = "2"`, `tauri-plugin-shell`, `tauri-plugin-dialog`), `npm run tauri dev`, and `beforeBuildCommand: "npm run build"`.

### Negative

- **WebView2 dependency**: On Windows, FlowLens requires Edge WebView2 runtime to be present (or bundled), adding an install-time dependency Electron avoids by shipping its own Chromium.
- **Platform-specific webview quirks**: CSS/JS behavior can differ slightly between Windows (WebView2), macOS (WKWebView), and Linux (WebKitGTK); the project currently targets Windows.
- **Smaller ecosystem/plugin surface**: Fewer third-party plugins than Electron; some features (e.g. auto-updater, native save dialogs) are handled via official Tauri plugins or small Rust wrappers rather than a large npm ecosystem.
- **Tauri version lock-in**: The codebase targets Tauri 2 specifically; upgrading to a future major version requires testing both Rust and frontend sides.

## Alternatives considered

| Alternative | Assessment |
|-------------|------------|
| **Electron** | Rejected: bundles Chromium + Node, increasing installer size and baseline memory; Node integration adds a larger attack surface; no benefit for a Rust-heavy native capture app. |
| **Tauri 1** | Rejected: older API; Tauri 2 is already adopted in `package.json` (`@tauri-apps/cli: ^2`, `tauri = "2"`), `tauri.conf.json`, and CI. |
| **WPF / WinForms / native Windows only** | Rejected: would abandon the cross-platform web UI, require rewriting the whole frontend, and conflict with the existing Svelte/Vite codebase. |
| **Flutter / Compose Multiplatform** | Rejected: new toolchain, no existing Rust→Dart bridge for the event stream, and no existing UI code to reuse. |
