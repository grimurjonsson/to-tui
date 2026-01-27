---
phase: 08-dynamic-loading
plan: 02
subsystem: tui-integration
tags: [plugin-loading, error-handling, startup, ui-popup]
requires: ["08-01"]
provides: ["plugin-error-display", "tui-plugin-integration", "startup-plugin-loading"]
affects: ["09", "10"]
tech-stack:
  added: []
  patterns: ["overlay-popup-pattern", "event-interception"]
key-files:
  created: []
  modified:
    - src/app/state.rs
    - src/ui/components/mod.rs
    - src/main.rs
    - src/ui/mod.rs
decisions:
  - decision: "test-only-methods"
    rationale: "Methods for future phases (handle_plugin_panic, plugin_loader_mut) marked #[cfg(test)] to avoid dead_code warnings"
    alternatives: ["#[allow(dead_code)]", "remove-until-needed"]
  - decision: "popup-dismissal-interception"
    rationale: "Plugin error popup dismisses on any key by intercepting in event loop before handle_key_event"
    alternatives: ["mode-based-handling", "dedicated-key-binding"]
metrics:
  duration: "6m43s"
  completed: "2026-01-25"
---

# Phase 08 Plan 02: TUI Plugin Integration Summary

Plugin loading integrated into TUI startup with error popup display and key dismissal.

## What Was Built

### Plugin Loading State (src/app/state.rs)
- Added `plugin_loader: PluginLoader` field to hold loaded plugin instances
- Added `pending_plugin_errors: Vec<PluginLoadError>` for error display
- Added `show_plugin_error_popup: bool` to control popup visibility
- Added `dismiss_plugin_error_popup()` method for user dismissal
- Added `loaded_plugin_count()` method to query loaded plugins
- Added `handle_plugin_panic()` for future runtime error handling (test-only for now)
- Added `plugin_loader_mut()` accessor for future plugin calls (test-only for now)
- Updated `AppState::new()` signature to accept plugin_loader and plugin_errors

### Plugin Error Popup UI (src/ui/components/mod.rs)
- Added `render_plugin_error_popup()` function
- Centered overlay at 65% width, height based on error count
- Shows count of failed plugins with header
- Lists each plugin name and error message
- Includes hint: "Run `totui plugin status` for details"
- Yellow "Press any key to dismiss" footer
- Red border to indicate error state

### TUI Startup Integration (src/main.rs)
- Discovers plugins via `PluginManager::discover()`
- Applies config with `plugin_manager.apply_config(&config.plugins)`
- Creates `PluginLoader` and calls `load_all()` to load enabled plugins
- Logs warnings for loading errors via tracing
- Passes `plugin_loader` and `plugin_errors` to AppState
- Logs successful plugin count after state creation

### Event Loop Integration (src/ui/mod.rs)
- Added check for `show_plugin_error_popup` in key event handling
- Calls `dismiss_plugin_error_popup()` on any key press
- Uses `continue` to consume the key event (not passed to other handlers)

## Key Implementation Details

### Error Display Flow
1. At startup, `PluginLoader::load_all()` returns Vec<PluginLoadError>
2. Errors passed to AppState, `show_plugin_error_popup` set if non-empty
3. First render shows error popup overlay
4. Any key dismisses popup but errors remain in `pending_plugin_errors`
5. User can run `totui plugin status` for detailed error info

### Popup Rendering Pattern
Followed existing overlay patterns (rollover modal, upgrade prompt):
- Center with Layout constraints
- Clear background with Clear widget
- Build lines with styled Spans
- Wrap in Block with colored border
- Render via Paragraph with wrap enabled

## Commits

| Hash | Description |
|------|-------------|
| eeffc24 | Add plugin loading state to AppState |
| 3de2c55 | Add plugin error popup UI component |
| c19a181 | Integrate plugin loading into TUI startup |

## Files Changed

| File | Changes |
|------|---------|
| src/app/state.rs | +39 lines (fields, methods, tests) |
| src/ui/components/mod.rs | +91 lines (render_plugin_error_popup) |
| src/main.rs | +20 lines (plugin loading integration) |
| src/ui/mod.rs | +5 lines (popup dismissal) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added test coverage for new methods**
- **Found during:** Task 3
- **Issue:** New methods handle_plugin_panic and plugin_loader_mut needed tests
- **Fix:** Added test_handle_plugin_panic and test_plugin_loader_mut tests
- **Files modified:** src/app/state.rs

**2. [Rule 3 - Blocking] Resolved dead_code clippy warnings**
- **Found during:** Task 3
- **Issue:** Clippy warned about unused plugin_loader field and methods
- **Fix:** Added loaded_plugin_count() used in main.rs, marked future methods #[cfg(test)]
- **Files modified:** src/app/state.rs, src/main.rs

## Verification

All verification commands passed:
- `cargo build --release` - Compiles successfully
- `cargo test --lib` - 158 tests pass
- `cargo test` - 164 tests pass (includes 6 binary tests)
- `cargo clippy -- -D warnings` - No warnings

## Next Phase Readiness

Phase 08 is complete with this plan. Ready for Phase 09 (Generate Workflow Integration):
- PluginLoader is available in AppState for calling plugin.generate()
- Error handling infrastructure is in place for runtime panics
- Plugin error popup can display runtime errors
- call_safely() from Phase 08-01 ready for use in generate workflow
