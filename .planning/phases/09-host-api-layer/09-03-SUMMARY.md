---
phase: 09-host-api-layer
plan: 03
completed: 2026-01-26
duration: 4min
subsystem: plugin
tags: [command-executor, plugin-mutations, ffi, undo-redo]

dependency-graph:
  requires: ["09-01"]
  provides: ["CommandExecutor", "execute_with_host"]
  affects: ["10-plugin-calling-convention"]

tech-stack:
  added: []
  patterns: ["command-pattern", "temp-id-mapping", "soft-delete"]

key-files:
  created:
    - src/plugin/command_executor.rs
  modified:
    - src/plugin/mod.rs
    - crates/totui-plugin-interface/src/plugin.rs
    - crates/totui-plugin-interface/src/lib.rs

decisions:
  - id: 09-03-temp-id-resolution
    choice: "Check temp_id_map first, then parse as UUID"
    reason: "Plugins create items with temp IDs, reference them in same batch"
  - id: 09-03-soft-delete
    choice: "Use deleted_at timestamp for DeleteTodo command"
    reason: "Codebase convention - never hard delete records"
  - id: 09-03-error-on-not-found
    choice: "Return Err(anyhow!(not found)) for invalid UUIDs"
    reason: "Per CONTEXT.md decision - fail fast, don't silently skip"

metrics:
  tasks: 3/3
  tests-added: 11
  files-created: 1
  files-modified: 3
---

# Phase 09 Plan 03: CommandExecutor Summary

CommandExecutor processes FfiCommand batches from plugins with temp ID resolution and soft delete.

## Completed Tasks

| # | Task | Commit | Key Changes |
|---|------|--------|-------------|
| 1 | Create CommandExecutor with execute_batch | ca156d4 | CommandExecutor struct, handle_create/update/delete/move |
| 2 | Extend Plugin trait with execute_with_host | afad9c9 | Plugin trait method, call_plugin_execute_with_host wrapper |
| 3 | Wire module and add unit tests | eeaaa6a | Module exports, 11 unit tests |

## Key Implementation Details

### CommandExecutor

The `CommandExecutor` struct processes plugin commands with these features:

1. **Temp ID Mapping:** Plugins can create items with `temp_id` and reference them in the same batch via `parent_id`. The executor maps temp IDs to real UUIDs as items are created.

2. **Command Handlers:**
   - `handle_create`: Creates TodoItem, resolves parent_id, inserts at appropriate position
   - `handle_update`: Updates content/state/priority/due_date/description, sets modified_at
   - `handle_delete`: Sets deleted_at (soft delete per codebase convention)
   - `handle_move`: Relocates item to Before/After/AtIndex position

3. **Post-batch:** Calls `todo_list.recalculate_parent_ids()` to ensure hierarchy consistency.

### Plugin Trait Extension

Added `execute_with_host(input, host) -> RVec<FfiCommand>` method to Plugin trait:
- Receives HostApi_TO for querying current state
- Returns commands for atomic batch execution
- Marked with `#[sabi(last_prefix_field)]` for future extensibility

### Panic Safety

`call_plugin_execute_with_host()` wrapper catches panics and converts to RResult::RErr, preventing UB at FFI boundary.

## Verification Results

- `cargo check`: Pass
- `cargo test`: 187 tests pass (11 new command_executor tests)
- `cargo clippy`: No warnings
- `execute_with_host` exists in plugin.rs

## Deviations from Plan

None - plan executed exactly as written.

## Files Changed

| File | Change |
|------|--------|
| src/plugin/command_executor.rs | Created - CommandExecutor implementation with 11 tests |
| src/plugin/mod.rs | Added command_executor module and CommandExecutor export |
| crates/totui-plugin-interface/src/plugin.rs | Added execute_with_host method and call wrapper |
| crates/totui-plugin-interface/src/lib.rs | Exported call_plugin_execute_with_host |

## Next Phase Readiness

Phase 09 plan 3 complete. Ready for Phase 10 (Plugin Calling Convention) which will:
- Integrate CommandExecutor into AppState
- Wire plugin execution workflow with undo snapshots
- Handle plugin results and errors in TUI
