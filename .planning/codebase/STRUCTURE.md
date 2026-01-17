# Codebase Structure

**Analysis Date:** 2026-01-17

## Directory Layout

```
to-tui/
├── src/
│   ├── main.rs              # CLI entrypoint, command routing, API server
│   ├── lib.rs               # Library exports for external crate usage
│   ├── cli.rs               # Clap command definitions
│   ├── config.rs            # Config loading from TOML
│   ├── bin/
│   │   └── totui-mcp.rs     # MCP server binary entrypoint
│   ├── todo/                # Core domain layer
│   │   ├── mod.rs           # Module exports
│   │   ├── item.rs          # TodoItem struct
│   │   ├── list.rs          # TodoList container
│   │   ├── state.rs         # TodoState enum
│   │   └── hierarchy.rs     # Parent-child operations
│   ├── storage/             # Persistence layer
│   │   ├── mod.rs           # Module exports
│   │   ├── database.rs      # SQLite operations
│   │   ├── file.rs          # File I/O coordination
│   │   ├── markdown.rs      # Markdown parsing/serialization
│   │   └── rollover.rs      # Daily rollover logic
│   ├── app/                 # TUI application state
│   │   ├── mod.rs           # Module exports
│   │   ├── state.rs         # AppState struct
│   │   ├── mode.rs          # Mode enum
│   │   └── event.rs         # Key/mouse event handling
│   ├── ui/                  # Terminal UI rendering
│   │   ├── mod.rs           # TUI main loop, overlays
│   │   ├── theme.rs         # Color theme system
│   │   └── components/
│   │       ├── mod.rs       # Component exports, overlay rendering
│   │       ├── todo_list.rs # Main todo list widget
│   │       └── status_bar.rs# Bottom status bar
│   ├── api/                 # REST API server
│   │   ├── mod.rs           # Module exports
│   │   ├── routes.rs        # Axum router setup
│   │   ├── handlers.rs      # HTTP request handlers
│   │   └── models.rs        # Request/response types
│   ├── mcp/                 # MCP server for LLM integration
│   │   ├── mod.rs           # Module exports
│   │   ├── server.rs        # Tool handlers
│   │   ├── schemas.rs       # JSON schema definitions
│   │   └── errors.rs        # MCP error types
│   ├── plugin/              # External todo generators
│   │   ├── mod.rs           # Generator trait, registry
│   │   ├── subprocess.rs    # Plugin execution
│   │   └── generators/
│   │       ├── mod.rs       # Generator exports
│   │       └── jira_claude.rs# Jira integration
│   ├── keybindings/
│   │   └── mod.rs           # Key parsing, action mapping, cache
│   └── utils/
│       ├── mod.rs           # Utility exports
│       ├── paths.rs         # Data/config path helpers
│       └── unicode.rs       # Unicode cursor navigation
├── bruno/                   # Bruno API testing collection
├── scripts/                 # Build/release scripts
├── skills/                  # MCP skill definitions
├── .github/workflows/       # CI/CD configuration
├── Cargo.toml               # Rust dependencies
├── justfile                 # Task runner commands
├── CLAUDE.md                # AI assistant instructions
└── DB_DESIGN.md             # Database schema documentation
```

## Directory Purposes

**`src/todo/`:**
- Purpose: Core domain entities and business logic
- Contains: TodoItem, TodoList, TodoState, hierarchy operations
- Key files: `item.rs` (entity), `list.rs` (aggregate), `hierarchy.rs` (tree ops)

**`src/storage/`:**
- Purpose: All data persistence operations
- Contains: Database access, file I/O, markdown parsing
- Key files: `database.rs` (SQLite), `file.rs` (coordination), `rollover.rs` (daily migration)

**`src/app/`:**
- Purpose: TUI application state and event handling
- Contains: AppState, Mode enum, keyboard/mouse handlers
- Key files: `state.rs` (all TUI state), `event.rs` (input handling)

**`src/ui/`:**
- Purpose: Terminal rendering with ratatui
- Contains: Main loop, components, theme system
- Key files: `mod.rs` (main loop), `components/todo_list.rs` (main widget)

**`src/api/`:**
- Purpose: REST API for external integrations
- Contains: Axum routes and handlers
- Key files: `routes.rs` (router), `handlers.rs` (CRUD operations)

**`src/mcp/`:**
- Purpose: Model Context Protocol server
- Contains: MCP tool implementations
- Key files: `server.rs` (tool handlers), `schemas.rs` (JSON schemas)

**`src/plugin/`:**
- Purpose: External todo generator system
- Contains: Generator trait, plugin registry
- Key files: `mod.rs` (trait + registry), `generators/jira_claude.rs` (Jira impl)

**`src/keybindings/`:**
- Purpose: Vim-style keybinding system
- Contains: Key parsing, action enum, binding cache
- Key files: `mod.rs` (all keybinding logic, ~785 lines)

**`src/utils/`:**
- Purpose: Shared utility functions
- Contains: Path helpers, unicode utilities
- Key files: `paths.rs` (data directory paths), `unicode.rs` (cursor movement)

## Key File Locations

**Entry Points:**
- `src/main.rs`: CLI and TUI entry, API server lifecycle
- `src/bin/totui-mcp.rs`: MCP server entry
- `src/lib.rs`: Library interface for external usage

**Configuration:**
- `src/config.rs`: Config struct and TOML loading
- `~/.to-tui/config.toml`: User config file (runtime)
- `Cargo.toml`: Rust dependencies and build config

**Core Logic:**
- `src/todo/item.rs`: TodoItem with all fields and methods
- `src/todo/hierarchy.rs`: Indent/outdent, move with children
- `src/app/event.rs`: All keyboard and mouse event handling
- `src/app/state.rs`: AppState with undo, selection, modes

**Storage:**
- `src/storage/database.rs`: SQLite CRUD operations
- `src/storage/file.rs`: Load/save coordination
- `src/storage/markdown.rs`: Markdown format parsing
- `~/.to-tui/todos.db`: SQLite database (runtime)
- `~/.to-tui/dailies/YYYY-MM-DD.md`: Daily markdown files (runtime)

**Testing:**
- Tests are co-located in source files as `#[cfg(test)] mod tests`
- Test utilities use `tempfile` crate for temporary directories

## Naming Conventions

**Files:**
- `snake_case.rs` for all Rust source files
- `mod.rs` for module exports
- `YYYY-MM-DD.md` for daily todo files

**Directories:**
- `snake_case/` for module directories
- `.planning/` for planning documents (dot-prefixed)

**Types:**
- `PascalCase` for structs, enums, traits: `TodoItem`, `AppState`, `TodoGenerator`
- State suffix for state structs: `AppState`, `PluginSubState`
- Response/Request suffix for API types: `TodoResponse`, `CreateTodoRequest`

**Functions:**
- `snake_case` for all functions
- `handle_*` prefix for event handlers: `handle_key_event`, `handle_navigate_mode`
- `get_*` prefix for getters: `get_daily_file_path`, `get_connection`
- `load_*/save_*` for I/O: `load_todo_list`, `save_todo_list`
- `*_item` suffix for single-item operations: `indent_item`, `toggle_current_item_state`

**Constants:**
- `SCREAMING_SNAKE_CASE`: `DEFAULT_API_PORT`, `MAX_UNDO_HISTORY`

## Where to Add New Code

**New Feature (e.g., tags, priorities):**
- Domain: Add fields to `src/todo/item.rs`, update `src/todo/list.rs` if needed
- Storage: Update `src/storage/database.rs` schema and queries
- Storage: Update `src/storage/markdown.rs` for file format
- UI: Update `src/ui/components/todo_list.rs` rendering
- API: Add handlers in `src/api/handlers.rs`, models in `src/api/models.rs`
- MCP: Update tool handlers in `src/mcp/server.rs`, schemas in `src/mcp/schemas.rs`
- Tests: Add tests in the same file's `#[cfg(test)]` module

**New TUI Mode:**
- Add variant to `src/app/mode.rs`
- Add handler function in `src/app/event.rs`
- Add dispatch in `handle_key_event()` match
- Add overlay rendering in `src/ui/components/mod.rs` if modal

**New API Endpoint:**
- Route: `src/api/routes.rs`
- Handler: `src/api/handlers.rs`
- Types: `src/api/models.rs`

**New MCP Tool:**
- Handler: `src/mcp/server.rs` with `#[tool(...)]` attribute
- Schema: `src/mcp/schemas.rs` for request type

**New Plugin/Generator:**
- Implementation: `src/plugin/generators/new_generator.rs`
- Export: Add to `src/plugin/generators/mod.rs`
- Register: Add to `register_builtin_generators()` in `src/plugin/mod.rs`

**Utilities:**
- Path-related: `src/utils/paths.rs`
- String/unicode: `src/utils/unicode.rs`
- New utility module: Create `src/utils/new_module.rs`, export from `src/utils/mod.rs`

**New Keybinding:**
- Add action to `Action` enum in `src/keybindings/mod.rs`
- Add default binding in `default_navigate_bindings()` or appropriate function
- Handle action in `execute_navigate_action()` in `src/app/event.rs`

## Special Directories

**`target/`:**
- Purpose: Cargo build output
- Generated: Yes (by cargo)
- Committed: No (in .gitignore)

**`.planning/`:**
- Purpose: Planning and analysis documents
- Generated: No (manually created)
- Committed: No (in .gitignore typically)

**`bruno/`:**
- Purpose: Bruno API testing collection
- Generated: No
- Committed: Yes

**`release-binaries/`:**
- Purpose: Built release binaries for distribution
- Generated: Yes (by build scripts)
- Committed: Selective

**`~/.to-tui/` (runtime):**
- Purpose: User data directory
- Contains: `config.toml`, `todos.db`, `dailies/`, `server.pid`
- Generated: Yes (at runtime)
- Committed: N/A (outside repo)

---

*Structure analysis: 2026-01-17*
