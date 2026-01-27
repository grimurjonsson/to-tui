---
phase: 11-plugin-configuration
plan: 02
subsystem: plugin
tags: [config, cli, loader, toml, validation, ffi]

# Dependency graph
requires:
  - phase: 11-01
    provides: FFI config types, PluginConfigLoader, path helpers
provides:
  - Config-aware plugin loading with validation
  - CLI commands for validate and config --init
  - Config errors surfaced in TUI error popup
affects: [12-15 plugin execution, plugin authors]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Config validation during plugin load
    - Config error conversion for unified popup display
    - TOML template generation from schema

key-files:
  created: []
  modified:
    - src/plugin/loader.rs
    - src/plugin/mod.rs
    - src/plugin/config.rs
    - src/cli.rs
    - src/main.rs
    - src/app/state.rs

key-decisions:
  - "ConfigError struct separate from PluginLoadError for clean separation"
  - "Convert ConfigError to PluginLoadError for unified popup display"
  - "Store config_errors in PluginLoader for retrieval after loading"

patterns-established:
  - "Config validation at plugin load time"
  - "Template generation from schema for plugin config --init"
  - "Config errors logged with 'config = true' context"

# Metrics
duration: 8min
completed: 2026-01-26
---

# Phase 11 Plan 02: PluginLoader Integration and CLI Commands Summary

**Config validation integrated into plugin loading with CLI commands for validate and config --init, config errors surfaced in TUI popup**

## Performance

- **Duration:** 8 min
- **Started:** 2026-01-26T13:15:00Z
- **Completed:** 2026-01-26T13:23:00Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- ConfigError struct for config validation failures with Display/Error traits
- load_all_with_config() validates config against schema before adding plugin
- CLI `totui plugin validate <name>` validates config without starting TUI
- CLI `totui plugin config <name> --init` creates config directory and template
- Config errors converted to PluginLoadError for unified TUI popup display
- Template generation includes descriptions, types, required/optional markers

## Task Commits

Each task was committed atomically:

1. **Task 1: Integrate config loading into PluginLoader** - `89c5127` (feat)
2. **Task 2: Add CLI commands for validate and config --init** - `f7f9e80` (feat)
3. **Task 3: Surface config errors in TUI error popup** - `25bac17` (feat)

## Files Created/Modified

- `src/plugin/loader.rs` - ConfigError struct, load_all_with_config(), get_config_errors()
- `src/plugin/mod.rs` - Export ConfigError
- `src/plugin/config.rs` - generate_config_template() for TOML template generation
- `src/cli.rs` - Validate and Config subcommands in PluginCommand
- `src/main.rs` - handle_plugin_validate(), handle_plugin_config(), TUI config error integration
- `src/app/state.rs` - Tests for config errors in pending_plugin_errors

## Decisions Made

- **ConfigError separate from PluginLoadError** - Clean separation between load failures and config failures, easier to log differently
- **Convert to PluginLoadError for popup** - Reuse existing popup infrastructure, unified user experience
- **Store config_errors in PluginLoader** - Allows retrieval after loading for debugging and testing

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plugin configuration system complete
- Plugins can define schemas, host validates config at load time
- Users can bootstrap config files with `totui plugin config <name> --init`
- Ready for Phase 12: Plugin UI Integration

---
*Phase: 11-plugin-configuration*
*Completed: 2026-01-26*
