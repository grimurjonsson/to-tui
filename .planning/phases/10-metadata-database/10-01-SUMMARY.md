---
phase: 10-metadata-database
plan: 01
subsystem: database
tags: [sqlite, json, metadata, plugin, crud]

# Dependency graph
requires:
  - phase: 10-02
    provides: FfiCommand metadata variants and HostApi trait metadata methods
provides:
  - todo_metadata and project_metadata database tables
  - CRUD operations for plugin metadata storage
  - JSON validation with reserved key prefix rejection
  - Merge and replace modes for metadata updates
affects: [10-03, 11-plugin-testing, 14-jira-plugin]

# Tech tracking
tech-stack:
  added: []
  patterns: [json-merge-on-write, plugin-namespaced-storage]

key-files:
  created: [src/storage/metadata.rs]
  modified: [src/storage/database.rs, src/storage/mod.rs, src/plugin/host_impl.rs, src/plugin/command_executor.rs]

key-decisions:
  - "Metadata indexed by (entity_id, plugin_name) unique constraint"
  - "Return {} for non-existent metadata (not null/error)"
  - "Reserved key prefix _ rejected at validation layer"
  - "Merge mode uses shallow JSON merge (new keys overwrite existing)"

patterns-established:
  - "Plugin namespace isolation: each plugin sees only its own metadata"
  - "JSON validation before storage: check reserved keys, validate syntax"
  - "Empty object {} as null substitute for missing metadata"

# Metrics
duration: 6min
completed: 2026-01-26
---

# Phase 10 Plan 01: Database Schema & Storage Layer Summary

**Plugin metadata storage layer with todo_metadata/project_metadata tables and CRUD operations using (entity_id, plugin_name) namespacing**

## Performance

- **Duration:** 6 min
- **Started:** 2026-01-26T11:29:37Z
- **Completed:** 2026-01-26T11:36:02Z
- **Tasks:** 3 (Task 3 merged into Task 2)
- **Files modified:** 5

## Accomplishments

- Created todo_metadata and project_metadata tables with proper indexes
- Implemented 6 CRUD functions for metadata storage with JSON validation
- Wired HostApi metadata methods to storage layer for immediate usability
- 12 unit tests covering all key behaviors including edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Add metadata table schema to init_database** - `04362f2` (feat)
2. **Task 2+3: Create metadata.rs CRUD module with tests** - `103ae21` (feat)

## Files Created/Modified

- `src/storage/metadata.rs` - Metadata CRUD operations (set/get/delete for todo and project)
- `src/storage/database.rs` - Schema initialization for metadata tables
- `src/storage/mod.rs` - Added metadata module export
- `src/plugin/host_impl.rs` - HostApi metadata method implementations
- `src/plugin/command_executor.rs` - Handle metadata FfiCommand variants (no-op)

## Decisions Made

- **Empty object for missing metadata**: Return `{}` instead of error/null for non-existent metadata - cleaner plugin code, no error handling needed
- **Shallow merge**: Merge mode overwrites at key level, not deep merge - simpler semantics, sufficient for typical metadata use cases
- **Reserved prefix at storage layer**: Validate `_` prefix rejection in storage, not FFI - single point of validation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Handle metadata FfiCommand variants in CommandExecutor**
- **Found during:** Task 1 (build failed)
- **Issue:** FfiCommand enum has metadata variants from 10-02, but CommandExecutor didn't handle them
- **Fix:** Added match arms for metadata commands (no-op since they're storage-layer operations)
- **Files modified:** src/plugin/command_executor.rs
- **Verification:** Build passes
- **Committed in:** 04362f2 (Task 1 commit)

**2. [Rule 3 - Blocking] Implement HostApi metadata methods in PluginHostApiImpl**
- **Found during:** Task 2 (build failed)
- **Issue:** HostApi trait has metadata methods from 10-02, but PluginHostApiImpl didn't implement them
- **Fix:** Added implementations calling metadata.rs CRUD functions
- **Files modified:** src/plugin/host_impl.rs
- **Verification:** Build passes, all tests pass
- **Committed in:** 103ae21 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary to compile. The FFI interface (10-02) was committed before storage (10-01), so implementing the trait methods was required for build success. This correctly wires the storage layer to the HostApi.

## Issues Encountered

- Tests require `--test-threads=1` due to HOME env var modification - acceptable for test isolation, could add serial_test crate later

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Storage layer complete with full CRUD operations
- HostApi already wired to storage (done as blocking fix)
- Plan 10-02 can be skipped (FFI interface already exists and is now functional)
- Plan 10-03 may have reduced scope since HostApi wiring is complete

---
*Phase: 10-metadata-database*
*Completed: 2026-01-26*
