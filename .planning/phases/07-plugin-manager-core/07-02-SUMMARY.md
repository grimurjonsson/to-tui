---
phase: 07-plugin-manager-core
plan: 02
subsystem: plugin
tags: [plugin-discovery, plugin-manager, version-compatibility, PLUG-06]

# Dependency graph
requires:
  - phase: 07-plugin-manager-core
    plan: 01
    provides: PluginManifest struct for parsing plugin.toml
provides:
  - PluginManager struct with discover(), list(), get() methods
  - PluginInfo struct tracking manifest, path, enabled, available, error
  - get_plugins_dir() utility function
  - Interface version compatibility check (PLUG-06)
affects: [07-03-plugin-enable-disable, 08-plugin-loader]

# Tech tracking
tech-stack:
  added: []
  patterns: [error-capture-pattern, version-compatibility-check]

key-files:
  created: [src/plugin/manager.rs]
  modified: [src/plugin/mod.rs, src/utils/paths.rs]

key-decisions:
  - "Error capture: parse errors stored in PluginInfo.error, not panicked"
  - "Missing plugins directory returns empty manager (graceful degradation)"
  - "Case-insensitive plugin name lookup via to_lowercase()"
  - "Availability separated from error: available=false for version mismatch, error for parse failures"

patterns-established:
  - "Plugin discovery: scan directory, load each plugin, capture errors per-plugin"
  - "Version compatibility: use totui_plugin_interface::is_version_compatible()"
  - "Two-level filtering: enabled_plugins() for active use, available_plugins() for UI listing"

# Metrics
duration: 3min
completed: 2026-01-24
---

# Phase 7 Plan 2: Plugin Discovery Summary

**PluginManager with discover() scanning ~/.local/share/to-tui/plugins/, interface version compatibility check (PLUG-06), and 9 unit tests**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-24T16:05:20Z
- **Completed:** 2026-01-24T16:08:39Z
- **Tasks:** 4
- **Files modified:** 3

## Accomplishments

- get_plugins_dir() function returns ~/.local/share/to-tui/plugins/ path using XDG data directory
- PluginInfo struct tracks manifest, path, enabled, available, availability_reason, error
- PluginManager::discover() scans plugins directory for plugin.toml files
- Parse errors captured in error field, not panicked (graceful error handling)
- Missing plugins directory returns empty manager (no crash)
- Interface version compatibility check using totui_plugin_interface::is_version_compatible()
- Incompatible plugins have available=false with detailed availability_reason
- Case-insensitive plugin name lookup via get()/get_mut()
- enabled_plugins() filters by enabled && available && no error
- available_plugins() shows all loadable plugins regardless of enabled state
- 9 unit tests covering all edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Add get_plugins_dir() to paths utility** - `a4848f9` (feat)
2. **Task 2: Create PluginInfo and PluginManager structs** - `cc4baec` (feat)
3. **Task 3: Add interface version compatibility check (PLUG-06)** - `6323896` (feat)
4. **Task 4: Add unit tests for PluginManager** - `b4cd82f` (test)

## Files Created/Modified

- `src/plugin/manager.rs` - PluginManager, PluginInfo structs, discover(), load_plugin_info(), 9 unit tests
- `src/plugin/mod.rs` - Added `pub mod manager;` and re-exports
- `src/utils/paths.rs` - Added get_plugins_dir() function and test

## Decisions Made

- Error capture: parse errors stored in PluginInfo.error field instead of returning Result
- Missing plugins directory returns empty manager (graceful degradation, no crash)
- Case-insensitive plugin name lookup via to_lowercase() on insert and lookup
- Availability separated from error: available=false for version mismatch (not a parse error), error field for parse/validation failures
- Two-level filtering: enabled_plugins() for plugins to actually use, available_plugins() for UI listing

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Borrow checker issue with min_interface_version: fixed by cloning the Option instead of borrowing it

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PluginManager ready for Phase 07-03 enable/disable functionality
- PluginInfo struct ready for loading plugins in Phase 08
- Interface version check (PLUG-06) complete and tested

---
*Phase: 07-plugin-manager-core*
*Completed: 2026-01-24*
