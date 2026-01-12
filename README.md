# todo-cli

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://www.rust-lang.org/)

A terminal-based todo list manager with daily rolling lists, hierarchical tasks, and LLM integration.

<!-- ![TUI Screenshot](docs/screenshot.png)
*Terminal UI with vim-style navigation* -->

## Features

- **Terminal UI (TUI)** - Beautiful interface with vim-style keybindings
- **Daily Rolling Lists** - Automatic rollover of incomplete tasks to the next day
- **Hierarchical Todos** - Nest tasks under parent items with Tab/Shift+Tab
- **Multiple States** - `[ ]` pending, `[*]` in progress (animated spinner), `[x]` done, `[?]` question, `[!]` important
- **REST API** - HTTP server for external integrations
- **MCP Server** - Model Context Protocol support for LLM tools (Claude, etc.)
- **SQLite Archive** - Historical todos stored in a searchable database
- **Plugin System** - Generate todos from external sources (Jira integration included)

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) 1.85 or later

### From Source

```bash
# Clone the repository
git clone https://github.com/grimurjonsson/todo-cli.git
cd todo-cli

# Build and install
just install

```

### Using Cargo

```bash
cargo install --git https://github.com/grimurjonsson/todo-cli.git
```

## Usage

### Terminal UI

Simply run `todo` to launch the interactive terminal interface:

```bash
todo
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
todo add "Buy groceries"

# Show today's todos
todo show

# Show todos from a specific date (from archive)
todo show --date 2024-01-15
```

### API Server

The REST API runs automatically when you start the TUI, or you can manage it manually:

```bash
# Start the API server (default port: 48372)
todo serve start

# Check server status
todo serve status

# Stop the server
todo serve stop

# Use a different port
todo serve start --port 3000
```

API endpoints:
- `GET /api/todos` - List todos for a date
- `POST /api/todos` - Create a todo
- `PUT /api/todos/:id` - Update a todo
- `DELETE /api/todos/:id` - Delete a todo
- `POST /api/todos/:id/complete` - Toggle completion

### MCP Server (for LLMs)

The MCP server allows AI assistants like Claude to manage your todos.

#### Installation as Claude Code Plugin

**Recommended for Claude Code users:**

1. Open Claude Code
2. Type `/plugin` to open the plugin manager
3. Go to the "Installed" tab
4. Look for an option to add a plugin from URL (may be a button or text input)
5. Enter the GitHub URL: `https://github.com/grimurjonsson/todo-cli.git`
6. After installation, build the MCP server (one-time setup):
   ```bash
   # Find the installed plugin directory
   cd ~/.claude/plugins/repos/todo-mcp
   # Or if installed via cache:
   # cd ~/.claude/plugins/cache/*/todo-mcp/*

   # Build the binary
   cargo build --release --bin todo-mcp
   ```
7. Restart Claude Code

The MCP server will now be available in **all** Claude Code instances.

**One-time Build Requirement:**

After installation, you need to build the MCP server binary once because:
- The plugin is installed from source (Git repository)
- The Rust binary needs to be compiled for your system
- Future updates can reuse the built binary

**Updating the Plugin:**

When updates are available:
1. Update through the plugin UI
2. Rebuild the binary: `cd ~/.claude/plugins/cache/.../todo-mcp/... && cargo build --release --bin todo-mcp`
3. Restart Claude Code

#### Local Development Setup

For developing the plugin locally:

```bash
just setup-mcp-claude-dev
```

This creates a symlink from `~/.claude/plugins/repos/todo-mcp` to your project directory, allowing you to test changes without reinstalling.

#### Manual MCP Server Setup

For other LLM tools (e.g., Claude Desktop, OpenCode):

```bash
# Run the MCP server
cargo run --release --bin todo-mcp
```

Configure in your LLM tool:

```json
{
  "mcp": {
    "todo-mcp": {
      "command": ["/path/to/todo-mcp"],
      "enabled": true
    }
  }
}
```

### Generate Todos from External Sources

```bash
# List available generators
todo generate --list

# Generate todos from a Jira ticket (requires acli and claude CLI)
todo generate jira PROJ-123

# Auto-confirm adding generated todos
todo generate jira PROJ-123 --yes
```

## Configuration

Copy the example configuration to get started:

```bash
mkdir -p ~/.config/todo-cli
cp config.example.toml ~/.config/todo-cli/config.toml
```

The config file lets you customize:
- Theme
- Keybindings (fully remappable)
- Key sequence timeout

## Data Storage

- **Today's todos**: `~/.local/share/todo-cli/dailies/YYYY-MM-DD.md`
- **Archive database**: `~/.local/share/todo-cli/archive.db`
- **Configuration**: `~/.config/todo-cli/config.toml`

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
