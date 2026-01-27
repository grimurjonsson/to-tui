---
phase: 09-host-api-layer
plan: 01
subsystem: plugin-interface
tags: [abi_stable, sabi_trait, ffi, host-api, command-pattern]

# Dependency graph
requires:
  - phase: 06-ffi-safe-types
    provides: FfiTodoItem, FfiTodoState, FfiPriority, StableAbi derives
provides:
  - FfiCommand enum for plugin mutations
  - HostApi trait (query interface for plugins)
  - FfiProjectContext, FfiTodoQuery, FfiTodoNode types
  - Extended FfiTodoItem with position field
affects: [09-02, 09-03, 10-plugin-execute-method, 11-command-processing]

# Tech tracking
tech-stack:
  added: []
  patterns: [sabi_trait for FFI-safe traits, last_prefix_field for extensibility]

key-files:
  created:
    - crates/totui-plugin-interface/src/host_api.rs
  modified:
    - crates/totui-plugin-interface/src/types.rs
    - crates/totui-plugin-interface/src/lib.rs
    - src/plugin/ffi_convert.rs

key-decisions:
  - "HostApi uses sabi_trait generating HostApi_TO for FFI-safe trait object"
  - "FfiCommand uses repr(C) enum for mutation operations"
  - "query_todos_tree marked with last_prefix_field for future extensibility"
  - "position field added to FfiTodoItem (host-assigned during query)"

patterns-established:
  - "HostApi_TO naming follows abi_stable underscore convention (like Plugin_TO)"
  - "ROption<RString> for optional string fields in FFI types"
  - "Manual Default impl for query structs (StableAbi doesn't support derive Default)"

# Metrics
duration: 5min
completed: 2026-01-26
---

# Phase 09 Plan 01: Host API Types Summary

**FFI-safe Host API types with command enum, query types, and HostApi trait for plugin-host communication**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-26T10:00:08Z
- **Completed:** 2026-01-26T10:05:00Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- Created FfiCommand enum with CreateTodo, UpdateTodo, DeleteTodo, MoveTodo variants
- Implemented HostApi trait with sabi_trait generating HostApi_TO type
- Added query types: FfiTodoQuery, FfiStateFilter, FfiProjectContext, FfiTodoNode
- Extended FfiTodoItem with position field for list ordering

## Task Commits

Each task was committed atomically:

1. **Task 1+2: Create host_api.rs with types and HostApi trait** - `d6c9d38` (feat)
2. **Task 3: Add position field and export host_api module** - `15be741` (feat)

## Files Created/Modified
- `crates/totui-plugin-interface/src/host_api.rs` - New module with all Host API types and trait
- `crates/totui-plugin-interface/src/types.rs` - Added position field to FfiTodoItem
- `crates/totui-plugin-interface/src/lib.rs` - Exported host_api module and re-exports
- `src/plugin/ffi_convert.rs` - Updated From impl and tests for position field

## Decisions Made
- Combined Tasks 1 and 2 into single file (host_api.rs) as they're tightly coupled
- Used manual Default impl for FfiTodoQuery since StableAbi doesn't support derive Default
- HostApi trait marked Send + Sync for thread safety

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Host API types ready for Plan 09-02 (query implementation)
- HostApi_TO type available for trait object usage
- FfiCommand ready for Plan 09-03 (command builder)

---
*Phase: 09-host-api-layer*
*Completed: 2026-01-26*
