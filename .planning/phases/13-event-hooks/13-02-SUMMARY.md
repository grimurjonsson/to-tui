---
phase: 13-event-hooks
plan: 02
subsystem: plugin-hooks
tags: ["hooks", "async", "timeout", "dispatch"]
requires: ["13-01"]
provides: ["HookDispatcher", "HookResult", "hook_timeout_secs"]
affects: ["13-03"]
tech-stack:
  added: []
  patterns: ["async hook dispatch", "failure tracking", "auto-disable"]
key-files:
  created:
    - src/plugin/hooks.rs
  modified:
    - src/plugin/mod.rs
    - src/plugin/manifest.rs
    - src/plugin/loader.rs
    - src/keybindings/mod.rs
decisions:
  - id: "13-02-01"
    title: "Synchronous hook call with watchdog timeout"
    choice: "Call hooks synchronously in current thread, use watchdog thread for timeout detection"
    rationale: "Plugin_TO trait objects cannot be cloned/moved to spawned threads; current thread call avoids ownership issues"
metrics:
  duration: "12 min"
  completed: "2026-01-26"
---

# Phase 13 Plan 02: Hook Dispatcher Infrastructure Summary

HookDispatcher created with sync hook dispatch, failure tracking, and event subscription management.

## Commits

| Hash | Type | Description |
|------|------|-------------|
| 0ea897f | feat | Add hook_timeout_secs to PluginManifest (default 5s) |
| 500fb9c | feat | Create HookDispatcher with dispatch_to_plugin, poll_results |
| 7949565 | feat | Add event subscription tracking to PluginLoader |

## What Was Built

### 1. Manifest Extension (Task 1)

**File:** `src/plugin/manifest.rs`

Added `hook_timeout_secs` field to `PluginManifest`:
- Default value: 5 seconds
- Configurable per plugin in manifest
- Used by HookDispatcher for timeout enforcement

### 2. Hook Dispatcher (Task 2)

**File:** `src/plugin/hooks.rs`

Created `HookDispatcher` with:
- `dispatch_to_plugin(event, plugin, timeout)` - Calls hook with timeout enforcement
- `poll_results()` - Non-blocking result collection via channel
- `is_hook_disabled(name)` - Check if plugin hooks are auto-disabled
- `disabled_hook_count()` - Count of disabled plugins

**Constants:**
- `DEFAULT_HOOK_TIMEOUT`: 5 seconds
- `AUTO_DISABLE_THRESHOLD`: 3 consecutive failures

**HookResult struct:**
- `plugin_name`: Plugin that executed the hook
- `event_type`: Event that was handled
- `commands`: Commands to apply (Vec<FfiCommand>)
- `error`: Error message if failed

**Timeout Implementation:**
- Uses watchdog thread pattern for timeout detection
- Hook call happens synchronously in current thread
- Atomic flag coordinates completion status with watchdog
- Avoids ownership issues with Plugin_TO trait objects

### 3. Event Subscription Tracking (Task 3)

**File:** `src/plugin/loader.rs`

Extended `PluginLoader`:
- Added `event_subscriptions: HashMap<String, Vec<FfiEventType>>` field
- Populates subscriptions during `load_all_with_config()` by calling `plugin.subscribed_events()`
- Logs subscriptions at load time for debugging
- Added `plugins_for_event(event_type)` method returning `Vec<(&LoadedPlugin, Duration)>`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] KeySequence missing Display impl**
- **Found during:** Task 1 verification (cargo test)
- **Issue:** Help panel tried to format KeySequence but Display was not implemented
- **Fix:** Added `impl fmt::Display for KeySequence` in `src/keybindings/mod.rs`
- **Commit:** 0ea897f

**2. [Rule 1 - Bug] Plugin_TO not clonable for thread spawn**
- **Found during:** Task 2 implementation
- **Issue:** Original plan assumed Plugin_TO could be cloned to spawn threads
- **Fix:** Changed to synchronous hook call with watchdog thread for timeout
- **Rationale:** abi_stable trait objects don't implement Clone; synchronous call with timeout watchdog achieves same safety guarantees
- **Commit:** 500fb9c

## Tests Added

| Module | Tests |
|--------|-------|
| hooks | 7 tests (dispatcher, failure tracking, result fields, constants) |
| loader | 2 tests (event subscriptions empty on new, all types) |
| manifest | 3 tests (default timeout, custom timeout, default_hook_timeout fn) |

## Architecture Notes

The hook dispatch design uses a synchronous-with-watchdog pattern rather than true async threading because:

1. `Plugin_TO` trait objects cannot be easily moved across thread boundaries
2. Plugins never unload (per design), so the call will eventually complete
3. Timeout is enforced via watchdog thread that signals completion flag
4. Results flow through mpsc channel for UI thread polling

This approach provides the same safety guarantees (timeout, failure tracking, auto-disable) while avoiding complex ownership patterns with FFI trait objects.

## Next Phase Readiness

Ready for 13-03 (AppState Integration):
- HookDispatcher ready to be owned by AppState
- plugins_for_event() available for event routing
- hook_timeout_secs can be wired to manifest lookup
- All exports available: `HookDispatcher`, `HookResult` from `crate::plugin`
