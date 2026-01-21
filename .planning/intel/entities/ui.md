# Terminal UI

**Directory**: `src/ui/`

## Purpose

Ratatui-based terminal user interface with vim-style keybindings, real-time rendering, and file watching for external changes.

## Components

### mod.rs - Main Loop

```rust
pub fn run_tui(state: AppState) -> Result<AppState> {
    // Terminal setup (raw mode, alternate screen, mouse capture)
    // Event loop: render → poll → handle → save
    // Terminal cleanup on exit
}
```

Key features:
- `TerminalGuard` - RAII cleanup on panic/exit
- Database file watcher (notify crate) for live reload
- 100ms poll interval for spinner animation

### theme.rs

```rust
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub question: Color,      // Yellow
    pub exclamation: Color,   // Red
    pub in_progress: Color,   // Cyan
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub priority_p0: Color,   // Red
    pub priority_p1: Color,   // Yellow/orange
    pub priority_p2: Color,   // Blue
}
```

Themes: `default`, `dark`, `light`

### components/mod.rs

Main render function and overlay renderers:
- `render(frame, state)` - Main UI layout
- Help overlay (? key)
- Plugin modal
- Rollover dialog
- Upgrade progress

### components/todo_list.rs

Todo list widget:
- Checkbox rendering with state symbols
- Indent visualization
- Text wrapping for long content
- Collapse/expand for nested items
- Description display
- Priority badges
- Selection highlighting

### components/status_bar.rs

Bottom status bar showing:
- Current mode
- Date (today/archived)
- Item count
- Readonly indicator
- Unsaved changes indicator
- Version info + upgrade available
