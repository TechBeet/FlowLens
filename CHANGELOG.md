# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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

### Changed
- `extract_connection_info_from` now returns `ConnectionKey` directly
- Event pipeline simplified through unified `EventBus` trait
- CI workflow updated to use `dtolnay/rust-toolchain@stable`

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