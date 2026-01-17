# Project Research Summary

**Project:** to-tui clipboard support
**Domain:** Rust TUI clipboard integration
**Researched:** 2026-01-17
**Confidence:** HIGH

## Executive Summary

Adding clipboard support to to-tui is a straightforward feature with a well-established solution. The **arboard** crate (maintained by 1Password) is the clear choice for cross-platform clipboard access, with over 21M downloads and active development. The integration fits naturally into to-tui's existing action-dispatch architecture.

The recommended approach is to use `y` (vim-style yank) as the primary keybinding rather than Cmd-C/Ctrl-C, which have platform-specific complications. The existing status bar infrastructure handles user feedback perfectly. Implementation complexity is LOW.

Key risk: Linux clipboard behavior differs from macOS/Windows (data vanishes on app exit). Mitigation: Keep clipboard instance alive in AppState, recommend clipboard managers to Linux users.

## Key Findings

### Recommended Stack

Use `arboard` with Wayland feature flag:

```toml
arboard = { version = "3.6", features = ["wayland-data-control"] }
```

**Core technologies:**
- **arboard 3.6.1**: Cross-platform clipboard — 1Password maintained, simple API, most recent release August 2025
- No other dependencies needed — integrates directly with existing ratatui/crossterm stack

### Expected Features

**Must have (table stakes):**
- Single-item copy to system clipboard — core use case
- Visual feedback on copy success — use existing `set_status_message()`
- Copy plain text only (no checkbox/markdown) — user specified
- Graceful failure on clipboard errors — status bar message

**Should have (competitive):**
- `y` key to yank (vim convention) — familiar, single keypress, always works
- Visual mode multi-select copy — join selected items with newlines

**Defer (v2+):**
- Highlight yanked item briefly (vim-highlightedyank style)
- Configurable copy format options
- Copy todo UUID for API/MCP integration

### Architecture Approach

Clipboard operations follow the existing action-dispatch pattern. A new `Action::CopyItem` routes through the keybinding system to `AppState.copy_current_item_to_clipboard()`.

**Major components:**
1. `Action::CopyItem` in `src/keybindings/mod.rs` — defines the action
2. `copy_current_item_to_clipboard()` in `src/app/state.rs` — formats and copies
3. `Option<arboard::Clipboard>` in `AppState` — lazily-initialized, kept alive

### Critical Pitfalls

1. **Ctrl-C vs SIGINT conflict** — Use `y` as primary keybinding; terminals expect Ctrl-C to cancel, not copy
2. **Linux clipboard vanishes on exit** — Keep Clipboard instance in AppState, not local variable
3. **Cmd key undetectable on macOS** — Most terminals can't detect Command key; don't rely on Cmd-C
4. **Wayland requires feature flag** — Enable `wayland-data-control` in Cargo.toml

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Core Clipboard Implementation
**Rationale:** Single focused phase is sufficient — low complexity, well-understood pattern
**Delivers:** Working `y` keybinding to copy current todo text
**Addresses:** All table stakes features
**Avoids:** Ctrl-C/Cmd-C pitfalls by using vim-style keybinding

**Implementation steps:**
1. Add arboard dependency with wayland feature
2. Add `Action::CopyItem` to keybindings enum
3. Add default `y` key mapping in `default_navigate_bindings()`
4. Add `clipboard: Option<arboard::Clipboard>` to `AppState`
5. Implement `copy_current_item_to_clipboard()` method
6. Handle action in `execute_navigate_action()`
7. Test on macOS, Linux (X11/Wayland), Windows

### Phase Ordering Rationale

Single phase is appropriate because:
- Feature is self-contained with no dependencies on other features
- All infrastructure already exists (status bar, keybindings, action dispatch)
- Complexity is LOW — estimated 2-4 hours implementation

### Research Flags

**Standard patterns (skip research-phase):**
- This phase uses established patterns from existing keybinding/action code
- arboard API is trivial: `Clipboard::new()?.set_text("text")?`
- No additional research needed before planning

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | arboard is ecosystem standard, verified via multiple sources |
| Features | MEDIUM | UX expectations based on vim/lazygit observation |
| Architecture | HIGH | Follows existing patterns exactly |
| Pitfalls | HIGH | Well-documented in arboard/crossterm docs |

**Overall confidence:** HIGH

### Gaps to Address

- Exact behavior of Cmd-C in user's terminal (document as terminal-dependent)
- Visual mode multi-copy can be deferred to follow-on enhancement

## Sources

### Primary (HIGH confidence)
- [arboard GitHub](https://github.com/1Password/arboard) — Version 3.6.1, August 2025
- [arboard docs.rs](https://docs.rs/arboard/latest/arboard/) — API reference
- [crossterm event module](https://docs.rs/crossterm/latest/crossterm/event/) — Keyboard handling

### Secondary (MEDIUM confidence)
- [ratatui-code-editor](https://crates.io/crates/ratatui-code-editor) — Verified arboard + ratatui integration
- [lazygit keybindings](https://github.com/jesseduffield/lazygit/blob/master/docs/keybindings/) — TUI copy patterns

---
*Research completed: 2026-01-17*
*Ready for roadmap: yes*
