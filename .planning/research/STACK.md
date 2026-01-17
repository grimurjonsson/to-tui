# Stack Research: Rust TUI Clipboard Integration

**Domain:** Rust TUI clipboard integration
**Researched:** 2026-01-17
**Confidence:** HIGH

## Summary

Cross-platform clipboard support in Rust TUI applications is well-served by a mature ecosystem. The clear winner is **arboard**, maintained by 1Password, which provides a simple API for text and image clipboard operations across macOS, Windows, and Linux (X11/Wayland).

For to-tui's use case (copying todo text to system clipboard with Cmd-C/Ctrl-C), arboard provides exactly what's needed: a simple `set_text()` API that works cross-platform with no external daemon requirements on macOS/Windows. On Linux, clipboard data persists as long as the TUI app is running (which is the expected use case).

**Primary recommendation:** Use `arboard = "3.6"` with the `wayland-data-control` feature for full Linux support.

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| arboard | 3.6.1 | System clipboard access | 1Password-maintained, simple API, cross-platform, 21M+ downloads, active development (Aug 2025 release) |

### Cargo.toml Addition

```toml
arboard = { version = "3.6", features = ["wayland-data-control"] }
```

**Feature rationale:**
- `wayland-data-control`: Enables native Wayland clipboard support. Without this, Linux users on pure Wayland (increasingly common in 2025+) won't have clipboard access. The feature falls back to X11 automatically if Wayland isn't available.

### API Usage

```rust
use arboard::Clipboard;

// Create clipboard instance
let mut clipboard = Clipboard::new()?;

// Copy text (accepts &str, String, or Cow<str>)
clipboard.set_text("Todo text to copy")?;

// Read text (if needed later)
let text = clipboard.get_text()?;
```

The API is straightforward and error-handling aligns with anyhow::Result patterns already used in to-tui.

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| arboard 3.6 | copypasta 0.10.2 | Only if you need Alacritty-ecosystem compatibility or already use smithay-clipboard |
| arboard 3.6 | clipboard-rs 0.3.1 | Only if you need iOS/Android support (both in beta/WIP) |
| arboard 3.6 | cli-clipboard | Never - wrapper around arboard with less maintenance |

### Detailed Alternative Analysis

**copypasta 0.10.2** (Alacritty team)
- Pros: Battle-tested in Alacritty terminal, good Wayland support
- Cons: Different API pattern (trait-based ClipboardProvider), less active than arboard
- Last updated: April 2025
- Decision: arboard's simpler API and 1Password backing makes it the better choice

**clipboard-rs 0.3.1**
- Pros: Supports file lists, RTF, custom types, change monitoring
- Cons: Newer with less ecosystem adoption, mobile support incomplete
- Last updated: November 2025
- Decision: Over-engineered for to-tui's text-only needs

**clipboard (rust-clipboard) 0.5.0**
- Unmaintained (arboard is its successor fork)
- Do not use

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| clipboard (rust-clipboard) | Unmaintained since arboard forked | arboard |
| cli-clipboard | Thin wrapper with no added value | arboard directly |
| copypasta-ext | Extensions for copypasta, not needed | arboard |
| Manual platform-specific code | Reinventing the wheel, error-prone | arboard |

## Integration Notes

### ratatui/crossterm Compatibility

arboard has no conflicts with ratatui or crossterm. They operate at different levels:
- crossterm: Terminal I/O and raw mode
- ratatui: UI rendering
- arboard: System clipboard (separate from terminal)

**Verified integration:** The `ratatui-code-editor` crate (a syntax-highlighted code editor widget) uses exactly this stack: ratatui + crossterm + arboard.

### Key Pattern for TUI Apps

```rust
// In your event handler (e.g., src/app/event.rs)
use arboard::Clipboard;

fn handle_copy(todo_text: &str) -> anyhow::Result<()> {
    let mut clipboard = Clipboard::new()
        .with_context(|| "Failed to access system clipboard")?;
    clipboard.set_text(todo_text)
        .with_context(|| "Failed to copy to clipboard")?;
    Ok(())
}
```

### Linux-Specific Behavior

On Linux (X11/Wayland), clipboard ownership means:
- Data stays available as long as your TUI app is running
- When the app exits, clipboard contents may become unavailable
- This is normal Linux behavior, not an arboard limitation
- For long-running TUI apps like to-tui, this is fine

**No daemon needed:** Unlike some clipboard tools that require external daemons (like xclip or wl-copy), arboard handles clipboard operations directly through system APIs. No additional system packages required at runtime (though X11 development libraries needed at compile time on Linux).

### macOS/Windows

No special considerations. Clipboard operations work synchronously and persist after the app exits.

### MSRV Compatibility

arboard requires Rust 1.71.0+. to-tui uses Rust 2024 edition, so this is not a concern.

## Platform Requirements

### Compile-time Dependencies

| Platform | Requirements |
|----------|--------------|
| macOS | None (uses native Cocoa APIs via objc2) |
| Windows | None (uses native win32 APIs via clipboard-win) |
| Linux | `xorg-dev`, `libxcb-composite0-dev` for X11 support |

### Runtime Dependencies

| Platform | Requirements |
|----------|--------------|
| macOS | None |
| Windows | None |
| Linux | Running X11 or Wayland compositor with data-control protocol |

## Sources

### Primary (HIGH confidence)
- [arboard GitHub repository](https://github.com/1Password/arboard) - Version 3.6.1, August 2025
- [arboard docs.rs documentation](https://docs.rs/arboard/latest/arboard/) - API reference
- [lib.rs arboard page](https://lib.rs/crates/arboard) - Version and feature verification

### Secondary (MEDIUM confidence)
- [copypasta GitHub repository](https://github.com/alacritty/copypasta) - Alternative comparison
- [clipboard-rs docs.rs](https://docs.rs/crate/clipboard-rs/latest) - Alternative comparison
- [ratatui-code-editor crates.io](https://crates.io/crates/ratatui-code-editor) - Integration pattern verification

### Verification Notes
- arboard version 3.6.1 confirmed via GitHub releases (August 23, 2025)
- copypasta version 0.10.2 confirmed via lib.rs (April 25, 2025)
- clipboard-rs version 0.3.1 confirmed via lib.rs (November 11, 2025)
- ratatui + crossterm + arboard integration verified via ratatui-code-editor dependency list

## Metadata

**Confidence breakdown:**
- Crate selection: HIGH - arboard is clearly the ecosystem standard, 1Password-maintained
- Version: HIGH - verified via multiple sources (GitHub, lib.rs, docs.rs)
- API: HIGH - verified via docs.rs documentation
- Integration: HIGH - verified via ratatui-code-editor real-world usage

**Research date:** 2026-01-17
**Valid until:** 2026-04-17 (clipboard crates are stable, 90-day validity reasonable)
