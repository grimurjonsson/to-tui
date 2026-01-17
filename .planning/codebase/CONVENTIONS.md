# Coding Conventions

**Analysis Date:** 2026-01-17

## Naming Patterns

**Files:**
- Module files: `snake_case.rs` (e.g., `todo_list.rs`, `file.rs`)
- Mod files: `mod.rs` in directories or `{module_name}.rs` at parent level
- Binary entrypoints: `main.rs`, `bin/{name}.rs`

**Functions:**
- snake_case: `load_todo_list()`, `save_undo()`, `handle_key_event()`
- Verb-first naming: `get_*`, `load_*`, `save_*`, `handle_*`, `parse_*`, `create_*`
- Boolean getters: `is_*`, `has_*`, `can_*` (e.g., `is_complete()`, `has_children()`)

**Variables:**
- snake_case: `cursor_position`, `todo_list`, `edit_buffer`
- Boolean flags: `should_quit`, `is_creating_new_item`, `unsaved_changes`
- Iterators/indices: `idx`, `i`, `j` for loops; descriptive names for complex logic

**Types:**
- Structs: PascalCase (e.g., `TodoItem`, `AppState`, `TodoMcpServer`)
- Enums: PascalCase with PascalCase variants (e.g., `TodoState::Checked`, `Mode::Navigate`)
- Traits: PascalCase (e.g., `IntoMcpError`)
- Type aliases: PascalCase

**Constants:**
- SCREAMING_SNAKE_CASE: `MAX_UNDO_HISTORY`, `DEFAULT_API_PORT`

## Code Style

**Formatting:**
- Default `rustfmt` (no custom `.rustfmt.toml`)
- 4-space indentation (Rust default)
- Max line width: 100 characters (rustfmt default)

**Linting:**
- `cargo clippy` must pass with no warnings
- No `#[allow(dead_code)]` - remove unused code instead
- Unused imports and variables generate warnings

## Import Organization

**Order:**
1. Standard library (`std::*`)
2. External crates alphabetically
3. Crate-internal (`crate::*`, `super::*`)

**Example from `src/main.rs`:**
```rust
use anyhow::{Result, anyhow};
use chrono::Local;
use clap::Parser;
use cli::{Cli, Commands, DEFAULT_API_PORT, ServeCommand};
use config::Config;
use keybindings::KeybindingCache;
use std::env;
use std::fs;
use std::io::{Read, Write};
```

**Path Aliases:**
- `crate::` for absolute paths within the crate
- `super::` for parent module
- Direct module names when in scope via `use`

## Error Handling

**Patterns:**
- Use `anyhow::Result` for fallible functions
- Add context with `.with_context(|| "message")` for meaningful errors
- Use `?` operator for propagation

**Example from `src/storage/file.rs`:**
```rust
pub fn load_todo_list(date: NaiveDate) -> Result<TodoList> {
    ensure_directories_exist()?;
    database::init_database()?;

    let file_path = get_daily_file_path(date)?;

    let content = fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    let list = parse_todo_list(&content, date, file_path.clone())
        .with_context(|| "Failed to parse todo list")?;

    Ok(list)
}
```

**Error Construction:**
- `anyhow!("message")` for simple errors
- Custom error types for domain-specific errors (e.g., `McpErrorDetail`)
- `Err(anyhow!("Index out of bounds"))` for validation failures

**API/MCP Error Handling:**
- `ErrorResponse::internal(e)` for 500 errors
- `ErrorResponse::not_found("message")` for 404 errors
- `ErrorResponse::bad_request("message")` for 400 errors
- `McpErrorDetail` struct with code, message, retryable flag, suggestion

## Logging

**Framework:** `tracing` crate

**Patterns:**
```rust
use tracing::{debug, error, info, warn};

info!(date = ?params.0.date, "list_todos called");
info!(id = %response.id, content = %response.content, "create_todo completed");
warn!(code = %detail.code, message = %detail.message, "Retryable error occurred");
error!(code = %detail.code, message = %detail.message, "Non-retryable error occurred");
```

**When to Log:**
- Info: API calls, significant operations
- Debug: Detailed operation flow
- Warn: Recoverable issues
- Error: Non-recoverable issues

## Comments

**When to Comment:**
- Non-obvious business logic
- Public API documentation
- Complex algorithms

**Documentation Style:**
```rust
/// Parse an RFC3339 timestamp string into a DateTime<Utc>
fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {

/// Returns the set of indices that should be hidden due to collapsed parents
pub fn build_hidden_indices(&self) -> HashSet<usize> {

/// Find the insert position for a new child under a parent.
/// Returns (indent_level, insert_index) for the new child, or None if parent not found.
pub fn find_insert_position_for_child(&self, parent_id: uuid::Uuid) -> Option<(usize, usize)> {
```

**Inline Comments:**
- Use `//` for single-line explanations
- Explain "why" not "what"

## Function Design

**Size:**
- Prefer small, focused functions
- Extract helper functions for complex logic
- Keep handlers lean, delegate to domain functions

**Parameters:**
- Pass references when not taking ownership: `&self`, `&TodoList`
- Take ownership when needed: `content: String`
- Use `impl Into<String>` for flexible string parameters in public APIs

**Return Values:**
- `Result<T>` for fallible operations
- `Option<T>` for optional values
- `bool` for success/failure when no data needed
- Tuple `(T, U)` for multiple related values

## Module Design

**Exports:**
- Re-export key types in `mod.rs`:
```rust
// src/todo/mod.rs
pub use item::TodoItem;
pub use list::TodoList;
pub use state::TodoState;
```

**Barrel Files:**
- Use `mod.rs` to re-export public items
- Keep internal modules private unless needed externally

**Module Organization:**
- Group related functionality (e.g., `storage/` contains `file.rs`, `database.rs`, `markdown.rs`)
- Separate concerns: domain (`todo/`), persistence (`storage/`), presentation (`ui/`)

## Struct Design

**Public Fields:**
- Use `pub` fields for simple data structs (e.g., `TodoItem`, `AppState`)
- Group related fields together

**Builder Pattern:**
- Use `new()` for simple construction
- Use `with_*` methods for optional configuration
- Use `full()` for complete initialization

**Example from `src/todo/item.rs`:**
```rust
impl TodoItem {
    pub fn new(content: String, indent_level: usize) -> Self { ... }

    #[cfg(test)]
    pub fn with_state(content: String, state: TodoState, indent_level: usize) -> Self { ... }

    pub fn full(
        content: String,
        state: TodoState,
        indent_level: usize,
        parent_id: Option<Uuid>,
        due_date: Option<NaiveDate>,
        description: Option<String>,
        collapsed: bool,
    ) -> Self { ... }
}
```

## Enum Design

**State Enums:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoState {
    Empty,       // [ ]
    Checked,     // [x]
    Question,    // [?]
    Exclamation, // [!]
    InProgress,  // [*]
}
```

**Mode Enums:**
```rust
pub enum Mode {
    Navigate,
    Visual,
    Edit,
    ConfirmDelete,
    Plugin,
    Rollover,
}
```

## Derive Macros

**Common Derives:**
- `#[derive(Debug, Clone)]` - most structs
- `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` - enums and small value types
- `#[derive(Serialize, Deserialize)]` - API models and config
- `#[derive(Default)]` - when sensible defaults exist

## Test-Only Code

**Pattern:**
```rust
#[cfg(test)]
pub fn with_state(content: String, state: TodoState, indent_level: usize) -> Self { ... }

#[cfg(test)]
pub fn toggle_item_state(&mut self, index: usize) -> Result<()> { ... }
```

Use `#[cfg(test)]` for helper methods only needed in tests.

## Database Conventions

**Soft Deletes:**
- Never hard-delete records
- Set `deleted_at` timestamp instead
- All SELECT queries must include `WHERE deleted_at IS NULL`

**Timestamps:**
- RFC3339 format for all timestamps
- `created_at`, `updated_at`, `completed_at`, `deleted_at`

---

*Convention analysis: 2026-01-17*
