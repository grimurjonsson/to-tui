# TodoItem

**File**: `src/todo/item.rs`

## Purpose

Core domain entity representing a single todo item with support for hierarchical nesting, multiple states, priorities, and rich metadata.

## Structure

```rust
pub struct TodoItem {
    pub id: Uuid,
    pub content: String,
    pub state: TodoState,
    pub indent_level: usize,
    pub parent_id: Option<Uuid>,
    pub due_date: Option<NaiveDate>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub collapsed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

## Key Methods

- `new(content)` - Create new todo with generated UUID
- `toggle_state()` - Cycle through Empty → Checked → Empty
- `set_state(state)` - Set specific state, updates completed_at
- `is_done()` - Check if state is Checked
- `with_indent(level)` - Builder pattern for indent level
- `with_parent(id)` - Builder pattern for parent relationship

## States

- **Empty** `[ ]` - Pending/not started
- **Checked** `[x]` - Completed
- **Question** `[?]` - Needs clarification
- **Exclamation** `[!]` - Important/urgent
- **InProgress** `[*]` - Currently working on

## Serialization

Implements `Serialize`/`Deserialize` for JSON API and database storage.
