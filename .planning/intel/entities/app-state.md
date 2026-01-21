# AppState

**File**: `src/app/state.rs`

## Purpose

Central application state for the TUI, managing todo list, UI state, undo/redo history, plugin execution, and async operations.

## Structure

```rust
pub struct AppState {
    // Core data
    pub todo_list: TodoList,
    pub today: NaiveDate,
    pub viewing_date: NaiveDate,

    // UI state
    pub cursor_position: usize,
    pub mode: Mode,
    pub scroll_offset: usize,
    pub input_buffer: String,
    pub edit_position: usize,

    // Undo/Redo
    pub undo_stack: Vec<TodoList>,
    pub redo_stack: Vec<TodoList>,

    // Status
    pub status_message: Option<(String, Instant)>,
    pub unsaved_changes: bool,
    pub should_quit: bool,

    // Plugin system
    pub plugin_state: Option<PluginState>,
    pub plugin_result_rx: Option<Receiver<PluginResult>>,

    // Async operations
    pub new_version_available: Option<String>,
    pub download_progress: Option<f32>,

    // Config
    pub theme: Theme,
    pub config: Config,
}
```

## Key Methods

### Navigation
- `move_cursor_up()` / `move_cursor_down()` - Move selection
- `get_selected_todo()` - Get currently selected item
- `scroll_to_item(id)` - Scroll to specific item

### Editing
- `start_edit()` / `finish_edit()` - Enter/exit edit mode
- `push_undo()` - Save state for undo
- `undo()` / `redo()` - History navigation

### State Management
- `set_mode(mode)` - Change application mode
- `set_status_message(msg)` - Show temporary message
- `reload_from_database()` - Refresh from storage
- `save()` - Persist changes to file

### Readonly Mode
- `is_readonly()` - True when viewing archived date
- Prevents edits when viewing past dates
