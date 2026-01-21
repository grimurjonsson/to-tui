---
phase: quick
plan: 001
subsystem: docs
tags: [readme, installation, mcp]

# Dependency graph
requires:
  - phase: 05-automatic-self-upgrade
    provides: Auto-upgrade functionality that makes manual update instructions unnecessary
provides:
  - Simplified README.md focused on quick installation via curl command
  - Concise MCP configuration instructions
affects: [documentation, onboarding]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - README.md

key-decisions:
  - "Remove manual installation methods (From Source, Using Cargo) - auto-upgrade makes them unnecessary"
  - "Remove plugin marketplace instructions - obsolete approach"
  - "Use /usr/local/bin/totui-mcp as canonical path in MCP examples"

patterns-established: []

# Metrics
duration: 1min 20sec
completed: 2026-01-21
---

# Quick Task 001: Cleanup README - Simplify Install

**Streamlined README.md to show only curl installation and essential MCP configuration**

## Performance

- **Duration:** 1 min 20 sec
- **Started:** 2026-01-21T13:54:43Z
- **Completed:** 2026-01-21T13:56:03Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Installation section reduced to ~12 lines (from ~30 lines) - only curl command remains
- MCP section reduced to ~27 lines (from ~98 lines) - removed obsolete marketplace instructions
- All paths updated to /usr/local/bin/totui-mcp (installer's default location)

## Task Commits

Each task was committed atomically:

1. **Task 1: Simplify Installation Section** - `80941f9` (docs)
   - Removed "From Source" subsection
   - Removed "Using Cargo" subsection
   - Kept only curl command with installer explanation

2. **Task 2: Simplify MCP Server Section** - `854f114` (docs)
   - Removed "Plugin Marketplace" instructions (obsolete)
   - Removed "Updating the Plugin" section (auto-upgrade handles this)
   - Removed "Pre-built Binaries" section (installer handles this)
   - Removed "Local Development Setup" section (belongs in CONTRIBUTING)
   - Updated all paths to /usr/local/bin/totui-mcp
   - Simplified to essential claude mcp add commands and JSON example

## Files Created/Modified
- `README.md` - Simplified installation and MCP sections for faster onboarding

## Decisions Made
- Keep only the curl installation method - auto-upgrade functionality (from phase 05) makes manual installation alternatives unnecessary
- Remove plugin marketplace approach - this was an experimental approach that's now obsolete
- Use /usr/local/bin/totui-mcp consistently - this is where the installer places the binary by default

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- README is now more focused and easier to scan for new users
- Documentation accurately reflects the current installation and configuration approach
- No blockers or concerns

---
*Phase: quick*
*Completed: 2026-01-21*
