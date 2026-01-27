---
phase: 15-migration
plan: 01
subsystem: plugin
tags: [plugin, jira, claude, ffi, cdylib, abi-stable]

# Dependency graph
requires:
  - phase: 06-ffi-types
    provides: FFI-safe types and Plugin trait
  - phase: 07-plugin-management
    provides: PluginLoader and manifest parsing
provides:
  - jira-claude plugin crate implementing Plugin trait
  - to-tui-plugins registry repository structure
  - Multi-platform release workflow
affects: [15-migration, future-plugins]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Plugin implementation with export_root_module macro"
    - "Subprocess execution for external CLI tools"
    - "FfiTodoItem conversion from domain logic"

key-files:
  created:
    - jira-claude/Cargo.toml
    - jira-claude/src/lib.rs
    - jira-claude/plugin.toml
    - jira-claude/README.md
    - jira-claude/CHANGELOG.md
    - LICENSE
    - README.md
    - marketplace.toml
    - .github/workflows/release.yml
  modified: []

key-decisions:
  - "Use local path dependency for development (git reference for release)"
  - "Subprocess execution via std::process::Command"
  - "No config schema (plugin requires no configuration)"
  - "No event subscriptions (generator-only plugin)"

patterns-established:
  - "Plugin export pattern: export_root_module + PluginModule + create_plugin"
  - "Error handling: convert Result to RResult for FFI boundary"
  - "Todo generation: return FfiTodoItem vec with root + children"

# Metrics
duration: 5min
completed: 2026-01-26
---

# Phase 15 Plan 01: Create jira-claude Plugin Crate Summary

**First external plugin in to-tui-plugins registry implementing Plugin trait for Jira ticket todo generation via acli and Claude CLI**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-26T23:36:27Z
- **Completed:** 2026-01-26T23:42:00Z
- **Tasks:** 3
- **Files created:** 9

## Accomplishments

- Created jira-claude plugin crate with FFI-safe Plugin trait implementation
- Ported existing jira_claude.rs generator logic to standalone plugin
- Set up to-tui-plugins registry repository structure
- Created multi-platform GitHub Actions release workflow
- Established plugin development patterns for future plugins

## Task Commits

Each task was committed atomically:

1. **Task 1: Create jira-claude plugin crate** - `3f7379b` (feat)
2. **Task 2: Create documentation and registry files** - `bafd3dd` (docs)
3. **Task 3: Create GitHub Actions release workflow** - `f81082b` (chore)

_Note: Commits are in the to-tui-plugins repository, not to-tui_

## Files Created

In `grimurjonsson/to-tui-plugins` repository:

- `jira-claude/Cargo.toml` - Plugin crate configuration with cdylib output
- `jira-claude/src/lib.rs` - Plugin implementation with FFI exports
- `jira-claude/plugin.toml` - Plugin manifest with permissions
- `jira-claude/README.md` - Usage documentation and examples
- `jira-claude/CHANGELOG.md` - Version 0.1.0 release notes
- `LICENSE` - MIT license for repository
- `README.md` - Registry documentation and plugin listing
- `marketplace.toml` - Plugin registry manifest (placeholder for CI)
- `.github/workflows/release.yml` - Multi-platform build and release workflow

## Decisions Made

1. **Local path dependency for development** - The totui-plugin-interface crate isn't pushed to GitHub yet (Phase 6-14 work is local). Using local path for verification; will update to git reference when plugin framework is pushed.

2. **Direct subprocess execution** - Uses std::process::Command directly instead of a separate module, keeping the plugin self-contained.

3. **No configuration schema** - Plugin requires no user configuration; authentication handled by underlying acli and claude CLI tools.

4. **Generator-only pattern** - Implements generate() method; execute_with_host() returns empty vec; no event subscriptions.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed TD_Opaque import path**
- **Found during:** Task 1 (cargo check)
- **Issue:** `abi_stable::type_level::TD_Opaque` path incorrect
- **Fix:** Changed to `abi_stable::sabi_trait::TD_Opaque`
- **Files modified:** jira-claude/src/lib.rs
- **Committed in:** 3f7379b (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor import path correction. No scope creep.

## Issues Encountered

**Plugin interface not available on GitHub**

The totui-plugin-interface crate exists locally but hasn't been pushed to the GitHub repository. The v0.4.0 tag predates the plugin framework development (Phases 6-14). Currently using a local path dependency for verification.

**Resolution:** For the plugin to be buildable by GitHub Actions or external users, the plugin framework must be pushed to GitHub. This is a prerequisite for Phase 15-02 (first release tag).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready for Phase 15-02:** TUI integration to remove built-in jira generator

**Blocker for release:** The totui-plugin-interface must be pushed to GitHub before:
- GitHub Actions can build the plugin
- Users can install jira-claude via `totui plugin install`

**Recommendation:** Push current development (Phases 6-14) to main branch or a v2.0-dev branch before proceeding with plugin release.

---
*Phase: 15-migration*
*Completed: 2026-01-26*
