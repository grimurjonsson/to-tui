---
phase: 13-event-hooks
plan: 01
subsystem: plugin-interface
tags: [abi_stable, ffi, events, hooks, plugin-api]

# Dependency graph
requires:
  - phase: 09-host-api
    provides: FfiCommand, Plugin trait, HostApi types
  - phase: 06-ffi-types
    provides: FfiTodoItem, FfiTodoState, RString, RVec
provides:
  - FfiEvent enum with lifecycle event variants
  - FfiEventType enum for event subscription
  - FfiHookResponse struct for hook return values
  - Plugin trait extension with subscribed_events() and on_event()
  - call_plugin_on_event() panic-safe wrapper
affects: [13-02, 13-03, totui-plugin-host, example-plugins]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Event enum pattern with variant-specific data"
    - "Event type enum for subscription filtering"
    - "#[sabi(last_prefix_field)] on final trait method"

key-files:
  created:
    - crates/totui-plugin-interface/src/events.rs
  modified:
    - crates/totui-plugin-interface/src/plugin.rs
    - crates/totui-plugin-interface/src/lib.rs

key-decisions:
  - "FfiEventType separate enum (not derive from FfiEvent discriminant)"
  - "FfiFieldChange indicates single modified field or Multiple"
  - "FfiEventSource tracks event origin (Manual, Rollover, Plugin, Api)"
  - "on_event() is last_prefix_field (ABI extensibility)"

patterns-established:
  - "Event types use #[repr(u8)] with explicit discriminants"
  - "Event enum uses #[repr(C)] for FFI safety"
  - "Helper methods on event enum for type extraction"

# Metrics
duration: 7min
completed: 2026-01-26
---

# Phase 13 Plan 01: Event Types Summary

**FFI-safe event types and Plugin trait extension for lifecycle hooks with FfiEvent enum, subscription mechanism, and panic-safe wrapper**

## Performance

- **Duration:** 7 min
- **Started:** 2026-01-26T15:40:54Z
- **Completed:** 2026-01-26T15:47:54Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Created events.rs module with complete FFI-safe event type hierarchy
- Extended Plugin trait with subscribed_events() and on_event() hook methods
- Added call_plugin_on_event() panic-safe wrapper for FFI boundary safety
- Comprehensive unit tests covering all event types and helper methods

## Task Commits

Each task was committed atomically:

1. **Task 1: Create events.rs with FFI-safe event types** - `94c3045` (feat)
2. **Task 2: Extend Plugin trait with hook methods** - `f0d798f` (feat)
3. **Task 3: Add helper methods to FfiEvent** - `ce7ee0d` (feat)

## Files Created/Modified
- `crates/totui-plugin-interface/src/events.rs` - New module with FfiEvent, FfiEventType, FfiEventSource, FfiFieldChange, FfiHookResponse
- `crates/totui-plugin-interface/src/plugin.rs` - Extended Plugin trait with subscribed_events() and on_event()
- `crates/totui-plugin-interface/src/lib.rs` - Export events module and types, add call_plugin_on_event to re-exports

## Decisions Made
- FfiEventType as separate enum for subscription (cleaner API than deriving from event discriminant)
- FfiFieldChange includes Multiple variant for batch updates
- FfiEventSource distinguishes Manual/Rollover/Plugin/Api origins
- Moved #[sabi(last_prefix_field)] from on_config_loaded to on_event (new last method)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Event types ready for host-side dispatcher (Plan 02)
- Plugin trait ready for example plugin implementation (Plan 03)
- All types exported from crate root for consumer use

---
*Phase: 13-event-hooks*
*Completed: 2026-01-26*
