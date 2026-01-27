---
phase: 15-migration
plan: 02
subsystem: plugin
tags: [plugin-loader, generator, migration, ffi]

# Dependency graph
requires:
  - phase: 15-01
    provides: jira-claude external plugin crate
  - phase: 08
    provides: PluginLoader with call_generate
provides:
  - Empty PluginRegistry (legacy, backwards compat)
  - LoadedPlugin with version and description fields
  - TUI/CLI using PluginLoader for plugin listing and execution
affects: [15-03, ui-plugin-menu, cli-generate]

# Tech tracking
tech-stack:
  added: []
  patterns: [external-plugins-only]

key-files:
  created: []
  modified:
    - src/plugin/mod.rs
    - src/plugin/loader.rs
    - src/app/state.rs
    - src/app/event.rs
    - src/main.rs

key-decisions:
  - "Remove plugin_registry field from AppState entirely"
  - "Call plugins synchronously instead of spawning thread"
  - "Add description field alongside version for UI display"

patterns-established:
  - "Plugin listing: Always use plugin_loader.loaded_plugins(), not PluginRegistry"
  - "Plugin execution: Use plugin_loader.call_generate() for generate workflow"

# Metrics
duration: 9min
completed: 2026-01-26
---

# Phase 15 Plan 02: Remove Built-in Generator Summary

**Removed built-in Jira generator, empty PluginRegistry for legacy compat, LoadedPlugin now stores version/description from manifest**

## Performance

- **Duration:** 9 min
- **Started:** 2026-01-26T23:44:59Z
- **Completed:** 2026-01-26T23:54:00Z
- **Tasks:** 3 (Task 0: verify, Task 1: remove generator, Task 1.5: add version)
- **Files modified:** 6 (plus 2 deleted)

## Accomplishments

- Deleted src/plugin/generators/ directory (jira_claude.rs, mod.rs)
- Made PluginRegistry empty (returns empty Vec, legacy backwards compat)
- Added version and description fields to LoadedPlugin struct
- Updated TUI plugin menu to use PluginLoader instead of PluginRegistry
- Updated CLI `generate` command to use PluginManager + PluginLoader
- Removed unused plugin_registry field from AppState
- Cleaned up unused execute_plugin_with_host method and imports

## Task Commits

Each task was committed atomically:

1. **Task 0: Verify 15-01 CODE completion** - (no commit, verification only)
2. **Task 1: Remove built-in Jira generator** - `05ac8bd` (refactor)
3. **Task 1.5: Add version field to LoadedPlugin** - `9dd7a86` (feat)

## Files Created/Modified

- `src/plugin/generators/jira_claude.rs` - DELETED (built-in Jira generator)
- `src/plugin/generators/mod.rs` - DELETED (generators module)
- `src/plugin/mod.rs` - Removed generators module, empty PluginRegistry
- `src/plugin/loader.rs` - Added version and description to LoadedPlugin
- `src/app/state.rs` - Removed plugin_registry, updated plugin menu
- `src/app/event.rs` - Use PluginLoader for plugin execution
- `src/main.rs` - Updated generate command, removed plugin_registry

## Decisions Made

- **Remove plugin_registry entirely:** Since PluginRegistry is now empty and unused, removed the field from AppState rather than keeping dead code
- **Synchronous plugin calls:** Changed from spawning a thread with PluginRegistry to calling plugin_loader.call_generate() directly - simpler and plugins are already loaded
- **Add description to LoadedPlugin:** Plan only specified version, but UI needs description for plugin menu display - added both fields together

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Update TUI and CLI to use PluginLoader instead of PluginRegistry**
- **Found during:** Task 1 (Remove built-in Jira generator)
- **Issue:** After making PluginRegistry empty, code wouldn't compile - TUI and CLI were calling .generate() on PluginRegistry results
- **Fix:** Updated open_plugin_menu() and handle_plugin_input() to use plugin_loader, updated handle_generate() CLI to use PluginManager + PluginLoader
- **Files modified:** src/app/state.rs, src/app/event.rs, src/main.rs
- **Verification:** cargo check passes, tests pass
- **Committed in:** 05ac8bd (Task 1 commit)

**2. [Rule 2 - Missing Critical] Add description field to LoadedPlugin**
- **Found during:** Task 1.5 (Add version field)
- **Issue:** Plan specified version field, but TUI plugin menu also needs description for display
- **Fix:** Added description field alongside version, populated from manifest in load_plugin()
- **Files modified:** src/plugin/loader.rs
- **Verification:** cargo check passes
- **Committed in:** 9dd7a86 (Task 1.5 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both fixes necessary for correctness. No scope creep - description was needed for existing UI functionality.

## Issues Encountered

- Pre-existing test failures in storage::metadata::tests (readonly database errors) - unrelated to this plan, did not affect execution

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Built-in generators completely removed
- External plugins are now the only source of generators
- jira-claude plugin exists in to-tui-plugins repo (from 15-01)
- Ready for 15-03: TUI plugin mode integration

---
*Phase: 15-migration*
*Completed: 2026-01-26*
