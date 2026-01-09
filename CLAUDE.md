# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust-based terminal todo list manager with:
- **TUI** (Terminal UI) using ratatui with vim-style keybindings
- **REST API** for external integrations (default port: 48372)
- **MCP server** for LLM integration (Model Context Protocol)
- **SQLite archive** for historical todo storage
- **Plugin system** for generating todos from external sources (e.g., Jira)

## Build & Development Commands

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test <test_name>

# Run TUI in debug mode
cargo run

# Run TUI with debug logging
RUST_LOG=debug cargo run

# Format code
cargo fmt

# Lint (must pass with no warnings)
cargo clippy

# Run MCP server
cargo run --release --bin todo-mcp

# Run MCP server with debug logging
RUST_LOG=debug cargo run --bin todo-mcp
```

### Using Just

The project includes a justfile for common tasks:

```bash
just build                      # Build release binary
just test                       # Run all tests
just tui                        # Run the TUI
just install                    # Build and symlink to /usr/local/bin
just start-mcp-server           # Run MCP server (release)
just start-mcp-server-debug     # Run MCP server (debug)
just inspect-mcp                # Open MCP inspector
just configure-mcp-opencode     # Add to OpenCode config
```

## Architecture

### Module Structure

- **`src/main.rs`**: CLI entrypoint, command routing, and API server lifecycle management
- **`src/lib.rs`**: Public library interface (exports mcp, plugin, storage, todo, utils)
- **`src/bin/todo-mcp.rs`**: MCP server binary entrypoint

### Core Modules

- **`todo/`**: Core todo item logic
  - `item.rs`: TodoItem struct with UUID, content, state, indent_level, parent_id, etc.
  - `list.rs`: TodoList container with date and path
  - `state.rs`: TodoState enum (Empty, Done, Question, Important)
  - `hierarchy.rs`: Parent-child relationship operations

- **`storage/`**: Data persistence layer
  - `file.rs`: Writes TodoList to markdown files (`~/.local/share/todo-cli/dailies/YYYY-MM-DD.md`)
  - `markdown.rs`: Parses markdown format with checkbox states `[ ]`, `[x]`, `[?]`, `[!]`
  - `database.rs`: SQLite operations for `todos` and `archived_todos` tables
  - `rollover.rs`: Daily rollover logic (copies incomplete items to new date, archives old items)

- **`app/`**: TUI application state
  - `state.rs`: AppState with todo_list, cursor_position, mode, undo/redo history, plugin state
  - `mode.rs`: Mode enum (Normal, Insert, Delete, Plugin, etc.)
  - `event.rs`: User input event handling

- **`ui/`**: Terminal UI rendering (ratatui)
  - `components/todo_list.rs`: Main todo list widget
  - `components/status_bar.rs`: Bottom status bar
  - `theme.rs`: Color themes

- **`api/`**: REST API server (axum)
  - `routes.rs`: API route definitions
  - `handlers.rs`: HTTP request handlers
  - `models.rs`: API request/response models

- **`mcp/`**: Model Context Protocol server
  - `server.rs`: TodoMcpServer implementation with tool handlers
  - `schemas.rs`: JSON schema definitions for MCP tools
  - `errors.rs`: MCP-specific error types

- **`plugin/`**: External todo generator system
  - `generators/jira_claude.rs`: Jira ticket → todos using acli + Claude CLI
  - `subprocess.rs`: Plugin execution infrastructure

### Data Flow

1. **Startup**: Load today's todos from `~/.local/share/todo-cli/dailies/YYYY-MM-DD.md`
2. **Rollover check**: If opening for first time today, prompt to copy incomplete items from yesterday
3. **TUI loop**: User edits todos, changes saved to markdown file in real-time
4. **API server**: Auto-starts when TUI launches, provides HTTP access to same data
5. **Archival**: During rollover, old date's items move to `archived_todos` table for history

### State Management

- **TodoItem**: Individual todo with UUID, content, state, indent_level, parent_id
- **TodoList**: Collection of items with date and file path
- **AppState**: TUI state including cursor position, mode, undo history (max 50), plugin state
- **Undo/Redo**: Full TodoList snapshots stored in AppState history

### Database Schema

Two main tables:
- **`todos`**: Active items indexed by date
- **`archived_todos`**: Historical items indexed by original_date

Both use:
- UUID primary keys
- Soft deletes (deleted_at timestamp)
- Parent-child relationships (parent_id + indent_level)
- RFC3339 timestamps for created_at, updated_at, completed_at

See `DB_DESIGN.md` for full schema details.

## Code Style Requirements

- **No `#[allow(dead_code)]`**: Remove unused code instead
- **Use `anyhow::Result`** for error handling in functions that can fail
- **Add error context** with `.with_context(|| "descriptive message")`
- **Soft deletes**: Never hard-delete database records; set deleted_at timestamp
- **Query filtering**: All SELECT queries must include `WHERE deleted_at IS NULL`
- **Follow existing patterns**: Match the style in surrounding code

## Key Conventions

- **Todo states**: ` ` (empty), `x` (done), `?` (question), `!` (important)
- **Date format**: YYYY-MM-DD for storage, "Month DD, YYYY" for display
- **Timestamps**: RFC3339 format
- **Configuration**: `~/.config/todo-cli/config.toml`
- **Data directory**: `~/.local/share/todo-cli/`
- **Daily files**: `~/.local/share/todo-cli/dailies/YYYY-MM-DD.md`
- **Archive database**: `~/.local/share/todo-cli/archive.db`

## MCP Server Integration

The MCP server exposes tools for LLM interaction:
- `list_todos`: Get todos for a date with formatted markdown output
- `create_todo`: Add new todo (optionally nested under parent_id)
- `update_todo`: Modify content, state, due_date, or description
- `delete_todo`: Remove todo and all children
- `mark_complete`: Toggle done/pending state

Configure in Claude Desktop or OpenCode using `just configure-mcp-opencode`.

## Plugin System

Generators create todos from external sources. Built-in:
- **jira**: Fetches Jira ticket via acli, generates todos using Claude CLI

Plugins implement the `Generator` trait with `generate(&input) -> Vec<TodoItem>`.
