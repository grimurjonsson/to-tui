---
phase: 08-dynamic-loading
plan: 01
subsystem: plugin
tags: [abi_stable, ffi, panic-safety, plugin-loader]

# Dependency graph
requires:
  - phase: 07-plugin-manager-core
    provides: PluginManager with discovery and version checking
provides:
  - PluginLoader struct for loading native plugins via abi_stable
  - LoadedPlugin proxy pattern with 'static lifetime
  - PluginErrorKind enum for categorized error handling
  - call_safely() for panic-catching FFI calls
  - Session-based plugin disabling after panics
affects: [09-plugin-ui-integration, 10-loading-feedback]

# Tech tracking
tech-stack:
  added: [tracing-appender]
  patterns: [proxy-pattern-via-library-leak, panic-catch-at-ffi-boundary]

key-files:
  created: [src/plugin/loader.rs]
  modified: [src/plugin/mod.rs, Cargo.toml, crates/totui-plugin-interface/src/lib.rs]

key-decisions:
  - "Use abi_stable's library leaking as the proxy pattern implementation"
  - "Map LibraryError variants to user-friendly PluginErrorKind categories"
  - "Session-disabled plugins return SessionDisabled error on subsequent calls"

patterns-established:
  - "call_safely() wraps all plugin method calls in catch_unwind"
  - "Plugin panics are logged via tracing with forced backtrace capture"
  - "Case-insensitive plugin lookup via to_lowercase() on names"

# Metrics
duration: 5min
completed: 2026-01-25
---

# Phase 08 Plan 01: Plugin Loader Infrastructure Summary

**PluginLoader with abi_stable integration, panic-catching call_safely(), and session-based disabling for crashed plugins**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-25T10:24:37Z
- **Completed:** 2026-01-25T10:29:59Z
- **Tasks:** 2
- **Files modified:** 4 (+ 8 pre-existing clippy fixes)

## Accomplishments

- Created PluginLoader struct that loads plugins via abi_stable's load_from_directory
- Implemented panic-catching call_safely() with automatic session disabling
- Categorized loading errors into VersionMismatch, LibraryCorrupted, SymbolMissing, etc.
- Added tracing-appender for panic logging with forced backtrace capture

## Task Commits

Each task was committed atomically:

1. **Task 1: Create PluginLoader with abi_stable loading and error types** - `77449d8` (feat)
2. **Task 2: Add panic-safe plugin calling with session disabling** - `77f4fbe` (test)

## Files Created/Modified

- `src/plugin/loader.rs` - PluginLoader struct with load_all(), call_safely(), call_generate()
- `src/plugin/mod.rs` - Added loader module and re-exports
- `Cargo.toml` - Added tracing-appender dependency
- `crates/totui-plugin-interface/src/lib.rs` - Added allow for abi_stable macro warning

**Additional clippy fixes (pre-existing issues):**
- `src/api/handlers.rs` - Added allow for result_large_err
- `src/app/event.rs` - Replaced manual div_ceil with div_ceil()
- `src/app/state.rs` - Added allow for too_many_arguments, collapsed if
- `src/main.rs` - Collapsed if, push_str to push
- `src/storage/migration.rs` - map_or to is_some_and
- `src/todo/item.rs` - Added allow for too_many_arguments
- `src/ui/components/mod.rs` - Collapsed if, vec_init_then_push fixes

## Decisions Made

1. **Proxy pattern via library leaking:** abi_stable intentionally leaks libraries (never unloads) to guarantee plugins outlive all trait objects, avoiding TLS destructor issues. This IS the proxy pattern.
2. **Error categorization:** LibraryError variants are mapped to user-friendly categories - OpenError/GetSymbolError become "corrupted or incompatible", IncompatibleVersionNumber shows version requirement.
3. **Session disable on panic:** When call_safely() catches a panic, the plugin is marked session_disabled=true and all subsequent calls return SessionDisabled error.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing RootModule trait import**
- **Found during:** Task 1
- **Issue:** load_from_directory() requires RootModule trait in scope
- **Fix:** Added RootModule to abi_stable imports
- **Files modified:** src/plugin/loader.rs
- **Committed in:** 77449d8 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed IncompatibleVersionNumber field names**
- **Found during:** Task 1
- **Issue:** Used wrong field name (library_version vs expected_version/actual_version)
- **Fix:** Updated to correct field names from abi_stable API
- **Files modified:** src/plugin/loader.rs
- **Committed in:** 77449d8 (Task 1 commit)

**3. [Rule 1 - Bug] Fixed pre-existing clippy warnings**
- **Found during:** Task 1 verification
- **Issue:** Multiple clippy warnings in codebase (map_or, div_ceil, collapsible_if, etc.)
- **Fix:** Applied clippy suggestions across multiple files
- **Files modified:** 8 files
- **Committed in:** 77449d8 (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (2 bugs, 1 blocking)
**Impact on plan:** All fixes necessary for compilation and lint compliance. No scope creep.

## Issues Encountered

None - execution proceeded smoothly after fixing the blocking and bug issues.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PluginLoader is ready for integration with TUI
- call_safely() and call_generate() provide safe interfaces for calling plugins
- Error types are ready for UI display (version mismatch messages, corruption messages)
- Session disabling prevents repeated crashes from panicked plugins

---
*Phase: 08-dynamic-loading*
*Completed: 2026-01-25*
