---
phase: 02-scrolling-mouse-support
plan: 02
subsystem: ui
tags: [ratatui, mouse, scroll, event-handling]

# Dependency graph
requires:
  - phase: 02-01
    provides: ListState-based scroll tracking with sync_list_state()
provides:
  - Mouse scroll wheel navigation (3 items per scroll)
  - Scroll-offset-aware click-to-item mapping
affects: [02-03, mouse-interactions, future-mouse-features]

# Tech tracking
tech-stack:
  added: []
  patterns: [scroll-offset-aware click mapping]

key-files:
  created: []
  modified:
    - src/app/event.rs

key-decisions:
  - "Scroll wheel moves cursor by 3 items using existing navigation methods"
  - "Scroll events allowed in readonly mode for viewing archived dates"
  - "Click mapping skips scrolled-past items before counting visual rows"

patterns-established:
  - "Scroll offset accounting: use list_state.offset() to skip items above viewport"
  - "Mouse scroll = repeated cursor movement to keep cursor/scroll in sync"

# Metrics
duration: 3min
completed: 2026-01-17
---

# Phase 2 Plan 02: Mouse Scroll and Click Selection Summary

**Mouse scroll wheel navigation and scroll-offset-aware click-to-item mapping for correct selection at any scroll position**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-17T16:00:00Z
- **Completed:** 2026-01-17T16:03:00Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added mouse scroll wheel support (ScrollUp/ScrollDown events)
- Scroll moves cursor by 3 items, keeping cursor and view synchronized
- Scroll works in readonly mode for viewing archived dates
- Fixed click-to-item mapping to account for scroll offset

## Task Commits

Each task was committed atomically:

1. **Task 1: Handle mouse scroll wheel events** - `3e0c34e` (feat)
2. **Task 2: Fix click-to-item mapping with scroll offset** - `37f1490` (feat)

## Files Created/Modified
- `src/app/event.rs` - Added ScrollUp/ScrollDown handling, scroll-offset-aware click mapping

## Decisions Made
- Treat scroll as repeated cursor movement (3 iterations) to keep cursor and scroll in sync
- Allow scroll in readonly mode but block click interactions (users should be able to browse archived dates)
- Track visible_item_count separately from visual_row when mapping clicks to account for scrolled-past items

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Mouse scroll and click work correctly at any scroll position
- Ready for 02-03 (scroll position indicators) if not already completed
- Foundation complete for additional mouse interactions

---
*Phase: 02-scrolling-mouse-support*
*Completed: 2026-01-17*
