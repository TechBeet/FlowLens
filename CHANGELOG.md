# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Light/dark theme toggle** in the native egui GUI (☀ / ☾ button in the header)
- **CJK font auto-detection** (`flowlens-ui::fonts`): loads Microsoft YaHei / PingFang /
  Noto CJK at runtime so Chinese UI text renders instead of showing tofu boxes
- **Theme-aware design system** in the GUI: a single `Palette` struct drives every
  surface, text, border and semantic colour in both themes
- **Row hover highlighting and row separators** in the GUI traffic table
- Real Npcap/WFP capture engine replacing mock producers
- `pump_engine_into_bus` function for draining packets into event bus
- Auto-detection of Npcap availability with graceful fallback
- Integration tests for real traffic capture
- Multi-platform build workflows (Windows, macOS, Linux)
- GitHub issue templates (bug report, feature request, build issue)
- CONTRIBUTING.md guide
- CODE_OF_CONDUCT.md
- SECURITY.md policy
- `.editorconfig` for consistent code formatting
- `.gitattributes` for cross-platform line endings
- FUNDING.yml for community support

### Fixed
- **GUI showed an empty table**: `run()` built a brand-new `GuiFrontend` inside the
  eframe creation closure, discarding the instance that had already subscribed to
  the event bus. The same instance is now reused.
- **Events published before any subscriber existed were silently dropped**:
  `spawn_mock_producer` / `spawn_engine_pump` now wait for the first subscriber
  before publishing, and only stop on a genuine channel-closed error.
- `.gitattributes` contained invalid directives (`root = true`, a `[Makefile]`
  section, and spaces around `=`), which made every `git` invocation emit
  `is not a valid attribute name` errors.
- Web UI sources (`types.ts`, `api.ts`, `global.css`) were stored as UTF-16LE,
  which breaks the Vite/esbuild pipeline. They are now UTF-8.
- Removed the duplicate/orphaned `ui/src/lib/App.svelte`.
- `GuiFrontend` now derives `Clone` and shares its buffers via `Arc`, so the
  background receiver and the renderer observe the same state.

### Changed
- `extract_connection_info_from` now returns `ConnectionKey` directly
- Event pipeline simplified through unified `EventBus` trait
- `EventBus::publish` now returns `Result<(), EventBusError>`; the receiver count
  is reported via logs instead of being conflated with an error.
- GUI render snapshot is refreshed only when the event count changes.
- CI workflow updated to use `dtolnay/rust-toolchain@stable`
- README now documents an honest feature-status matrix instead of claiming
  unimplemented capabilities (TLS MITM decryption, gRPC, WFP driver).

### Removed
- Debug artefacts that do not belong in a source repository: `check_out.txt`,
  `qa_test_output.txt`, `test_out.txt`, `show_window.ps1`, `FIX_SUMMARY.md`

### Security
- Documented security vulnerability reporting process

## [0.1.0] - 2026-01-01

### Added
- Initial release with core capture functionality
- WFP filter management
- Process attribution via ETW
- TLS interception support
- CLI with multiple command modes (--capture, --mcp, --wfp-install)
- TUI interface with ratatui
- GUI interface with egui
- MCP server for AI integration
- HAR export support
- SQLite storage backend