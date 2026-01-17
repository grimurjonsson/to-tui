# Feature Research

**Domain:** Terminal clipboard UX
**Researched:** 2026-01-17
**Confidence:** MEDIUM

## Summary

Terminal clipboard operations in vim-style TUI apps follow established conventions from vim's yank/put system, but with important differences for system clipboard integration. The to-tui project already has the infrastructure needed (status bar with timed messages, modal state machine, keybinding system) to implement clipboard support cleanly.

**Key insight:** Users expect instant visual feedback when copying. The existing `status_message` mechanism with 3-second timeout is appropriate for copy confirmation.

## Feature Landscape

### Table Stakes (Users Expect These)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Single-item copy to system clipboard | Core use case - user wants todo text elsewhere | LOW | Use `arboard` or `cli-clipboard` crate |
| Visual feedback on copy success | Vim shows "X lines yanked", lazygit shows toast | LOW | Already have `set_status_message()` with green highlight |
| Copy current line content only (no checkbox) | User specified; matches "clean paste" expectation | LOW | Just `item.content.clone()` |
| Cmd-C on macOS / Ctrl-C alternative on Linux | Platform convention for copy | MEDIUM | See technical notes below |
| Graceful failure on clipboard errors | Wayland/X11 can fail; SSH sessions problematic | LOW | Show error in status bar |

### Differentiators (Nice to Have)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| `y` key to yank (vim convention) | Familiar to vim users, single keypress | LOW | Add to keybindings as `Action::Copy` |
| Visual mode multi-select copy | Copy multiple todos at once | MEDIUM | Already have Visual mode; join with newlines |
| Copy with hierarchy (indented text) | Preserve structure when pasting | LOW | Format indentation as spaces/tabs |
| Copy todo ID (UUID) | Useful for API/MCP integration | LOW | Different keybinding e.g., `yid` |
| Highlight yanked item briefly | Vim-highlightedyank style feedback | MEDIUM | Requires render state for timed highlight |
| Configurable copy format | User chooses: plain text, markdown, with checkbox | HIGH | Config file option |

### Anti-Features (Don't Implement)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Clipboard history / paste menu | Power users want multiple clipboards | Scope creep; system clipboard managers exist (clipse, CopyQ) | Let OS handle history |
| Internal yank registers (vim a-z) | Full vim emulation | Massive complexity for niche use; vim users have vim | Single system clipboard |
| Paste from clipboard (Ctrl-V) | Symmetry with copy | Ambiguous: paste as new todo? Into edit buffer? Where in hierarchy? | Only support paste in Edit mode (already works via terminal) |
| Auto-copy on selection | Some terminals do this | Conflicts with Visual mode for other operations (indent, delete) | Explicit copy action only |
| Copy with ANSI colors | Preserve styling | Garbage when pasted to plain text apps | Plain text only |
| OSC52 remote clipboard | SSH clipboard sync | Terminal-dependent, complex setup; out of scope | Document as user responsibility |

## UX Conventions

### Vim/Terminal Clipboard Patterns

**Vim's model:**
- `y` = yank (copy) to internal register
- `"+y` = yank to system clipboard (+ register)
- `yy` = yank current line
- Visual mode `y` = yank selection

**Simplified for TUI apps:**
- Most TUI apps skip internal registers entirely
- Copy goes directly to system clipboard
- Single keybinding (not chords like `"+y`)
- Lazygit uses `y` and `Ctrl-o` for different copy operations

**Recommended for to-tui:**
- `y` in Navigate mode = copy current todo content to system clipboard
- `y` in Visual mode = copy selected todos (newline-separated)
- Cmd-C / Ctrl-Shift-C as alternative (platform-dependent)

### Feedback Patterns

**What vim does:**
- Text feedback: "1 line yanked" in command line
- Modern vim/neovim: Brief highlight of yanked region (via plugin or built-in)
- Duration: ~500-1000ms for highlight

**What lazygit does:**
- Toast message in status area
- Shows "Copied to clipboard" or similar
- Disappears after ~2-3 seconds

**What to-tui should do:**
- Use existing `set_status_message()` mechanism
- Message: "Copied: {truncated content}" or "Copied {n} items"
- Green background (already implemented for status messages)
- 3-second timeout (already implemented)
- On error: Show error message (e.g., "Clipboard unavailable")

**Example flow:**
1. User presses `y` in Navigate mode
2. Status bar turns green with "Copied: Buy groceries..."
3. After 3 seconds, returns to normal status bar

### Keyboard Shortcut Considerations

**The Ctrl-C problem:**
- In terminals, Ctrl-C sends SIGINT (interrupt signal)
- TUI apps in raw mode can intercept this, but users expect Ctrl-C to interrupt
- Most terminal apps use Ctrl-Shift-C for copy instead

**The Cmd-C problem (macOS):**
- Cmd key (Super) detection requires `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`
- Many terminals don't properly support Cmd key combinations
- Known crossterm issue: Cmd+key often doesn't trigger events

**Recommended approach:**
1. Primary: `y` key (vim convention, always works)
2. Secondary: Ctrl-Shift-C (if terminal supports it)
3. Document that Cmd-C depends on terminal emulator support
4. Do NOT intercept plain Ctrl-C (let it send SIGINT for expected behavior)

## Technical Notes

### Rust Clipboard Libraries

| Library | Wayland | X11 | macOS | Windows | Notes |
|---------|---------|-----|-------|---------|-------|
| `arboard` (1Password) | Yes* | Yes | Yes | Yes | *Requires `wayland-data-control` feature |
| `cli-clipboard` | Yes | Yes | Yes | Yes | Fork of rust-clipboard with Wayland support |
| `terminal-clipboard` | Yes | Yes | Yes | Yes | Focused on terminal apps |

**Recommendation:** Use `arboard` with appropriate features. Well-maintained by 1Password.

### Integration Points in to-tui

1. **Keybindings** (`src/keybindings.rs`): Add `Action::CopyToClipboard`
2. **Event handling** (`src/app/event.rs`): Handle action in navigate/visual modes
3. **Status bar** (`src/ui/components/status_bar.rs`): Already supports timed messages
4. **AppState** (`src/app/state.rs`): Already has `set_status_message()`

### Error Cases to Handle

- Clipboard unavailable (Wayland compositor not running, X11 not available)
- SSH session without OSC52 support
- Permission denied
- Empty selection (nothing to copy)

## Complexity Assessment

| Implementation | Effort | Risk |
|----------------|--------|------|
| Basic `y` to copy current item | 2-4 hours | Low |
| Status bar feedback | Already done | None |
| Visual mode multi-copy | 1-2 hours | Low |
| Cmd-C/Ctrl-Shift-C support | 2-4 hours | Medium (terminal-dependent) |
| Highlight yanked item | 4-8 hours | Medium (render complexity) |

## Sources

### Primary (HIGH confidence)
- [Ratatui GitHub](https://github.com/ratatui/ratatui) - TUI framework docs
- [arboard GitHub](https://github.com/1Password/arboard) - Clipboard library
- [cli-clipboard GitHub](https://github.com/allie-wake-up/cli-clipboard) - Alternative clipboard library
- [terminal-clipboard GitHub](https://github.com/Canop/terminal-clipboard) - Terminal-focused clipboard

### Secondary (MEDIUM confidence)
- [vim-highlightedyank](https://github.com/machakann/vim-highlightedyank) - Vim yank feedback patterns
- [lazygit keybindings](https://github.com/jesseduffield/lazygit/blob/master/docs/keybindings/Keybindings_en.md) - TUI copy patterns
- [crossterm KeyModifiers](https://docs.rs/crossterm/latest/crossterm/event/struct.KeyModifiers.html) - Modifier key handling
- [Alacritty copy mode](https://wiki.archlinux.org/title/Alacritty) - Terminal emulator conventions

### Tertiary (LOW confidence - WebSearch only)
- [NN/G UI Copy Guidelines](https://www.nngroup.com/articles/ui-copy/) - General UX patterns
- [Neovim clipboard docs](https://ofirgall.github.io/learn-nvim/chapters/04-copy-paste-visual.html) - Register conventions
