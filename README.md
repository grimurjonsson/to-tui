# to-tui

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.92+-orange.svg)](https://www.rust-lang.org/)

A terminal-based todo list manager with daily rolling lists, hierarchical tasks, and LLM integration.

<!-- ![TUI Screenshot](docs/screenshot.png)
*Terminal UI with vim-style navigation* -->

## Features

- **Terminal UI (TUI)** - Beautiful interface with vim-style keybindings
- **Daily Rolling Lists** - Automatic rollover of incomplete tasks to the next day
- **Hierarchical Todos** - Nest tasks under parent items with Tab/Shift+Tab
- **Multiple States** - `[ ]` pending, `[*]` in progress (animated spinner), `[x]` done, `[?]` question, `[!]` important
- **Web workspace** - Responsive local editor with cross-process live updates (`totui web --open`)
- **REST API** - HTTP server for external integrations
- **MCP Server** - Model Context Protocol support for LLM tools (Claude, etc.)
- **SQLite Archive** - Historical todos stored in a searchable database
- **Plugin System** - Generate todos from external sources (Jira integration included)

## Installation

Run this in your terminal to download and install pre-built binaries:

```bash
curl -fsSL https://raw.githubusercontent.com/grimurjonsson/to-tui/main/scripts/install.sh | bash
```

The installer will prompt you to choose what to install:
- **totui** - The terminal UI app
- **totui-mcp** - MCP server for Claude/LLM integration
- **Both** - Install both binaries

## Usage

### Terminal UI

Simply run `totui` to launch the interactive terminal interface:

```bash
totui
```

#### Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `n` | New todo |
| `i` | Edit todo |
| `x` | Toggle done |
| `Space` | Cycle state (empty → in progress → done → question → important) |
| `Tab` | Indent (make child) |
| `Shift+Tab` | Outdent (make parent) |
| `dd` | Delete |
| `c` | Collapse/expand children |
| `<` / `>` | Previous / next day |
| `T` | Go to today |
| `?` | Show help |
| `q` | Quit |

### Command Line

```bash
# Add a todo without opening the TUI
totui add "Buy groceries"

# Show today's todos
totui show

# Show todos from a specific date (from archive)
totui show --date 2024-01-15
```

### API Server

The REST API/web workspace runs automatically when you start the TUI. Press **w**
or click the **w web-ui** footer control to start, stop, restart, or open it in a browser.
The indicator updates when the server changes, including through the CLI.
You can also manage the server manually:

```bash
# Start the API server (default port: 48372)
totui web start

# Check server status
totui web status

# Stop the server
totui web stop

# Use a different port
totui web start --port 3000
```

API endpoints:
- `GET /api/todos` - List todos for a date
- `POST /api/todos` - Create a todo
- `PUT /api/todos/:id` - Update a todo
- `DELETE /api/todos/:id` - Delete a todo
- `POST /api/todos/:id/complete` - Toggle completion

### Skills and CLI/API (for LLMs)

The bundled skills use `totui todo` and the selected server API. Run from your
project folder:

```bash
totui todo context
totui todo list
totui todo create --content "Review the change"
```

`context` reports the backend and folder-selected project. Skills announce that
destination before creating todos and confirm it afterward. Pin a destination with
`--remote home` and `--project NAME`; use `--local` only for intentional local work.
The CLI keeps JSON on stdout and prints mutation destinations on stderr.

### Legacy MCP server (local data only)

The standalone MCP server manages local todos. It does not follow a selected remote
backend; use the skills/CLI path above for remote workspaces.

Add totui-mcp to your Claude Code configuration:

```bash
# User-scoped (available in all projects)
claude mcp add --transport stdio --scope user totui-mcp -- /usr/local/bin/totui-mcp

# Project-scoped (creates .mcp.json in project)
claude mcp add --transport stdio --scope project totui-mcp -- /usr/local/bin/totui-mcp
```

Verify installation:
```bash
claude mcp list
```

Or check in Claude Code:
```
/mcp
```

For Codex CLI:

```bash
codex mcp add totui-mcp -- /usr/local/bin/totui-mcp
codex mcp list
```

From a checkout, `just configure-mcp-codex` does the same against the freshly built binary, and `just install-codex-skills` symlinks the bundled skills into `~/.codex/skills`.

For other LLM tools, add to your MCP configuration file:

```json
{
  "totui-mcp": {
    "command": "/usr/local/bin/totui-mcp",
    "args": []
  }
}
```

### Generate Todos from External Sources

```bash
# List available generators
totui generate --list

# Generate todos from a Jira ticket (requires acli and claude CLI)
totui generate jira PROJ-123

# Auto-confirm adding generated todos
totui generate jira PROJ-123 --yes
```

## Configuration

Copy the example configuration to get started:

```bash
mkdir -p ~/.config/to-tui
cp config.example.toml ~/.config/to-tui/config.toml
```

The config file lets you customize:
- Theme
- Keybindings (fully remappable)
- Key sequence timeout

## Data Storage

- **Today's todos**: `~/.local/share/to-tui/dailies/YYYY-MM-DD.md`
- **Archive database**: `~/.local/share/to-tui/archive.db`
- **Configuration**: `~/.config/to-tui/config.toml`

## Development

```bash
# Run tests
cargo test

# Run the TUI in debug mode
cargo run

# Run with debug logging
RUST_LOG=debug cargo run

# Format code
cargo fmt

# Lint
cargo clippy
```

### Using Just

If you have [just](https://github.com/casey/just) installed:

```bash
just          # List available commands
just build    # Build release binary
just test     # Run tests
just tui      # Run the TUI
just install  # Build and install to /usr/local/bin
```

## Contributing

Contributions are welcome! Here's how to get started:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Run lints (`cargo clippy` - fix all warnings)
6. Format code (`cargo fmt`)
7. Commit your changes (`git commit -m 'Add amazing feature'`)
8. Push to your branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

### Code Style

- No `#[allow(dead_code)]` - remove unused code
- Use `anyhow::Result` for error handling
- Add context to errors with `.with_context()`
- Follow existing patterns in the codebase

## License

MIT License - see [LICENSE](LICENSE) for details.

## Local web interface

For a persistent Linux VPS service, use `totui server install` (or
`totui server wizard`). It installs a native systemd service with persistent
SQLite storage, boot startup, failure recovery, and journal logs. See the
[Linux server installation guide](docs/server.md) for setup behind an existing
authenticated reverse proxy, per-user workspaces with `--auth`, upgrades, and backups.

To open a remote workspace in the TUI, upgrade both installations, then run
`totui remote add home https://totui.gimmi.is`, `totui remote login home`, and
`totui remote use home`. See [remote client setup](docs/server.md#connect-a-desktop-client)
for browser sign-in and switching back to local storage.

Run `totui web --open` (or `cargo run --bin totui -- web --open`) to use the
responsive task workspace at <http://127.0.0.1:48372>. The Rust binary bundles the
frontend; no Node runtime is required. `totui serve start` also serves the web UI.
Both commands now bind to loopback by default; `TOTUI_BIND` overrides the interface.

See [Web startup, development, synchronization, and verification](docs/web.md) for
isolated data setup, browser tests, concurrency behavior, measured latency, and
future HTTPS/OAuth hosting notes.
