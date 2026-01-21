# TodoList

**File**: `src/todo/list.rs`

## Purpose

Container for TodoItems with date association, hierarchy management, and operations for manipulating the list.

## Structure

```rust
pub struct TodoList {
    pub items: Vec<TodoItem>,
    pub date: NaiveDate,
    pub path: Option<PathBuf>,
}
```

## Key Methods

### Navigation
- `get_item(index)` / `get_item_mut(index)` - Access items by index
- `find_by_id(uuid)` - Find item by UUID
- `get_parent(item)` - Get parent of nested item

### Manipulation
- `add_item(item)` - Append new item
- `insert_item(index, item)` - Insert at position
- `remove_item(index)` - Remove and return item
- `move_item(from, to)` - Reorder items

### Hierarchy
- `indent_item(index)` - Increase indent level, set parent
- `dedent_item(index)` - Decrease indent level
- `get_children(parent_id)` - Get all children of item
- `get_subtree(index)` - Get item with all descendants
- `delete_with_children(index)` - Remove item and descendants

### Filtering
- `incomplete_items()` - Get items not marked done
- `items_by_state(state)` - Filter by state

## Persistence

Associated with a file path for markdown storage. The `date` field determines the daily file location.
