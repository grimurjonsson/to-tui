# Roadmap: to-tui Clipboard Support

## Overview

Add clipboard support to to-tui, enabling users to copy todo text to the system clipboard with a single keypress (`y`). This is a focused enhancement to the existing TUI with well-understood implementation patterns.

## Phases

- [x] **Phase 1: Clipboard Support** - Implement `y` key to copy current todo text to system clipboard
- [x] **Phase 2: Scrolling & Mouse Support** - Add scrolling when text exceeds viewable area vertically, plus mouse support
- [x] **Phase 4: Claude Code Plugin Configuration** - Fix MCP server configuration to work with Claude Code's plugin/marketplace system
- [ ] **Phase 5: Automatic Self-Upgrade** - Download and install new versions automatically from upgrade prompt modal

## Phase Details

### Phase 1: Clipboard Support
**Goal**: User can copy todo text to system clipboard with `y` key
**Depends on**: Nothing (first phase)
**Requirements**: CLIP-01, CLIP-02, CLIP-03, CLIP-04
**Success Criteria** (what must be TRUE):
  1. User can press `y` in Navigate mode and selected todo text is copied to system clipboard
  2. Status bar shows "Copied: [todo text]" confirmation after successful copy
  3. Status bar shows error message when clipboard unavailable
  4. Copied text is plain text only (no checkbox, no markdown formatting)
**Research**: Complete (see .planning/research/)
**Plans**: 2 plans in 2 waves

Plans:
- [x] 01-01: Add arboard dependency and clipboard module (Wave 1)
- [x] 01-02: Implement copy action and keybinding (Wave 2)

### Phase 2: Scrolling & Mouse Support
**Goal**: Enable scrolling when content exceeds viewable area vertically, plus mouse interaction support
**Depends on**: Phase 1 (Clipboard Support)
**Requirements**: SCROLL-01, SCROLL-02, SCROLL-03, SCROLL-04
**Success Criteria** (what must be TRUE):
  1. Todo list scrolls when items exceed viewable area height
  2. User can scroll using keyboard (cursor movement) and mouse wheel
  3. Mouse clicks select/interact with correct item at any scroll position
  4. Scroll position indicator shows current position in list
**Research**: Level 0 (Skip) — uses existing ratatui ListState pattern
**Plans**: 3 plans in 2 waves

Plans:
- [x] 02-01: Add ListState scroll tracking and StatefulWidget rendering (Wave 1)
- [x] 02-02: Mouse scroll wheel and click offset handling (Wave 2)
- [x] 02-03: Scroll position indicator in title bar (Wave 2)

### Phase 3: Todo Priority System
**Goal**: Enable priority levels (P0/P1/P2) for todos with visual indicators and sorting capability
**Depends on**: Phase 2 (Scrolling & Mouse Support)
**Requirements**: PRIO-01, PRIO-02, PRIO-03, PRIO-04, PRIO-05
**Success Criteria** (what must be TRUE):
  1. Database stores priority levels (P0/P1/P2 or None)
  2. User can press `p` to cycle through priority levels (None -> P0 -> P1 -> P2 -> None)
  3. Existing `p` (plugin) binding moved to `P` (capital P)
  4. Priority is visually indicated in TUI (colored badge)
  5. User can press `s` to sort todos by priority (root todos first, then children recursively)
**Research**: Level 0 (Skip) — internal data model changes only
**Plans**: 4 plans in 3 waves

Plans:
- [x] 03-01: Priority data model - enum, TodoItem field, database & markdown persistence (Wave 1)
- [x] 03-02: Priority cycling keybinding - 'p' to cycle priority, 'P' for plugin menu (Wave 2)
- [x] 03-03: Priority visual display - colored badges in TUI (Wave 2)
- [x] 03-04: Sort by priority - 's' key to sort todos by priority level (Wave 3)

### Phase 4: Claude Code Plugin Configuration
**Goal**: Fix MCP server configuration to work with Claude Code's plugin/marketplace system per Anthropic documentation
**Depends on**: Phase 3 (Todo Priority System)
**Requirements**: CONFIG-01 (portable paths), CONFIG-02 (complete metadata), CONFIG-03 (installation docs)
**Success Criteria** (what must be TRUE):
  1. totui-mcp server is discoverable/configurable in Claude Code
  2. MCP tools (list_todos, create_todo, etc.) accessible from Claude Code sessions
  3. Configuration follows current Anthropic documentation patterns
  4. Installation/setup instructions updated
**Research**: Level 2 (Complete) — see .planning/phases/04-claude-code-plugin-configuration/04-RESEARCH.md
**Plans**: 3 plans in 2 waves

Plans:
- [x] 04-01: Update .mcp.json and plugin.json with portable paths and complete metadata (Wave 1)
- [x] 04-02: Verify marketplace.json and install-binary.sh consistency (Wave 1)
- [x] 04-03: Update README with correct Claude Code plugin installation commands (Wave 2)

### Phase 5: Automatic Self-Upgrade
**Goal**: When user accepts upgrade in the upgrade prompt modal, automatically download and install the new version with progress indication, then prompt for restart
**Depends on**: Phase 4 (Claude Code Plugin Configuration)
**Requirements**: UPGRADE-01 (download binary), UPGRADE-02 (progress bar), UPGRADE-03 (restart prompt), UPGRADE-04 (atomic upgrade)
**Success Criteria** (what must be TRUE):
  1. Pressing Y in upgrade modal starts automatic download of new release binary
  2. Progress bar shows download progress (if available from HTTP response)
  3. After download completes, modal shows "Installation ready, restart and upgrade? (Y/n)"
  4. Pressing Y at restart prompt exits program, replaces binary, and relaunches
  5. Pressing N at restart prompt dismisses modal without upgrading
  6. Download failures show error message and allow retry or dismiss
**Research**: Complete — see .planning/phases/05-automatic-self-upgrade/05-RESEARCH.md
**Plans**: 3 plans in 3 waves

Plans:
- [ ] 05-01: Add dependencies and create upgrade module with download infrastructure (Wave 1)
- [ ] 05-02: Integrate sub-states into AppState and event handling (Wave 2)
- [ ] 05-03: Complete UI rendering and binary replacement/restart (Wave 3)

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Clipboard Support | 2/2 | ✓ Complete | 2026-01-17 |
| 2. Scrolling & Mouse Support | 3/3 | ✓ Complete | 2026-01-17 |
| 3. Todo Priority System | 4/4 | ✓ Complete | 2026-01-19 |
| 4. Claude Code Plugin Configuration | 3/3 | ✓ Complete | 2026-01-20 |
| 5. Automatic Self-Upgrade | 0/3 | Not Started | - |
