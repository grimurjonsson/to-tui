# Roadmap: to-tui Clipboard Support

## Overview

Add clipboard support to to-tui, enabling users to copy todo text to the system clipboard with a single keypress (`y`). This is a focused enhancement to the existing TUI with well-understood implementation patterns.

## Phases

- [ ] **Phase 1: Clipboard Support** - Implement `y` key to copy current todo text to system clipboard

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
**Research**: Unlikely (research completed, patterns established)
**Plans**: TBD

Plans:
- [ ] 01-01: Add arboard dependency and clipboard module
- [ ] 01-02: Implement copy action and keybinding

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Clipboard Support | 0/2 | Not started | - |
