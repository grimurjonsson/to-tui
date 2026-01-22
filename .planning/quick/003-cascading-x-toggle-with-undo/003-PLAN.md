---
phase: quick
plan: 003
type: execute
wave: 1
depends_on: []
files_modified:
  - src/app/state.rs
autonomous: true

must_haves:
  truths:
    - "Pressing 'x' toggles Done state on current item AND all nested children"
    - "Pressing Space cycles only the current item (unchanged behavior)"
    - "Pressing 'u' after cascading 'x' restores ALL items to previous states"
  artifacts:
    - path: "src/app/state.rs"
      provides: "toggle_current_item_state_with_children method"
      contains: "toggle_current_item_state_with_children"
  key_links:
    - from: "src/app/state.rs toggle_current_item_state_with_children"
      to: "TodoList::get_item_range"
      via: "finds all descendants"
      pattern: "get_item_range"
---

<objective>
Implement cascading "x" toggle behavior where pressing "x" toggles Done state on the current item AND all nested children, while preserving Space (cycle states) behavior and ensuring undo restores all affected items atomically.

Purpose: Improve productivity by allowing quick completion of entire task trees with a single keypress
Output: Modified state.rs with new cascading toggle method
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/app/state.rs - Contains toggle_current_item_state() that needs modification
@src/app/event.rs - Maps Action::ToggleState to toggle method (no changes needed)
@src/todo/hierarchy.rs - Contains get_item_range() for finding children
@src/todo/state.rs - TodoState::toggle() method
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement cascading toggle in AppState</name>
  <files>src/app/state.rs</files>
  <action>
Rename and modify `toggle_current_item_state()` to toggle the current item AND all descendants:

1. Keep the existing `save_undo()` call at the start (this already saves the entire TodoList, so undo will restore all items atomically)

2. Use `self.todo_list.get_item_range(self.cursor_position)` to get `(start, end)` - this returns the range including the item and all its children

3. Determine the target state: If current item is Checked, target is Empty; otherwise target is Checked (this is the toggle logic from TodoState::toggle())

4. Iterate from start to end and set each item's state to the target state. Update modified_at timestamp for each.

5. Set unsaved_changes = true and return true

The method signature stays the same: `pub fn toggle_current_item_state(&mut self) -> bool`

Key insight: save_undo() already clones the entire TodoList before any changes, so when user presses "u", ALL items (including children that were already "x" before) restore to their exact previous states. No additional undo logic needed.
  </action>
  <verify>
Run `cargo test` - existing tests should pass
Run `cargo clippy` - no warnings
  </verify>
  <done>
toggle_current_item_state() modifies current item AND all descendants, undo restores all to previous states
  </done>
</task>

<task type="auto">
  <name>Task 2: Add unit tests for cascading toggle</name>
  <files>src/app/state.rs</files>
  <action>
Add tests to the existing `#[cfg(test)] mod tests` section in state.rs:

1. `test_toggle_cascades_to_children`:
   - Create TodoList with parent (Empty) and two children (Empty)
   - Set up indent levels (parent=0, children=1)
   - Create AppState, set cursor to parent
   - Call toggle_current_item_state()
   - Assert parent AND both children are now Checked

2. `test_toggle_cascade_undo_restores_all`:
   - Create TodoList with parent (Empty), child1 (already Checked), child2 (Empty)
   - Set up indent levels
   - Create AppState, set cursor to parent
   - Call toggle_current_item_state() - all become Checked
   - Call undo()
   - Assert: parent is Empty, child1 is Checked (was already x), child2 is Empty

3. `test_toggle_cascade_unchecks_all`:
   - Create TodoList with parent (Checked), child (Checked)
   - Call toggle_current_item_state()
   - Assert both are now Empty
  </action>
  <verify>
Run `cargo test toggle_cascade` - all new tests pass
  </verify>
  <done>
Unit tests verify cascading toggle AND undo behavior including mixed initial states
  </done>
</task>

</tasks>

<verification>
1. `cargo test` - all tests pass
2. `cargo clippy` - no warnings
3. Manual test: Run TUI, create parent with children, press "x" on parent - all toggle
4. Manual test: After cascading toggle, press "u" - all items restore to exact previous states
5. Manual test: Press Space on an item - only that item cycles (unchanged behavior)
</verification>

<success_criteria>
- "x" key toggles Done state on current item AND all nested children
- Space key cycles only current item (unchanged)
- Undo restores all affected items to their exact previous states
- All tests pass, no clippy warnings
</success_criteria>

<output>
After completion, create `.planning/quick/003-cascading-x-toggle-with-undo/003-SUMMARY.md`
</output>
