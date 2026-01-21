---
phase: 05-automatic-self-upgrade
plan: 02
subsystem: upgrade-integration
tags: [state-management, event-handling, async-polling, tui]

dependency-graph:
  requires:
    - phase: 05-01
      provides: [upgrade-types, download-function, progress-channel]
  provides:
    - upgrade-sub-state-tracking
    - download-event-handling
    - progress-polling
  affects: [05-03]

tech-stack:
  added: []
  patterns: [tokio-mpsc-try-recv, sub-state-pattern, event-loop-polling]

key-files:
  created: []
  modified:
    - src/app/state.rs
    - src/app/event.rs
    - src/ui/mod.rs

decisions:
  - id: sub-state-pattern
    choice: Use Option<UpgradeSubState> to track upgrade mode state
    rationale: Allows modal to exist in different states (Prompt, Downloading, Error, RestartPrompt) while Mode::UpgradePrompt is active

patterns-established:
  - "Async task communication: spawn_download() returns tokio::mpsc::Receiver, poll with try_recv() in event loop"
  - "Sub-state pattern: Option<SubState> field tracks modal state independently of Mode enum"

metrics:
  duration: 3 min
  completed: 2026-01-21
---

# Phase 5 Plan 2: TUI Integration Summary

Wired upgrade sub-states into AppState with download initiation, progress polling, and event handling for all sub-state transitions.

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-21T12:42:40Z
- **Completed:** 2026-01-21T12:45:50Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- AppState now tracks upgrade sub-state and download progress receiver
- Event handler supports all four sub-states: Prompt, Downloading, Error, RestartPrompt
- Download progress is polled every 100ms tick in the event loop

## Task Commits

1. **Task 1: Add upgrade sub-state tracking to AppState** - `cee2473` (feat)
2. **Task 2: Update event handling for upgrade mode sub-states** - `3ddf464` (feat)
3. **Task 3: Add download progress polling to event loop** - `4d50d09` (feat)

## Files Modified

| File | Changes |
|------|---------|
| src/app/state.rs | Added UpgradeSubState/DownloadProgress imports, upgrade_sub_state and download_progress_rx fields, start_download() and check_download_progress() methods |
| src/app/event.rs | Added UpgradeSubState import, rewrote handle_upgrade_prompt_mode() to handle all sub-states |
| src/ui/mod.rs | Added check_download_progress() call in event loop |

## What Was Built

### AppState Changes

**New Fields:**
- `upgrade_sub_state: Option<UpgradeSubState>` - Current sub-state during upgrade mode
- `download_progress_rx: Option<tokio_mpsc::Receiver<DownloadProgress>>` - Channel for download updates

**New Methods:**

**`start_download(&mut self)`**
- Gets version from `new_version_available`
- Constructs download URL via `get_asset_download_url()`
- Creates temp path: `/tmp/totui-{version}.tar.gz`
- Spawns download task and stores receiver
- Sets sub-state to `Downloading { progress: 0.0, bytes_downloaded: 0, total_bytes: None }`

**`check_download_progress(&mut self)`**
- Non-blocking poll of download_progress_rx via `try_recv()`
- Updates sub-state based on received message:
  - `Progress` -> Update Downloading fields
  - `Complete` -> Transition to RestartPrompt
  - `Error` -> Transition to Error sub-state
  - Channel disconnected -> Set Error "Download task crashed"

### Event Handling Changes

**handle_upgrade_prompt_mode() rewritten to handle sub-states:**

| Sub-State | Y | N/Esc | S | R |
|-----------|---|-------|---|---|
| Prompt | start_download() | dismiss_upgrade_session() | skip_version_permanently() | - |
| Downloading | - | Cancel download | - | - |
| Error | - | Dismiss error | - | Retry download |
| RestartPrompt | Placeholder (Plan 03) | Dismiss | - | - |

### Event Loop Integration

Added `state.check_download_progress()` call in the main loop, polled every ~100ms tick alongside other async checks (plugin_result, version_update, spinner).

## State Machine

```
                         Y pressed
[Mode::UpgradePrompt] ─────────────────► [UpgradeSubState::Downloading]
[UpgradeSubState::Prompt]                         │
                                                  │
                                 ┌────────────────┴────────────────┐
                                 │                                 │
                          DownloadProgress::                 DownloadProgress::
                              Complete                          Error
                                 │                                 │
                                 ▼                                 ▼
                    [UpgradeSubState::RestartPrompt]    [UpgradeSubState::Error]
                                 │                                 │
                          Y pressed                          R pressed
                          (Plan 03)                                │
                                 │                                 │
                                 ▼                                 ▼
                         extract_and_restart()              start_download()
```

## Decisions Made

- Used `Option<UpgradeSubState>` pattern so upgrade mode can have distinct UI states without adding more Mode enum variants
- Chose non-blocking `try_recv()` to avoid blocking the event loop while waiting for download progress

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 03 (Binary Replacement):
- `RestartPrompt { downloaded_path }` contains the downloaded tar.gz path
- Event handler Y press in RestartPrompt is a placeholder for extract/replace/restart
- Will need to implement:
  - Extract binary from tar.gz using self_update
  - Replace current binary
  - Signal restart or print instructions

---
*Phase: 05-automatic-self-upgrade*
*Plan: 02*
*Completed: 2026-01-21*
