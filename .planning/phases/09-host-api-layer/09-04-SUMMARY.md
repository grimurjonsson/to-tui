---
phase: 09-host-api-layer
plan: 04
type: summary
subsystem: plugin-framework
tags: [gap-closure, calling-convention, undo-integration, command-pattern]

# Dependency graph
dependency:
  requires: [09-01, 09-02, 09-03]
  provides: [execute_plugin_with_host-calling-convention]
  affects: [10-01]

# Tech stack additions
tech-stack:
  added: []
  patterns: [calling-convention-in-appstate]

# Files changed
key-files:
  created: []
  modified:
    - src/app/state.rs

# Decisions from this plan
decisions:
  - id: "09-04-01"
    choice: "Calling convention in AppState not CommandExecutor"
    rationale: "AppState owns save_undo(), undo_stack, todo_list, and plugin_loader"

# Metrics
metrics:
  duration: "5min"
  completed: "2026-01-26"
---

# Phase 9 Plan 4: Host API Calling Convention Summary

**One-liner:** Added execute_plugin_with_host method to AppState - creates undo snapshot before executing plugin commands via CommandExecutor.

## What Was Built

### Gap Closure: Plugin Calling Convention

Addressed gaps identified in Phase 9 verification:
1. **Undo/redo integration missing** - CommandExecutor.execute_batch() didn't integrate with AppState's undo system
2. **No calling convention** - No method existed to invoke plugin with HostApi and process returned commands

**Solution:** New `execute_plugin_with_host` method in AppState that:
- Finds plugin by name from PluginLoader
- Builds enabled projects set (currently just current project)
- Creates PluginHostApiImpl with query access
- Calls plugin's execute_with_host via panic-safe wrapper
- Creates undo snapshot BEFORE executing any commands
- Executes FfiCommands through CommandExecutor
- Marks state as unsaved

### Key Code Pattern

```rust
// In AppState.execute_plugin_with_host()
let commands = call_plugin_execute_with_host(&plugin, input, host_api)?;

if !commands.is_empty() {
    self.save_undo();  // BEFORE mutations - enables undo of all commands
    executor.execute_batch(commands, &mut self.todo_list)?;
    self.unsaved_changes = true;
}
```

## Commits

| Hash | Type | Description |
|------|------|-------------|
| fc0a142 | feat | Add execute_plugin_with_host method to AppState |
| 6765a36 | test | Add unit test for execute_plugin_with_host |

## Key Links Verified

| From | To | Via | Purpose |
|------|----|-----|---------|
| AppState | CommandExecutor | execute_batch() | Process plugin commands |
| AppState | PluginHostApiImpl | new() | Provide query access |
| AppState | save_undo() | self.save_undo() | Undo snapshot before mutations |

## Deviations from Plan

None - plan executed exactly as written.

## Test Results

- All 188 tests pass (180 lib + 7 main + 1 doctest)
- New test: `test_execute_plugin_with_host_not_found` verifies error handling

## Technical Notes

1. **Why calling convention is in AppState (not CommandExecutor):**
   - AppState owns `save_undo()` and `undo_stack`
   - AppState owns `todo_list` (mutable reference needed)
   - AppState owns `plugin_loader` (access to plugins)
   - Follows existing patterns (e.g., `toggle_current_item_state` calls save_undo before mutating)

2. **Undo snapshot timing:**
   - Created AFTER checking for empty commands (optimization)
   - Created BEFORE execute_batch (ensures undo captures pre-mutation state)

3. **Dead code warning expected:**
   - Method is public but not called from TUI yet
   - Phase 10 will wire the P key to use this method

## Next Phase Readiness

Phase 9 Host API Layer is now COMPLETE:
- Plan 01: FfiCommand, HostApi trait, HostApi_TO
- Plan 02: PluginHostApiImpl query implementation
- Plan 03: CommandExecutor with temp ID mapping
- Plan 04: Calling convention with undo integration

Ready for Phase 10 (Plugin UI Integration) which will:
- Wire P key to execute_plugin_with_host
- Add plugin selection UI
- Display command execution results
