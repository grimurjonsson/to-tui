# Storage Layer

**Directory**: `src/storage/`

## Purpose

Persistence layer handling markdown files, SQLite database, and data lifecycle (rollover, archival).

## Components

### file.rs
- `load_or_create_todo_list(date)` - Load daily file or create empty list
- `save_todo_list(list)` - Write list to markdown file
- Path: `~/.local/share/to-tui/dailies/YYYY-MM-DD.md`

### markdown.rs
- `parse_markdown(content)` - Parse markdown to TodoItems
- `serialize_to_markdown(list)` - Convert list to markdown
- Handles checkbox states: `[ ]`, `[x]`, `[?]`, `[!]`, `[*]`
- Preserves indent levels via leading spaces/tabs
- Parses inline metadata (due dates, descriptions)

### database.rs
```rust
pub struct Database {
    conn: Connection,
}
```
- SQLite storage for archived todos
- Tables: `todos`, `archived_todos`
- Soft deletes: `deleted_at IS NULL` filter
- Methods: `insert_todo()`, `get_todos_by_date()`, `archive_todos()`

### rollover.rs
```rust
pub struct RolloverChecker {
    last_date: NaiveDate,
}
```
- Detects first open of day
- Copies incomplete items from previous day
- Archives completed items to database

### ui_cache.rs
```rust
pub struct UiCache {
    pub selected_todo_id: Option<Uuid>,
}
```
- Persists UI state between sessions
- Restores cursor position on reopen
