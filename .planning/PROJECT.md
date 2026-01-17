# to-tui

## What This Is

A terminal-based todo list manager with vim-style keybindings, built in Rust. Provides a TUI for daily task management with markdown file storage, SQLite archival, REST API for integrations, and MCP server for LLM tooling.

## Core Value

Fast, keyboard-driven todo management that lives in the terminal and integrates with the tools I already use.

## Requirements

### Validated

- ✓ TUI with vim-style navigation (j/k, gg/G, etc.) — existing
- ✓ Todo states: empty, done, question, important, in-progress — existing
- ✓ Markdown file persistence (~/.to-tui/dailies/YYYY-MM-DD.md) — existing
- ✓ SQLite database for archival and querying — existing
- ✓ Daily rollover (copy incomplete items to new day) — existing
- ✓ Hierarchical todos with parent-child relationships — existing
- ✓ Undo/redo with 50-state history — existing
- ✓ REST API server for external integrations — existing
- ✓ MCP server for LLM integration — existing
- ✓ Customizable keybindings via config.toml — existing
- ✓ Plugin system for external todo generators (Jira) — existing
- ✓ Cross-platform builds (macOS, Linux, Windows) — existing

### Active

- [ ] Clipboard support: Cmd-C/Ctrl-C to copy current todo text to system clipboard

### Out of Scope

- Cloud sync — local-first design is intentional
- Mobile app — terminal-focused tool
- Collaboration features — single-user design

## Context

This is a mature Rust project with clean layered architecture:
- Domain layer (`todo/`) with pure business logic
- Storage layer with dual persistence (markdown + SQLite)
- Multiple interface adapters (TUI, REST API, MCP)
- Modal state machine for vim-like interaction

The TUI uses ratatui + crossterm. Keybindings are configurable and processed through a `KeybindingCache`. Adding clipboard support requires:
1. A clipboard crate dependency
2. Keybinding for Cmd-C/Ctrl-C
3. Handler to copy selected todo content

## Constraints

- **Tech stack**: Rust 2024 edition, ratatui/crossterm for TUI
- **Compatibility**: Must work on macOS, Linux, Windows
- **Keybindings**: Must respect existing vim-style patterns
- **No external deps**: Prefer crates that don't require system clipboard daemons

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Copy text only (no checkbox/hierarchy) | User preference — clean text for pasting elsewhere | — Pending |

---
*Last updated: 2026-01-17 after initialization*
