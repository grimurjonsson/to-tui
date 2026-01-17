# Requirements: to-tui Clipboard Support

**Defined:** 2026-01-17
**Core Value:** Fast, keyboard-driven todo management that lives in the terminal and integrates with the tools I already use.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Clipboard

- [ ] **CLIP-01**: User can copy current todo text to system clipboard
- [ ] **CLIP-02**: User can press `y` key in Navigate mode to trigger copy
- [ ] **CLIP-03**: User sees status bar confirmation after successful copy
- [ ] **CLIP-04**: User sees error message when clipboard is unavailable

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Clipboard

- **CLIP-05**: User can copy multiple selected todos in Visual mode
- **CLIP-06**: User can copy with indentation hierarchy preserved
- **CLIP-07**: User can copy todo UUID for API/MCP integration
- **CLIP-08**: User sees brief highlight on yanked item (vim-highlightedyank style)
- **CLIP-09**: User can configure copy format (plain text, markdown, with checkbox)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Clipboard history / paste menu | System clipboard managers exist (clipse, CopyQ) |
| Internal yank registers (vim a-z) | Massive complexity for niche use |
| Paste from clipboard (Ctrl-V) | Ambiguous: paste where? As new todo? Into edit buffer? |
| Auto-copy on selection | Conflicts with Visual mode for other operations |
| Copy with ANSI colors | Garbage when pasted to plain text apps |
| OSC52 remote clipboard | Terminal-dependent, complex setup |
| Cmd-C / Ctrl-Shift-C keybinding | Terminal-dependent, unreliable; `y` is primary |

## Traceability

Which phases cover which requirements. Updated by create-roadmap.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLIP-01 | Phase 1 | Pending |
| CLIP-02 | Phase 1 | Pending |
| CLIP-03 | Phase 1 | Pending |
| CLIP-04 | Phase 1 | Pending |

**Coverage:**
- v1 requirements: 4 total
- Mapped to phases: 4 ✓
- Unmapped: 0

---
*Requirements defined: 2026-01-17*
*Last updated: 2026-01-17 after roadmap creation*
