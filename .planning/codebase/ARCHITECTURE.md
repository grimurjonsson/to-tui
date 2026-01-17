# Architecture

**Analysis Date:** 2026-01-17

## Pattern Overview

**Overall:** Layered Architecture with Event-Driven TUI

**Key Characteristics:**
- Domain-driven core (`todo/`) with pure business logic
- Storage abstraction layer (`storage/`) handling dual persistence (markdown + SQLite)
- Multiple interface adapters (TUI, REST API, MCP server) sharing core domain
- Modal state machine for TUI interaction modes
- Vim-style keybinding system with customizable mappings

## Layers

**Domain Layer (`src/todo/`):**
- Purpose: Core business entities and logic
- Location: `src/todo/`
- Contains: `TodoItem`, `TodoList`, `TodoState`, hierarchy operations
- Depends on: External crates only (uuid, chrono)
- Used by: All other layers

**Storage Layer (`src/storage/`):**
- Purpose: Data persistence abstraction
- Location: `src/storage/`
- Contains: Database operations, file I/O, markdown parsing, rollover logic
- Depends on: Domain layer, rusqlite, file system
- Used by: App state, API handlers, MCP server

**Application State Layer (`src/app/`):**
- Purpose: TUI application state management
- Location: `src/app/`
- Contains: `AppState`, `Mode` enum, event handling
- Depends on: Domain layer, storage layer, keybindings
- Used by: UI layer

**UI Layer (`src/ui/`):**
- Purpose: Terminal rendering
- Location: `src/ui/`
- Contains: Ratatui components, theme system
- Depends on: App state layer
- Used by: Main TUI loop

**API Layer (`src/api/`):**
- Purpose: REST HTTP interface
- Location: `src/api/`
- Contains: Axum routes, handlers, request/response models
- Depends on: Domain layer, storage layer
- Used by: External HTTP clients

**MCP Layer (`src/mcp/`):**
- Purpose: Model Context Protocol for LLM integration
- Location: `src/mcp/`
- Contains: MCP server, tool handlers, schemas
- Depends on: Domain layer, storage layer, rmcp crate
- Used by: Claude Desktop, LLM tools

**Plugin Layer (`src/plugin/`):**
- Purpose: External todo generators
- Location: `src/plugin/`
- Contains: Generator trait, plugin registry, Jira integration
- Depends on: Domain layer
- Used by: App state, CLI

## Data Flow

**TUI Edit Flow:**

1. User presses key in terminal
2. `crossterm` captures key event
3. `handle_key_event()` in `src/app/event.rs` dispatches based on current `Mode`
4. `KeybindingCache` looks up action for key sequence
5. Action handler modifies `AppState.todo_list`
6. `save_undo()` pushes state to undo stack
7. `save_todo_list()` persists to both database and markdown file
8. Next render cycle draws updated state via `src/ui/components/`

**API Request Flow:**

1. HTTP request hits axum router (`src/api/routes.rs`)
2. Handler in `src/api/handlers.rs` parses request
3. `load_todo_list()` fetches from database (fallback to markdown)
4. Handler modifies `TodoList`
5. `save_todo_list()` persists changes
6. Response serialized and returned

**MCP Tool Flow:**

1. LLM sends tool call via stdio transport
2. `TodoMcpServer` receives request via rmcp
3. Tool handler (e.g., `list_todos`) processes request
4. Storage layer loads/saves data
5. Response returned with formatted output

**State Management:**
- TUI uses single `AppState` struct containing all UI state
- Undo history: vector of `(TodoList, cursor_position)` tuples, max 50 entries
- Mode-based state machine: Navigate -> Edit/Visual/Plugin/Rollover/ConfirmDelete
- Persistence: dual-write to SQLite database and markdown files

## Key Abstractions

**TodoItem (`src/todo/item.rs`):**
- Purpose: Single todo entry with all metadata
- Examples: `src/todo/item.rs:6-19`
- Pattern: Rich domain entity with behavior methods (`toggle_state`, `cycle_state`)

**TodoList (`src/todo/list.rs`):**
- Purpose: Collection of todos for a specific date
- Examples: `src/todo/list.rs:9-13`
- Pattern: Aggregate root with hierarchy operations

**TodoState (`src/todo/state.rs`):**
- Purpose: Enum representing checkbox state
- Examples: `src/todo/state.rs:4-10`
- Pattern: Value object with `Empty`, `Checked`, `Question`, `Exclamation`, `InProgress`

**Mode (`src/app/mode.rs`):**
- Purpose: Current TUI interaction mode
- Examples: `src/app/mode.rs:4-12`
- Pattern: State machine states: Navigate, Edit, Visual, ConfirmDelete, Plugin, Rollover

**AppState (`src/app/state.rs`):**
- Purpose: All TUI runtime state
- Examples: `src/app/state.rs:44-73`
- Pattern: God object containing todo_list, cursor, mode, undo stack, plugin state

## Entry Points

**Main Binary (`src/main.rs`):**
- Location: `src/main.rs:40`
- Triggers: CLI invocation as `totui`
- Responsibilities: Parse CLI args, load config, start TUI or handle subcommands

**MCP Binary (`src/bin/totui-mcp.rs`):**
- Location: `src/bin/totui-mcp.rs:11`
- Triggers: CLI invocation as `totui-mcp`
- Responsibilities: Start MCP server on stdio transport

**API Server (`src/main.rs:257`):**
- Location: `src/main.rs:257` (`run_server_foreground`)
- Triggers: `totui serve start` or auto-started by TUI
- Responsibilities: Serve REST API on port 48372

**Library (`src/lib.rs`):**
- Location: `src/lib.rs`
- Triggers: External crate usage
- Responsibilities: Export public modules (mcp, plugin, storage, todo, utils)

## Error Handling

**Strategy:** `anyhow::Result` with context

**Patterns:**
- All fallible functions return `anyhow::Result<T>`
- Error context added via `.with_context(|| "message")`
- MCP layer has dedicated `McpErrorDetail` type (`src/mcp/errors.rs`)
- API layer uses `ErrorResponse` helper for HTTP status codes
- Soft deletes for database records (set `deleted_at`, never hard delete)

## Cross-Cutting Concerns

**Logging:**
- Framework: `tracing` crate
- API/MCP servers use `tracing_subscriber` with env filter
- TUI mode: No logging by default, enabled via `RUST_LOG=debug`

**Validation:**
- Todo content validated at creation (non-empty)
- Date parsing uses chrono with explicit format
- UUID parsing for item IDs
- State parsing with defined valid characters

**Authentication:**
- None - local application
- API server binds to localhost only by default

**Database Access:**
- All queries filter `WHERE deleted_at IS NULL`
- Soft deletes via `soft_delete_todos()` function
- Dual persistence: SQLite database + markdown files

---

*Architecture analysis: 2026-01-17*
