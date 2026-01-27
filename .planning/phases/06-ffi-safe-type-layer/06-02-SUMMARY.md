---
phase: 06-ffi-safe-type-layer
plan: 02
subsystem: plugin
tags: [abi_stable, ffi, sabi_trait, plugin-trait, version-protocol, semver]

# Dependency graph
requires:
  - phase: 06-01
    provides: FFI-safe types (FfiTodoItem, FfiTodoState, FfiPriority)
provides:
  - Plugin trait with #[sabi_trait] for FFI-safe trait objects
  - PluginModule with RootModule for library loading
  - Version compatibility checking with semver rules
  - Panic handling wrapper for safe FFI boundary calls
affects: [07-plugin-trait, 08-host-infrastructure, example-plugins]

# Tech tracking
tech-stack:
  added: [semver 1.0]
  patterns: [sabi_trait-plugin-interface, root-module-versioning, panic-boundary-handling]

key-files:
  created:
    - crates/totui-plugin-interface/src/plugin.rs
    - crates/totui-plugin-interface/src/version.rs
  modified:
    - crates/totui-plugin-interface/src/lib.rs
    - crates/totui-plugin-interface/Cargo.toml

key-decisions:
  - "Use #[sabi(last_prefix_field)] on generate() to allow future trait extension"
  - "Semver compatibility: same major + host >= plugin_min for compatibility"
  - "Plugin_TO naming follows abi_stable convention (underscore prefix_ref)"

patterns-established:
  - "Plugin trait: sabi_trait generates Plugin_TO for trait objects"
  - "RootModule: PluginModule_Ref as library entry point with version strings"
  - "Panic safety: call_plugin_generate() wraps calls with catch_unwind"
  - "Version check: is_version_compatible() before any plugin calls"

# Metrics
duration: 3min
completed: 2026-01-24
---

# Phase 6 Plan 2: Plugin Trait and Version Protocol Summary

**Plugin trait with #[sabi_trait] generating Plugin_TO, RootModule-based PluginModule for library loading, semver version compatibility checking, and panic handling wrapper**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-24T15:03:49Z
- **Completed:** 2026-01-24T15:06:37Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Defined Plugin trait using #[sabi_trait] macro - generates Plugin_TO for FFI-safe trait objects
- Implemented PluginModule with RootModule for dynamic library loading with automatic version strings
- Created is_version_compatible() function with semver rules (same major, host >= plugin_min)
- Added call_plugin_generate() panic handling wrapper to catch plugin panics at FFI boundary
- Added 6 unit tests for version compatibility scenarios plus doctest for is_version_compatible

## Task Commits

Each task was committed atomically:

1. **Task 1: Define Plugin trait with #[sabi_trait]** - `429ceb7` (feat)
2. **Task 2: Implement PluginModule with RootModule for version protocol** - `75dbbf5` (feat)
3. **Task 3: Add panic handling and version tests** - (included in Tasks 1 and 2)

Note: Task 3 content (panic handling and tests) was included in Tasks 1 and 2 for better commit atomicity.

## Files Created/Modified

- `crates/totui-plugin-interface/src/plugin.rs` - Plugin trait with sabi_trait, Plugin_TO type, call_plugin_generate wrapper
- `crates/totui-plugin-interface/src/version.rs` - PluginModule, RootModule impl, is_version_compatible(), INTERFACE_VERSION
- `crates/totui-plugin-interface/src/lib.rs` - Re-exports for Plugin, Plugin_TO, PluginModule, version functions
- `crates/totui-plugin-interface/Cargo.toml` - Added semver 1.0 dependency

## Decisions Made

1. **#[sabi(last_prefix_field)] on generate()** - Allows adding new methods to Plugin trait in future versions without breaking ABI
2. **Semver compatibility rules** - Same major version required, host version must be >= plugin minimum (standard semver semantics)
3. **Allow non_camel_case_types in version.rs** - PluginModule_Ref naming follows abi_stable convention
4. **Include panic handling in plugin.rs** - call_plugin_generate() colocated with Plugin trait for discoverability

## Deviations from Plan

None - plan executed exactly as written. Task 3 content was logically combined with Tasks 1 and 2.

## Issues Encountered

- **sabi_trait macro warning** - The abi_stable library generates a `non_local_definitions` warning from the macro. This is expected behavior from the library and cannot be suppressed in user code. The warning does not affect functionality.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- FFI-safe type layer complete: types (06-01) + trait/version (06-02)
- Ready for host infrastructure (phase 08) to implement plugin loading
- Ready for example plugin development to test the interface
- PluginModule provides clean library entry point pattern for plugin authors

---
*Phase: 06-ffi-safe-type-layer*
*Completed: 2026-01-24*
