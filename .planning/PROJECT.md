# to-tui

## What This Is

A terminal-based todo list manager with vim-style keybindings, built in Rust. Provides a TUI for daily task management with markdown file storage, SQLite archival, REST API for integrations, MCP server for LLM tooling, and automatic self-upgrade.

## Core Value

Fast, keyboard-driven todo management that lives in the terminal and integrates with the tools I already use.

## Current State

**Version:** v1.0 (shipped 2026-01-21)
**Codebase:** 10,823 lines of Rust
**Tech stack:** Rust 2024, ratatui/crossterm TUI, SQLite, axum REST API, rmcp MCP server

**Recent additions:**
- Clipboard support (`y` to copy)
- Scrolling with mouse wheel support
- Priority system (P0/P1/P2) with colored badges
- Sort by priority (`s` key)
- Claude Code plugin configuration
- Automatic self-upgrade with progress bar

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
- ✓ Clipboard support with `y` key — v1.0
- ✓ Scrolling with mouse wheel — v1.0
- ✓ Priority system (P0/P1/P2) with visual badges — v1.0
- ✓ Sort by priority — v1.0
- ✓ Automatic self-upgrade — v1.0

### Active

**Milestone v2.0: Plugin Framework**

- [ ] Dynamic plugin loading via libloading (.so/.dylib/.dll)
- [ ] Plugin trait with lifecycle hooks (init, shutdown)
- [ ] Plugin capability: create/manipulate todos
- [ ] Plugin capability: query database (read-only)
- [ ] Plugin capability: add custom metadata to todos/projects
- [ ] Plugin capability: register custom keybindings
- [ ] Plugin registry with manifest format
- [ ] Local plugin directory (~/.config/to-tui/plugins/)
- [ ] GitHub repo plugin source (grimurjonsson/to-tui-plugins default)
- [ ] Plugin auto-download and version management
- [ ] Refactor existing Jira generator to new plugin system

### Out of Scope

- Cloud sync — local-first design is intentional
- Mobile app — terminal-focused tool
- Collaboration features — single-user design
- Clipboard history / paste menu — system clipboard managers exist
- Internal yank registers (vim a-z) — massive complexity for niche use
- UI theming via plugins — deferred to v2.1+
- Claude Code skill bundling in plugins — deferred to v2.1+
- Any-language plugins via IPC — deferred to v2.1+

## Constraints

- **Tech stack**: Rust 2024 edition, ratatui/crossterm for TUI
- **Compatibility**: Must work on macOS, Linux, Windows
- **Keybindings**: Must respect existing vim-style patterns
- **No external deps**: Prefer crates that don't require system clipboard daemons

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Copy text only (no checkbox/hierarchy) | User preference — clean text for pasting | ✓ Good |
| Use `y` key for copy (vim yank) | Avoid Ctrl-C terminal interrupt conflicts | ✓ Good |
| Use arboard with wayland-data-control | Linux Wayland support | ✓ Good |
| Priority badge [P0]/[P1]/[P2] format | Clear, compact, colored foreground | ✓ Good |
| Sort preserves children under parent | Intuitive hierarchy behavior | ✓ Good |
| std::thread for download (not tokio) | TUI runs without tokio runtime | ✓ Good |
| Restore terminal before exec() | exec() doesn't run Drop handlers | ✓ Good |
| Download raw binaries (not tar.gz) | Matches actual GitHub release format | ✓ Good |

---
*Last updated: 2026-01-24 after v2.0 milestone start*
