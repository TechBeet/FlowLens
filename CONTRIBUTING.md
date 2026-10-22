# Contributing to FlowLens

First off, thank you for considering contributing to FlowLens! This document provides guidelines and processes for contributing.

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Npcap SDK](https://npcap.com/#download) (for Windows build with Npcap support)
- Git

### Building from Source

```bash
# Clone the repository
git clone https://github.com/your-org/FlowLens.git
cd FlowLens

# Build the workspace
cargo build --workspace

# Run tests
cargo test --workspace
```

### Development Setup

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/your-feature`
3. Make your changes
4. Run tests: `cargo test --workspace`
5. Commit with [conventional commits](https://www.conventionalcommits.org/):
   - `feat:` for new features
   - `fix:` for bug fixes
   - `docs:` for documentation changes
   - `style:` for formatting (no code change)
   - `refactor:` for code refactoring
   - `test:` for adding tests
   - `chore:` for maintenance tasks

## Architecture

FlowLens is organized as a Cargo workspace with the following crates:

| Crate | Description |
|-------|-------------|
| `flowlens-core` | Core domain models, event bus, session state machine |
| `flowlens-capture` | Npcap capture engine and packet parsing |
| `flowlens-driver` | Windows kernel driver interface |
| `flowlens-attribution` | Process attribution via ETW/IP Helper |
| `flowlens-tls` | TLS interception and decryption |
| `flowlens-protocol` | HTTP/2, WebSocket, gRPC protocol parsers |
| `flowlens-storage` | SQLite storage backend |
| `flowlens-export` | HAR, JSON, CSV export formats |
| `flowlens-mcp` | MCP server for AI integration |
| `flowlens-ui` | TUI (ratatui) and GUI (egui) interfaces |
| `flowlens-cli` | CLI argument parsing and command routing |
| `flowlens-installer` | Portable installer logic |
| `flowlens-integration` | Integration tests |

## Pull Request Process

1. Update documentation if you change behavior
2. Add tests for new functionality
3. Ensure all tests pass: `cargo test --workspace`
4. Run formatter: `cargo fmt --all`
5. Run clippy: `cargo clippy --workspace --all-targets -- -D warnings`
6. Update CHANGELOG.md if applicable
7. Get review from maintainers

## Coding Standards

### Rust Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting
- Run `cargo clippy` before submitting
- Keep functions focused and under 50 lines when possible

### Documentation

- Document all public APIs with rustdoc comments
- Include examples for complex functionality
- Use Chinese and English for public-facing documentation

### Testing

- Unit tests: Every public function should have at least one test
- Integration tests: Test end-to-end flows
- Use `#[cfg(test)]` for unit tests
- Mark flaky tests with `#[ignore]`

## Reporting Issues

Please use the issue templates when reporting bugs or requesting features. Include:

- Version number
- Operating system and version
- Steps to reproduce (for bugs)
- Expected vs actual behavior (for bugs)

## Security

If you discover a security vulnerability, please follow our [Security Policy](SECURITY.md). Do **not** create a public GitHub issue.

## License

By contributing to FlowLens, you agree that your contributions will be licensed under the MIT License.