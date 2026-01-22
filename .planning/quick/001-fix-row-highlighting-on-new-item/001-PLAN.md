---
phase: quick-001
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/app/state.rs
  - src/app/event.rs
autonomous: true

must_haves:
  truths:
    - "When pressing 'n' to create a new item, the new edit row is visually highlighted"
    - "When pressing 'O' to insert above, the new edit row is visually highlighted"
    - "The highlight moves to the correct row when saving/confirming the new item"
  artifacts:
    - path: "src/app/state.rs"
      provides: "sync_list_state_for_new_item method"
    - path: "src/app/event.rs"
      provides: "Calls to sync_list_state_for_new_item in new item functions"
  key_links:
    - from: "src/app/event.rs"
      to: "src/app/state.rs"
      via: "sync_list_state_for_new_item call"
      pattern: "sync_list_state_for_new_item"
---

<objective>
Fix the visual row highlighting when creating new todo items.

Purpose: When pressing Enter/n/O to create a new item, the UI shows an edit row but the highlight stays on the previous row. The internal state is correct but the visual indicator is wrong, confusing users about which row they're editing.

Output: The new item edit row will be visually highlighted immediately when entering edit mode for a new item.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/app/state.rs (sync_list_state method, lines 206-237)
@src/app/event.rs (new_item_below, insert_item_above functions, lines 947-973)
@src/ui/components/todo_list.rs (render function with should_show_new_item_below/above logic)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add sync_list_state_for_new_item method</name>
  <files>src/app/state.rs</files>
  <action>
Add a new method `sync_list_state_for_new_item(&mut self)` to AppState that adjusts the list_state selection to account for a new item being created.

The logic:
1. Call the existing sync_list_state() first to get the base visible_index
2. If `is_creating_new_item` is true and `insert_above` is false, increment the selection by 1 (the new item row appears AFTER the current item)
3. If `is_creating_new_item` is true and `insert_above` is true, the selection stays the same (the new item row appears AT the current position, pushing others down)

Also need to account for expanded description boxes - if the current item has an expanded description, the new item row appears after that description box, so we need to add 1 more for the description's ListItem.

Add this method near sync_list_state() (around line 237):

```rust
/// Sync list_state selection for when creating a new item.
/// This accounts for the temporary edit row that appears during new item creation.
pub fn sync_list_state_for_new_item(&mut self) {
    // First get the base visible index
    self.sync_list_state();

    if !self.is_creating_new_item {
        return;
    }

    // Get current selection
    let current = match self.list_state.selected() {
        Some(idx) => idx,
        None => return,
    };

    if self.insert_above {
        // New item appears at current position, no change needed
        // The highlight should stay where it is
    } else {
        // New item appears below current item
        // Need to increment selection to point to the new row
        let mut offset = 1;

        // If current item has expanded description, that's another ListItem between
        if let Some(item) = self.todo_list.items.get(self.cursor_position) {
            if !item.collapsed && item.description.is_some() {
                offset += 1;
            }
        }

        self.list_state.select(Some(current + offset));
    }
}
```
  </action>
  <verify>
`cargo build` compiles without errors.
`cargo clippy` passes without warnings.
  </verify>
  <done>
New method `sync_list_state_for_new_item` exists in AppState and compiles successfully.
  </done>
</task>

<task type="auto">
  <name>Task 2: Call sync method when entering new item edit mode</name>
  <files>src/app/event.rs</files>
  <action>
Update the three functions that initiate new item creation to call `sync_list_state_for_new_item()`:

1. In `new_item_below()` (around line 957): Add `state.sync_list_state_for_new_item();` at the end

2. In `new_item_at_same_level()` (line 959-961): This just calls new_item_below(), so it's already covered

3. In `insert_item_above()` (around line 973): Add `state.sync_list_state_for_new_item();` at the end

The updated functions should look like:

```rust
fn new_item_below(state: &mut AppState) {
    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.mode = Mode::Edit;
    state.is_creating_new_item = true;
    state.insert_above = false;
    state.pending_indent_level = state
        .selected_item()
        .map(|item| item.indent_level)
        .unwrap_or(0);
    state.sync_list_state_for_new_item();
}

fn insert_item_above(state: &mut AppState) {
    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.mode = Mode::Edit;
    state.is_creating_new_item = true;
    state.insert_above = true;
    state.pending_indent_level = state
        .selected_item()
        .map(|item| item.indent_level)
        .unwrap_or(0);
    state.sync_list_state_for_new_item();
}
```
  </action>
  <verify>
`cargo build` compiles without errors.
`cargo clippy` passes without warnings.
`cargo test` passes all tests.
  </verify>
  <done>
Both `new_item_below` and `insert_item_above` call `sync_list_state_for_new_item()`.
  </done>
</task>

<task type="auto">
  <name>Task 3: Manual verification of the fix</name>
  <files></files>
  <action>
Run the TUI and verify the fix works:

```bash
cargo run
```

Test cases:
1. Navigate to an existing item, press 'n' to create new item below - the NEW row should be highlighted (showing the edit cursor)
2. Press Esc to cancel, navigate to an item, press 'O' to insert above - the NEW row should be highlighted
3. Create an item with a collapsed parent that has children - verify highlight is correct
4. If an item has an expanded description box, create a new item below it - verify highlight appears on the new row, not the description box

Expected: In all cases, the row with the blinking cursor (the edit row) should have the highlight/reverse video style applied.
  </action>
  <verify>
Manual testing confirms:
- 'n' key: new item row is highlighted
- 'O' key: new item row is highlighted
- Edge cases with descriptions work correctly
  </verify>
  <done>
Visual highlighting appears on the correct row when creating new items in all tested scenarios.
  </done>
</task>

</tasks>

<verification>
All checks pass:
- `cargo build` succeeds
- `cargo clippy` has no warnings
- `cargo test` passes
- Manual testing confirms visual highlight on new item rows
</verification>

<success_criteria>
- When pressing 'n' to create a new item, the new edit row shows the highlight
- When pressing 'O' to insert above, the new edit row shows the highlight
- No regressions in existing highlight behavior
- Code compiles and passes all tests
</success_criteria>

<output>
After completion, create `.planning/quick/001-fix-row-highlighting-on-new-item/001-SUMMARY.md`
</output>
