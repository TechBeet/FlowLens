# ADR 0005 — PathProvider and Settings Persistence

**Status:** Accepted  
**Deciders:** FlowLens maintainers  
**Date:** 2024-01-01  

## Context

FlowLens persists a small amount of user configuration between sessions:

- **Theme** (`dark`/`light`) — `read_theme`/`write_theme` in `commands.rs`, stored in `flowlens_theme.txt` in `std::env::temp_dir()`.
- **Event ring-buffer max** — `get_event_max`/`set_event_max`, stored in `flowlens_event_max.txt` in `temp_dir()`.
- **Keyboard shortcut overrides** — `read_shortcuts`/`write_shortcuts`, stored as `shortcuts.json` in the Tauri **app data dir** (`app_data_dir()` via `tauri::Manager::path`), because shortcuts are per-user and should survive installs.
- **Session data / HAR exports / replay history** — runtime memory plus user-chosen export paths via `pick_save_path` (tauri-plugin-dialog).

The decision concerns which directory abstraction to use for resolving these paths and how to keep settings persistence consistent and testable.

## Decision

Use **Tauri's `path` manager (`AppHandle::path()` / `BaseDirectory`)** as the single source for resolving user settings directories, with explicit fallbacks documented per file:

- **App data dir** (`app_data_dir()`): user-specific, persists across installs — used for `shortcuts.json` (`shortcuts_file_path(app)` in `commands.rs`).
- **Temp dir** (`std::env::temp_dir()`): used for theme (`flowlens_theme.txt`) and event max (`flowlens_event_max.txt`) because these are small, non-critical preferences where OS temp semantics are acceptable.
- **User-chosen paths**: HAR export location is chosen at export time via the native save dialog (`pick_save_path`), not a fixed directory.

There is no separate `PathProvider` crate; path resolution is centralized in `ui/src-tauri/src/commands.rs` helper functions (`theme_file_path`, `event_max_file_path`, `shortcuts_file_path`). Frontend calls these through Tauri commands (`read_theme`, `write_theme`, `get_event_max`, `set_event_max`, `read_shortcuts`, `write_shortcuts`, `pick_save_path`).

## Consequences

### Positive

- **Correct per-user locations**: `app_data_dir()` follows platform conventions (Windows `%LOCALAPPDATA%`), so shortcuts and settings do not roam unintended and survive reinstall.
- **Single resolution point**: All settings paths are defined in one file (`commands.rs`), making it easy to audit, change, or test path handling.
- **Graceful degradation**: Theme/event-max in `temp_dir()` means settings are best-effort; if temp is cleared, the app falls back to defaults (`dark`, `10000`) without crashing.
- **No new dependency**: Uses Tauri's built-in `Manager::path`; no extra crate needed.

### Negative

- **Split persistence locations**: Theme/event max live in `temp_dir()` while shortcuts live in `app_data_dir()`; a user "clear all settings" action would need to know both locations (currently none exists).
- **Temp-dir volatility**: Theme/event max can be lost if the OS or a cleaner empties `%TEMP%`; acceptable for non-critical preferences but worth documenting.
- **Windows-focused**: `app_data_dir()` behaves differently on macOS/Linux; the project targets Windows, but cross-platform support would need path verification per OS.

## Alternatives considered

| Alternative | Assessment |
|-------------|------------|
| **Custom `PathProvider` crate** | Rejected: over-engineering for ~3 settings files; Tauri's `path` manager already resolves the correct directories. |
| **Everything in `app_data_dir()`** | Rejected for theme/event max: temp_dir is simpler and sufficient for non-critical prefs; app_data is reserved for user data (shortcuts) that must persist. |
| **Everything in `temp_dir()`** | Rejected: shortcuts would be lost on temp cleanup; they are user-customized and worth persisting in app data. |
| **`#[path]` / compile-time paths** | Rejected: settings must be user-writable at runtime, not compile-time constants. |
