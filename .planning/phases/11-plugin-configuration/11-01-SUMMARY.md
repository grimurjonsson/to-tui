---
phase: 11-plugin-configuration
plan: 01
subsystem: plugin
tags: [ffi, config, toml, abi_stable, RHashMap]

# Dependency graph
requires:
  - phase: 10-metadata-database
    provides: Plugin trait and FFI types foundation
provides:
  - FFI-safe config types (FfiConfigValue, FfiConfigType, FfiConfigField, FfiConfigSchema)
  - Plugin config_schema() and on_config_loaded() trait methods
  - Host-side config loader with TOML validation
  - Plugin config path helpers for XDG directories
affects: [11-02 manager integration, 11-03 CLI commands, 12-15 plugin execution]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - FFI config enum pattern (FfiConfigValue with repr(C))
    - Schema-based TOML validation
    - Panic-safe plugin callbacks

key-files:
  created:
    - crates/totui-plugin-interface/src/config.rs
    - src/plugin/config.rs
  modified:
    - crates/totui-plugin-interface/src/lib.rs
    - crates/totui-plugin-interface/src/plugin.rs
    - src/plugin/mod.rs
    - src/utils/paths.rs

key-decisions:
  - "Schema via method not manifest - config_schema() on Plugin trait"
  - "Defaults in schema - FfiConfigField.default for host to apply"
  - "RHashMap for config values - natural key-value access across FFI"

patterns-established:
  - "FFI config types with StableAbi derive"
  - "Host validates before passing to plugin"
  - "Panic-safe wrapper for on_config_loaded()"

# Metrics
duration: 5min
completed: 2026-01-26
---

# Phase 11 Plan 01: Config Types and Host Loader Summary

**FFI-safe config types with StableAbi for plugin boundary, host-side TOML loader with schema validation**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-26T12:38:43Z
- **Completed:** 2026-01-26T12:43:18Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- FFI-safe config types (FfiConfigValue, FfiConfigType, FfiConfigField, FfiConfigSchema) with StableAbi
- Plugin trait extended with config_schema() and on_config_loaded() methods
- Host-side PluginConfigLoader validates TOML against schema with field-specific errors
- Path helpers for XDG plugin config directories (~/.config/to-tui/plugins/<name>/)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add FFI-safe config types to totui-plugin-interface** - `696e3f5` (feat)
2. **Task 2: Extend Plugin trait with config methods** - `46e5ffa` (feat)
3. **Task 3: Implement host-side config loader and path helper** - `9dba51c` (feat)

## Files Created/Modified

- `crates/totui-plugin-interface/src/config.rs` - FFI-safe config types with StableAbi
- `crates/totui-plugin-interface/src/lib.rs` - Export config types
- `crates/totui-plugin-interface/src/plugin.rs` - Plugin trait with config methods and panic-safe wrapper
- `src/plugin/config.rs` - PluginConfigLoader, ConfigValue, to_ffi_config()
- `src/plugin/mod.rs` - Export config module
- `src/utils/paths.rs` - get_plugin_config_dir() and get_plugin_config_path() helpers

## Decisions Made

- **Schema via Plugin trait method** - config_schema() returns FfiConfigSchema, not in manifest file, because schema is code and enables tooling like `totui plugin config <name>`
- **Defaults in schema** - FfiConfigField.default allows host to apply defaults before calling plugin
- **RHashMap for config values** - More natural key-value access than RVec of tuples

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Config types and loader ready for integration
- Next plan (11-02) will wire loader into PluginManager and call on_config_loaded()
- All tests pass, library builds clean

---
*Phase: 11-plugin-configuration*
*Completed: 2026-01-26*
