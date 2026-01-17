# Technology Stack

**Analysis Date:** 2026-01-17

## Languages

**Primary:**
- Rust 2024 Edition - Entire codebase including TUI, API server, MCP server, and library

**Secondary:**
- Bash - Installation scripts and justfile recipes
- TOML - Configuration files
- Markdown - Todo storage format

## Runtime

**Environment:**
- Rust stable toolchain (tested on stable-aarch64-apple-darwin)
- Async runtime: Tokio (full features)

**Package Manager:**
- Cargo
- Lockfile: `Cargo.lock` present and committed

**Version:** 0.2.2

## Frameworks

**Core:**
- ratatui 0.30 - Terminal UI rendering
- crossterm 0.29 - Terminal input/output handling
- axum 0.8 - REST API HTTP server
- rmcp 0.12 - Model Context Protocol server (with `server` and `transport-io` features)

**Testing:**
- Built-in Rust test framework
- tempfile 3.13 - Test fixtures and temporary directories
- pretty_assertions 1.4 - Enhanced test assertion output

**Build/Dev:**
- just (justfile) - Task runner for common commands
- cross - Cross-compilation for multiple platforms

## Key Dependencies

**Critical:**
- tokio 1.x (full features) - Async runtime for API and MCP servers
- rusqlite 0.38 (bundled) - SQLite database with bundled SQLite library
- serde 1.0 (derive) - Serialization/deserialization
- serde_json 1.0 - JSON parsing for API and MCP
- anyhow 1.0 - Error handling with context

**UI/TUI:**
- ratatui 0.30 - TUI widgets and rendering
- crossterm 0.29 - Terminal manipulation, raw mode, mouse capture
- dialoguer 0.11 - Interactive prompts for CLI commands
- unicode-width 0.2 - Character width calculation for TUI layout

**Data:**
- chrono 0.4 (serde) - Date/time handling with RFC3339 timestamps
- uuid 1.11 (v4, serde) - UUID generation for todo item IDs
- toml 0.9 - Configuration file parsing
- pulldown-cmark 0.13 - Markdown parsing for todo files

**HTTP/Server:**
- tower-http 0.6 (cors, trace) - HTTP middleware for CORS and tracing
- tracing 0.1 - Structured logging
- tracing-subscriber 0.3 (env-filter) - Log filtering via RUST_LOG

**Filesystem:**
- dirs 6.0 - Cross-platform home directory detection
- notify 8.2 - File system watching for database changes

**MCP:**
- rmcp 0.12 - MCP protocol implementation
- schemars 1 - JSON Schema generation for MCP tool definitions

**CLI:**
- clap 4.5 (derive) - Command-line argument parsing

## Configuration

**Environment:**
- `RUST_LOG` - Controls log level (e.g., `debug`, `info`, `RUST_LOG=debug cargo run`)
- No other environment variables required

**Application Config:**
- Location: `~/.to-tui/config.toml`
- Format: TOML
- Key settings:
  - `theme` - Color theme selection (default: "default")
  - `timeoutlen` - Key sequence timeout in ms (default: 1000)
  - `keybindings` - Custom keybinding mappings

**Build:**
- `Cargo.toml` - Rust project configuration
- `justfile` - Task automation recipes

## Binaries Produced

**Main:**
- `totui` - TUI application and CLI (`src/main.rs`)
- `totui-mcp` - MCP server binary (`src/bin/totui-mcp.rs`)

**Library:**
- `to_tui` - Library crate exposing mcp, plugin, storage, todo, utils modules (`src/lib.rs`)

## Platform Requirements

**Development:**
- Rust stable toolchain (2024 edition)
- macOS, Linux, or Windows
- Optional: `just` for task running
- Optional: `cross` for cross-compilation

**Production:**
- No external runtime dependencies (SQLite bundled)
- Builds for:
  - x86_64-unknown-linux-gnu
  - aarch64-unknown-linux-gnu
  - x86_64-apple-darwin
  - aarch64-apple-darwin
  - x86_64-pc-windows-gnu

**CI/CD:**
- GitHub Actions (`.github/workflows/release.yml`)
- Triggered on version tags (v*)
- Uses `cross` for Linux builds, native cargo for macOS

## Data Storage

**Locations:**
- Config: `~/.to-tui/config.toml`
- Database: `~/.to-tui/todos.db`
- Daily files: `~/.to-tui/dailies/YYYY-MM-DD.md`
- Server PID: `~/.to-tui/server.pid`

**Formats:**
- Database: SQLite (bundled, no external dependency)
- Files: Markdown with checkbox syntax
- Timestamps: RFC3339
- Dates: YYYY-MM-DD

---

*Stack analysis: 2026-01-17*
