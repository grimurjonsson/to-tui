---
phase: 06-ffi-safe-type-layer
plan: 01
subsystem: plugin
tags: [abi_stable, ffi, stable-abi, dynamic-plugins, rust-plugins]

# Dependency graph
requires: []
provides:
  - totui-plugin-interface crate with FFI-safe types
  - FfiTodoItem, FfiTodoState, FfiPriority with StableAbi
  - Bidirectional conversion between native and FFI types
affects: [06-02, 07-plugin-trait, 08-host-infrastructure]

# Tech tracking
tech-stack:
  added: [abi_stable 0.11]
  patterns: [workspace-crate-organization, ffi-type-conversion, repr-c-structs]

key-files:
  created:
    - crates/totui-plugin-interface/Cargo.toml
    - crates/totui-plugin-interface/src/lib.rs
    - crates/totui-plugin-interface/src/types.rs
    - src/plugin/ffi_convert.rs
  modified:
    - Cargo.toml

key-decisions:
  - "Use u32 for indent_level (usize not FFI-safe)"
  - "Use i64 timestamps instead of DateTime<Utc>"
  - "Exclude collapsed and deleted_at from FFI types (UI-only and host-filtered)"
  - "Use RString/ROption for FFI-safe strings and optionals"

patterns-established:
  - "FFI enums: #[repr(u8)] with explicit discriminants"
  - "FFI structs: #[repr(C)] with StableAbi derive"
  - "Conversion: From for infallible, TryFrom for fallible with context"
  - "Timestamps: timestamp_millis() for DateTime<Utc> serialization"

# Metrics
duration: 12min
completed: 2026-01-24
---

# Phase 6 Plan 1: FFI-Safe Type Layer Summary

**Workspace organization with totui-plugin-interface crate providing StableAbi types and bidirectional conversion for TodoItem, TodoState, Priority**

## Performance

- **Duration:** 12 min
- **Started:** 2026-01-24
- **Completed:** 2026-01-24
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Created Cargo workspace with totui-plugin-interface member crate
- Defined FFI-safe types: FfiTodoItem, FfiTodoState, FfiPriority with #[derive(StableAbi)]
- Implemented bidirectional From/TryFrom conversions with comprehensive error context
- Added unit tests for roundtrip conversion and error handling

## Task Commits

Each task was committed atomically:

1. **Task 1: Create workspace and interface crate structure** - `46fdf79` (feat)
2. **Task 2: Define FFI-safe types with StableAbi** - (included in Task 1 - types needed for lib.rs exports)
3. **Task 3: Implement bidirectional type conversion** - `5affc20` (feat)

## Files Created/Modified

- `Cargo.toml` - Added workspace section and totui-plugin-interface dependency
- `crates/totui-plugin-interface/Cargo.toml` - Interface crate with abi_stable 0.11 dependency
- `crates/totui-plugin-interface/src/lib.rs` - Re-exports FfiTodoItem, FfiTodoState, FfiPriority
- `crates/totui-plugin-interface/src/types.rs` - FFI-safe type definitions with StableAbi
- `src/plugin/ffi_convert.rs` - Bidirectional conversion implementations with tests
- `src/plugin/mod.rs` - Added ffi_convert module

## Decisions Made

1. **u32 for indent_level** - usize is not FFI-safe across platforms; u32 sufficient for indentation depth
2. **i64 timestamps** - DateTime<Utc> not FFI-safe; Unix milliseconds provides full precision
3. **Excluded collapsed/deleted_at** - UI-only field and host-filtered data not needed in FFI
4. **TryFrom for FfiTodoItem->TodoItem** - UUID and date parsing can fail; proper error context provided

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - abi_stable 0.11 worked as documented.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- FFI-safe types ready for Plugin trait definition (plan 06-02)
- Conversion implementations ready for host infrastructure (phase 08)
- Interface crate structure supports version protocol addition

---
*Phase: 06-ffi-safe-type-layer*
*Completed: 2026-01-24*
