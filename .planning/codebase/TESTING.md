# Testing Patterns

**Analysis Date:** 2026-01-17

## Test Framework

**Runner:**
- Built-in `cargo test` (Rust's standard test framework)
- No external test config files

**Assertion Library:**
- Standard library `assert!`, `assert_eq!`, `assert_ne!`
- `pretty_assertions` crate for enhanced diff output (dev-dependency)

**Run Commands:**
```bash
cargo test                    # Run all tests
cargo test <test_name>        # Run specific test
cargo test -- --nocapture     # Show println output
cargo test -- --list          # List all tests
```

## Test File Organization

**Location:**
- Co-located in same file as implementation (inline `#[cfg(test)]` modules)
- Tests live at bottom of each source file

**Naming:**
- Test functions: `test_<what_is_being_tested>`
- Example: `test_new`, `test_toggle_state`, `test_parse_simple_list`

**Structure:**
```
src/
├── todo/
│   ├── item.rs           # Contains #[cfg(test)] mod tests { ... }
│   ├── list.rs           # Contains #[cfg(test)] mod tests { ... }
│   ├── state.rs          # Contains #[cfg(test)] mod tests { ... }
│   └── hierarchy.rs      # Contains #[cfg(test)] mod tests { ... }
├── storage/
│   ├── database.rs       # Contains #[cfg(test)] mod tests { ... }
│   ├── file.rs           # Contains #[cfg(test)] mod tests { ... }
│   └── markdown.rs       # Contains #[cfg(test)] mod tests { ... }
└── utils/
    ├── paths.rs          # Contains #[cfg(test)] mod tests { ... }
    └── unicode.rs        # Contains #[cfg(test)] mod tests { ... }
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    // Helper function for test setup
    fn create_test_list() -> TodoList {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let path = PathBuf::from("/tmp/test.md");
        TodoList::new(date, path)
    }

    #[test]
    fn test_new() {
        let list = create_test_list();
        assert!(list.items.is_empty());
        assert_eq!(list.date.year(), 2025);
    }

    #[test]
    fn test_add_item() {
        let mut list = create_test_list();
        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());

        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].content, "Task 1");
        assert_eq!(list.items[1].content, "Task 2");
    }
}
```

**Patterns:**
- Setup: Helper functions at top of test module (`create_test_list()`, `setup_test_db()`)
- Teardown: Use `tempfile::TempDir` which auto-cleans on drop
- Assertions: One logical assertion per test when possible

## Mocking

**Framework:** No dedicated mocking framework

**Patterns:**
- Use `tempfile` crate for filesystem isolation
- Create in-memory database connections for database tests
- `#[cfg(test)]` helper constructors for test data

**Database Test Setup:**
```rust
fn setup_test_db() -> (TempDir, Connection) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).unwrap();

    conn.execute(
        "CREATE TABLE todos (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL,
            content TEXT NOT NULL,
            ...
        )",
        [],
    )
    .unwrap();

    (temp_dir, conn)
}
```

**What to Mock:**
- File system (use `tempfile::TempDir`)
- Database (create isolated test database)
- Current date (pass date as parameter instead of using `Local::now()`)

**What NOT to Mock:**
- Core domain logic (test real implementations)
- Simple value transformations

## Fixtures and Factories

**Test Data:**
```rust
fn create_test_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
}

fn create_test_path() -> PathBuf {
    PathBuf::from("/tmp/2025-12-31.md")
}

fn create_test_list(date: NaiveDate) -> TodoList {
    TodoList::new(date, PathBuf::from("/tmp/test.md"))
}
```

**Location:**
- Defined within `#[cfg(test)] mod tests { }` block
- Private to test module

**Test-Only Constructors:**
```rust
// In src/todo/item.rs
impl TodoItem {
    #[cfg(test)]
    pub fn with_state(content: String, state: TodoState, indent_level: usize) -> Self {
        let now = Utc::now();
        let completed_at = if state == TodoState::Checked {
            Some(now)
        } else {
            None
        };
        Self {
            id: Uuid::new_v4(),
            content,
            state,
            indent_level,
            // ... other fields
        }
    }
}
```

## Coverage

**Requirements:** Not enforced (no coverage threshold)

**View Coverage:**
```bash
# Using cargo-tarpaulin (if installed)
cargo tarpaulin --out Html
```

## Test Types

**Unit Tests:**
- Test individual functions and methods
- Co-located with implementation
- Fast execution (no external dependencies)

**Integration Tests:**
- Database tests that create real SQLite connections
- File I/O tests using `tempfile`
- Serialize/deserialize round-trip tests

**E2E Tests:**
- Not present in codebase
- TUI and API tested manually or via external tools

## Common Patterns

**Async Testing:**
- Not used (tests are synchronous)
- Database and file operations are synchronous

**Error Testing:**
```rust
#[test]
fn test_indent_first_item_fails() {
    let mut list = create_test_list();
    list.add_item("Parent".to_string());

    // Cannot indent first item
    assert!(list.indent_item(0).is_err());
}

#[test]
fn test_outdent_top_level_fails() {
    let mut list = create_test_list();
    list.add_item("Task".to_string());

    // Cannot outdent top-level item
    assert!(list.outdent_item(0).is_err());
}
```

**State Transition Testing:**
```rust
#[test]
fn test_toggle_state() {
    let mut item = TodoItem::new("Task".to_string(), 0);
    assert_eq!(item.state, TodoState::Empty);

    item.toggle_state();
    assert_eq!(item.state, TodoState::Checked);

    item.toggle_state();
    assert_eq!(item.state, TodoState::Empty);
}

#[test]
fn test_cycle_state() {
    let mut item = TodoItem::new("Task".to_string(), 0);
    assert_eq!(item.state, TodoState::Empty);

    item.cycle_state();
    assert_eq!(item.state, TodoState::InProgress);

    item.cycle_state();
    assert_eq!(item.state, TodoState::Checked);
    // ... continues through all states
}
```

**Round-Trip Testing:**
```rust
#[test]
fn test_round_trip() {
    let date = create_test_date();
    let path = create_test_path();
    let mut list = TodoList::new(date, path.clone());

    list.add_item_with_indent("Parent".to_string(), 0);
    list.add_item_with_indent("Child".to_string(), 1);
    list.items[1].state = TodoState::Checked;

    let markdown = serialize_todo_list_clean(&list);
    let parsed = parse_todo_list(&markdown, date, path).unwrap();

    assert_eq!(parsed.items.len(), 2);
    assert_eq!(parsed.items[0].content, "Parent");
    assert_eq!(parsed.items[1].content, "Child");
    assert_eq!(parsed.items[1].state, TodoState::Checked);
}
```

**Database Persistence Testing:**
```rust
#[test]
fn test_save_and_load_preserves_order() {
    let (_temp_dir, conn) = setup_test_db();
    let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let mut list = create_test_list(date);
    list.add_item("First".to_string());
    list.add_item("Second".to_string());
    list.add_item("Third".to_string());

    // Store original IDs
    let original_ids: Vec<Uuid> = list.items.iter().map(|i| i.id).collect();

    save_to_test_db(&conn, &list);
    let loaded = load_from_test_db(&conn, date);

    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].content, "First");
    assert_eq!(loaded[1].content, "Second");
    assert_eq!(loaded[2].content, "Third");

    // Verify IDs are preserved
    assert_eq!(loaded[0].id, original_ids[0]);
    assert_eq!(loaded[1].id, original_ids[1]);
    assert_eq!(loaded[2].id, original_ids[2]);
}
```

## Test Count by Module

| Module | Test Count |
|--------|------------|
| `storage::database` | 14 |
| `storage::markdown` | 8 |
| `storage::file` | 2 |
| `storage::rollover` | 1 |
| `todo::item` | 8 |
| `todo::list` | 12 |
| `todo::state` | 5 |
| `todo::hierarchy` | 2 |
| `utils::paths` | 6 |
| `utils::unicode` | 8 |
| `config` | 3 |
| `keybindings` | 6 |
| **Total** | **83** |

## Dev Dependencies

```toml
[dev-dependencies]
tempfile = "3.13"
pretty_assertions = "1.4"
```

- `tempfile`: Create temporary directories/files that auto-cleanup
- `pretty_assertions`: Better diff output for failed assertions

## Untested Areas

**Not Unit Tested:**
- `src/main.rs` command handlers
- `src/app/event.rs` event handling
- `src/ui/` rendering code
- `src/api/handlers.rs` HTTP handlers
- `src/mcp/server.rs` MCP tool implementations
- `src/plugin/` plugin system

**Testing Strategy for These:**
- Manual testing via TUI
- Bruno collection for API testing (`/bruno/`)
- MCP inspector for MCP server testing

---

*Testing analysis: 2026-01-17*
