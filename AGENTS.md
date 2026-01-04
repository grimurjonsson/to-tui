# AGENTS.md - Coding Agent Guidelines

This document provides guidelines for AI coding agents working in this Rust codebase.

## Project Overview

A terminal-based todo list manager with:
- TUI interface (ratatui)
- REST API server (axum)
- MCP server for LLM integration
- SQLite database storage
- Daily rolling lists with archive system

## Build, Test, and Lint Commands

```bash
# Build
cargo build                        # Debug build
cargo build --release              # Release build

# Run
cargo run --bin todo               # Run TUI
cargo run --bin todo-mcp           # Run MCP server

# Test
cargo test                         # Run all tests
cargo test --lib                   # Library tests only
cargo test storage::               # Tests in storage module
cargo test test_create_rolled      # Single test by name substring
cargo test -- --nocapture          # Show println! output

# Lint and Format
cargo clippy                       # Lint (fix all warnings)
cargo fmt                          # Format code
cargo fmt -- --check               # Check formatting without changes

# Check (fast compile check)
cargo check                        # Type check without building
```

## Code Style Guidelines

### CRITICAL: No Dead Code

**Do NOT leave compile warning in the codebase.**
- Run `cargo build` and fix ALL warnings before considering work complete

**Do NOT leave unused code in the codebase.**

- Remove unused functions, methods, imports, and variables
- **NEVER use `#[allow(dead_code)]`** - if code isn't used, delete it
- If a function is only used in tests, put it inside `#[cfg(test)]` block
- Run `cargo build` and fix ALL warnings before considering work complete

### Import Order

Group imports in this order, separated by blank lines:
1. Local crate imports (`use crate::...`, `use super::...`)
2. External crate imports
3. Standard library imports (rarely needed due to prelude)

```rust
use crate::todo::{TodoItem, TodoList};
use crate::utils::paths::get_daily_file_path;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::PathBuf;
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Functions | snake_case | `load_todo_list` |
| Variables | snake_case | `date_str` |
| Types/Structs | PascalCase | `TodoItem` |
| Enums | PascalCase | `TodoState` |
| Enum variants | PascalCase | `TodoState::Checked` |
| Constants | SCREAMING_SNAKE | `DEFAULT_API_PORT` |
| Modules | snake_case | `storage::database` |

### Error Handling

- Use `anyhow::Result<T>` for all fallible functions
- Use `?` operator for error propagation
- Add context with `.with_context()` for actionable error messages
- Never use `.unwrap()` in production code (ok in tests)

```rust
pub fn load_config() -> Result<Config> {
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config at {:?}", path))?;
    Ok(toml::from_str(&content)?)
}
```

### Types and Structs

- Derive common traits: `#[derive(Debug, Clone)]` minimum
- Add `PartialEq, Eq` for types that need comparison
- Add `Copy` only for small, simple types
- Use `Option<T>` for optional fields, not sentinel values
- Prefer owned types (`String`) over references in structs

```rust
#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: Uuid,
    pub content: String,
    pub state: TodoState,
    pub parent_id: Option<Uuid>,
}
```

### Comments and Documentation

- **Avoid comments** - code should be self-explanatory
- Only exceptions:
  - Clap `///` docstrings for CLI --help text
  - Complex algorithms that truly need explanation
  - Regex patterns
- Never write comments like "// Get the user" above `get_user()`

### Module Organization

```
src/
  lib.rs          # Public module exports only
  main.rs         # CLI entry point and handlers
  cli.rs          # Clap argument definitions
  config.rs       # Configuration loading
  storage/        # Data persistence
    mod.rs
    database.rs
    file.rs
    markdown.rs
  todo/           # Core domain types
    mod.rs
    item.rs
    list.rs
    state.rs
  api/            # REST API
  mcp/            # MCP server
  ui/             # TUI components
  utils/          # Shared utilities
```

### Testing

- Tests go in the same file, in a `#[cfg(test)] mod tests` block
- Use descriptive test names: `test_<what>_<scenario>`
- One assertion focus per test when practical
- Use `tempfile` crate for file system tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_state_wraps_around() {
        assert_eq!(TodoState::Exclamation.cycle(), TodoState::Empty);
    }
}
```

### Database Patterns

- Initialize tables in `init_database()` with `CREATE TABLE IF NOT EXISTS`
- Use `?` placeholders, never string interpolation for values
- Wrap multi-statement operations in transactions when needed
- Date format: `YYYY-MM-DD` as TEXT

### API Patterns (Axum)

- Return `impl IntoResponse` from handlers
- Use `Json<T>` for request/response bodies
- Use `Query<T>` for query parameters
- Handle errors by returning appropriate `StatusCode`

### Async Code

- Use `tokio` runtime (already configured)
- Only the API and MCP servers use async
- TUI and CLI remain synchronous

## Project-Specific Patterns

### Todo States
```rust
TodoState::Empty       // [ ] - pending
TodoState::Checked     // [x] - complete
TodoState::Question    // [?] - needs clarification
TodoState::Exclamation // [!] - important
```

### Date Handling
- Use `chrono::NaiveDate` for dates (no timezone)
- Format for storage/display: `"%Y-%m-%d"` or `"%B %d, %Y"`
- Today: `Local::now().date_naive()`

### UUID Usage
- Every `TodoItem` has a unique `Uuid`
- Generate with `Uuid::new_v4()`
- Store as TEXT in SQLite

## Quick Reference

```bash
# Common workflows
cargo test test_name        # Run specific test
cargo clippy               # Check for issues
cargo build 2>&1 | head    # Quick error check
just test                  # Run all tests via justfile
just tui                   # Run the TUI
```

## Before Submitting Changes

1. `cargo fmt` - Format code
2. `cargo clippy` - Fix all warnings
3. `cargo build` - Ensure no warnings (especially unused code)
4. `cargo test` - All tests pass
5. Remove any `#[allow(dead_code)]` you may have added
