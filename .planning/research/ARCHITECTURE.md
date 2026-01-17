# Architecture Research: Clipboard Integration

**Domain:** Clipboard integration in ratatui TUI
**Researched:** 2026-01-17
**Confidence:** HIGH

## Summary

Clipboard integration in a ratatui/crossterm TUI requires an external crate since neither ratatui nor crossterm provide built-in clipboard support. The `arboard` crate (maintained by 1Password) is the standard choice for cross-platform clipboard access. The integration fits naturally into the existing action-dispatch architecture already present in to-tui.

**Primary recommendation:** Add clipboard operations as new `Action` variants, handle them in `app/event.rs` alongside existing actions, and use a lazily-initialized `arboard::Clipboard` instance managed by `AppState`.

## Integration Point

### Where Clipboard Handling Belongs

Based on analysis of the existing codebase architecture:

1. **Action definitions** (`src/keybindings/mod.rs`): Add `CopyItem` action to the `Action` enum
2. **Keybinding defaults** (`src/keybindings/mod.rs`): Map `y` key to `copy_item` in navigate mode
3. **Action execution** (`src/app/event.rs`): Handle `Action::CopyItem` in `execute_navigate_action()`
4. **State management** (`src/app/state.rs`): Add clipboard instance and format method to `AppState`
5. **User feedback** (`src/ui/components/status_bar.rs`): Already supports status messages via `set_status_message()`

The existing architecture follows an action-dispatch pattern where:
- Keybindings map to `Action` enum variants
- `handle_navigate_mode()` looks up actions via `KeybindingCache`
- `execute_navigate_action()` performs the actual operation
- Status feedback uses `state.set_status_message()`

Clipboard operations should follow this exact same pattern.

### Data Flow

```
[User presses 'y' in Navigate mode]
    |
    v
[handle_key_event()] in app/event.rs
    |
    v
[handle_navigate_mode()] looks up key in KeybindingCache
    |
    v
[KeyLookupResult::Action(Action::CopyItem)]
    |
    v
[execute_navigate_action(Action::CopyItem, state)]
    |
    v
[state.copy_current_item_to_clipboard()] in app/state.rs
    |  - Get selected item content
    |  - Format as markdown: "- [ ] Task content"
    |  - Call arboard::Clipboard::set_text()
    v
[state.set_status_message("Copied to clipboard")]
    |
    v
[status_bar.rs renders message for 3 seconds]
```

### Recommended Module Placement

| Component | Location | Responsibility |
|-----------|----------|----------------|
| `Action::CopyItem` | `src/keybindings/mod.rs` | Define the copy action variant |
| Key mapping `y` | `src/keybindings/mod.rs` | Default binding in `default_navigate_bindings()` |
| `copy_current_item_to_clipboard()` | `src/app/state.rs` | Format item and write to system clipboard |
| Action handler | `src/app/event.rs` | Route action to state method |
| Clipboard instance | `src/app/state.rs` | Lazily-initialized `Option<arboard::Clipboard>` field |

### Clipboard Instance Management

The `arboard::Clipboard` struct should be:

1. **Lazily initialized** - Create only when first copy operation occurs
2. **Stored in AppState** - Avoids repeated initialization overhead
3. **Wrapped in Option** - Handle platforms where clipboard is unavailable

```rust
// In AppState struct
pub clipboard: Option<arboard::Clipboard>,

// In copy method
pub fn copy_current_item_to_clipboard(&mut self) -> bool {
    let item = match self.selected_item() {
        Some(item) => item,
        None => return false,
    };

    let text = format_item_as_markdown(item);

    // Lazy init clipboard
    if self.clipboard.is_none() {
        self.clipboard = arboard::Clipboard::new().ok();
    }

    if let Some(ref mut clipboard) = self.clipboard {
        if clipboard.set_text(&text).is_ok() {
            self.set_status_message("Copied to clipboard".to_string());
            return true;
        }
    }

    self.set_status_message("Clipboard unavailable".to_string());
    false
}
```

## Pattern Recommendations

### Follow Existing Action Pattern

The codebase already has a well-established pattern for actions:

1. **Define action** in `Action` enum with `#[serde(rename_all = "snake_case")]`
2. **Add Display/FromStr** implementations for the action
3. **Add default keybinding** in `default_navigate_bindings()`
4. **Handle in mode handler** - match arm in `execute_navigate_action()`
5. **Implement on AppState** - method that performs the operation

This pattern is used for all existing actions (ToggleState, Delete, Undo, etc.) and clipboard should follow it exactly.

### Readonly Mode Handling

The current architecture has a `dominated_by_readonly` check for write operations. Copy is a read operation and should NOT be blocked in readonly mode:

```rust
// In execute_navigate_action()
let dominated_by_readonly = matches!(
    action,
    Action::ToggleState
        | Action::CycleState
        | Action::Delete
        // ... other write actions
        // NOTE: Action::CopyItem is NOT included - copy works in readonly mode
);
```

This allows users to copy items from archived/historical views.

### Error Handling

The `arboard` crate returns `Result<(), Error>` for set operations. Errors should:

1. **Not crash the app** - Clipboard failure is non-fatal
2. **Provide feedback** - Use status message to inform user
3. **Log for debugging** - Use `tracing::warn!` for diagnostics

### Platform Considerations

| Platform | Notes |
|----------|-------|
| macOS | Works out of the box |
| Linux X11 | Works with default arboard backend |
| Linux Wayland | May need `wayland-data-control` feature |
| Windows | Works but avoid concurrent clipboard access |

For MVP, default arboard configuration is sufficient. Wayland-specific support can be added later if users report issues.

## Dependency Addition

Add to `Cargo.toml`:

```toml
[dependencies]
arboard = "3"  # Current stable version
```

No feature flags needed for basic text clipboard support.

## Anti-Patterns to Avoid

### Do NOT create clipboard per operation

```rust
// BAD: Creates new clipboard instance every time
fn copy_item(&self) {
    let clipboard = arboard::Clipboard::new().unwrap();
    clipboard.set_text("text").unwrap();
}
```

This wastes resources and can cause issues on some platforms.

### Do NOT use internal yank buffer approach

Some TUI editors (like tui-textarea) use internal yank buffers instead of system clipboard. This is NOT appropriate for to-tui because:

1. Users expect `y` (yank) to copy to system clipboard
2. The use case is sharing todo items with other applications
3. Internal buffers don't persist across app sessions

### Do NOT block on clipboard operations

On Linux, clipboard operations can involve IPC. Use non-blocking patterns:

```rust
// GOOD: Fire-and-forget with error handling
if clipboard.set_text(&text).is_err() {
    self.set_status_message("Clipboard unavailable".to_string());
}

// BAD: Would block the event loop
clipboard.set().wait();  // Don't use wait() in TUI event loop
```

## Testing Considerations

Clipboard operations are inherently platform-specific and difficult to test in CI. Recommended approach:

1. **Unit test formatting logic** - The markdown formatting function is pure and testable
2. **Integration test structure** - Test that copy action exists and routes correctly
3. **Manual testing** - Platform-specific clipboard behavior verified manually

## Sources

### Primary (HIGH confidence)
- [arboard documentation](https://docs.rs/arboard/latest/arboard/struct.Clipboard.html) - Official API reference
- [arboard GitHub](https://github.com/1Password/arboard) - Platform-specific notes and best practices
- Existing codebase analysis - `src/app/event.rs`, `src/keybindings/mod.rs`, `src/app/state.rs`

### Secondary (MEDIUM confidence)
- [ratatui component architecture](https://ratatui.rs/concepts/application-patterns/component-architecture/) - Action handling patterns
- [tui-textarea](https://github.com/rhysd/tui-textarea) - Reference for how other TUI widgets handle clipboard (uses internal yank buffer, which we explicitly avoid)

### Additional References
- [ratatui-code-editor](https://crates.io/crates/ratatui-code-editor) - Example of ratatui widget with clipboard integration
- [Turbo Vision for Rust](https://github.com/aovestdipaperino/turbo-vision-4-rust) - TUI framework using arboard for OS clipboard

## Metadata

**Confidence breakdown:**
- Integration point: HIGH - Follows existing patterns exactly
- arboard usage: HIGH - Well-documented, maintained by 1Password
- Platform support: MEDIUM - Linux Wayland may need feature flag

**Research date:** 2026-01-17
**Valid until:** 60 days (stable patterns, stable crate)
