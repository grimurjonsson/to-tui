# Pitfalls Research

**Domain:** Cross-platform terminal clipboard
**Researched:** 2026-01-17
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Ctrl-C vs SIGINT Conflict

**What goes wrong:**
In raw mode (which crossterm enables for TUI apps), Ctrl-C does NOT generate SIGINT. However, users expect Ctrl-C to copy text. On Linux/Windows, Ctrl-C is the traditional copy shortcut. But in terminals, Ctrl-C has meant "interrupt/cancel" for decades. This creates a UX conflict.

**Why it happens:**
- crossterm's `enable_raw_mode()` intercepts Ctrl-C before it reaches the OS signal handler
- The `ctrlc` crate handler will never be called when raw mode is active
- Users from GUI apps expect Ctrl-C = copy, but terminal users expect Ctrl-C = cancel/quit
- On Windows specifically, pressing Ctrl-C in a Windows terminal may immediately terminate the process unless raw mode properly disables `ENABLE_PROCESSED_INPUT`

**How to avoid:**
1. **Use crossterm's event system** to detect Ctrl-C as a key event:
   ```rust
   if key_event.code == KeyCode::Char('c')
      && key_event.modifiers.contains(KeyModifiers::CONTROL) {
       // Handle as copy OR as cancel depending on mode/context
   }
   ```
2. **Consider context-sensitive behavior:**
   - If text is selected: Ctrl-C = copy
   - If no text selected: Ctrl-C = cancel/quit (traditional terminal behavior)
3. **Document the keybinding** prominently for users
4. **Consider alternative keybindings** like `y` (yank, vim-style) to avoid the conflict entirely

**Warning signs:**
- App terminates unexpectedly when pressing Ctrl-C
- `ctrlc::set_handler` callback never fires
- Users report "copy doesn't work" on one platform but works on others

---

### Pitfall 2: Linux Clipboard Data Vanishes on App Exit

**What goes wrong:**
On Linux (X11 and Wayland), clipboard contents disappear when the application that copied them exits. Users copy text, close the TUI, then try to paste - nothing there.

**Why it happens:**
- X11 and Wayland use a "selection ownership" model
- The app that copied data is responsible for *serving* that data when paste requests come in
- Unlike macOS/Windows, data isn't immediately sent to a central clipboard store
- When the `Clipboard` object is dropped or the process exits, the data becomes unavailable
- Even dropping the Clipboard object early within a running app can cause data loss

**How to avoid:**
1. **Keep the Clipboard object alive** in long-lived app state (not a local variable)
2. **Use `arboard`'s `wait()` method** to block until another app requests the data:
   ```rust
   clipboard.set_text(text)?;
   clipboard.wait()?; // Blocks until data is served to another app
   ```
3. **Add strategic delays** near clipboard operations (workaround, not ideal):
   ```rust
   clipboard.set_text(text)?;
   std::thread::sleep(Duration::from_millis(100));
   ```
4. **Document the limitation** for users: "Ensure a clipboard manager is running on Linux"
5. **Recommend clipboard managers** like `copyq`, `clipman`, or `wl-clipboard` to users

**Warning signs:**
- "Copy works on Mac but not on Linux"
- "Sometimes the paste is empty"
- Debug logs show clipboard set succeeded but paste fails
- arboard debug builds warn about "clipboard lifetime mishandling"

---

### Pitfall 3: Command Key (Cmd) Not Detected on macOS

**What goes wrong:**
Using `<Cmd-c>` as the copy keybinding seems natural for macOS users, but the Command key (Super modifier) cannot be detected in most terminal emulators.

**Why it happens:**
- Traditional terminal protocols have no way to encode the Command/Super key
- Only terminals supporting the "kitty keyboard protocol" can distinguish Cmd
- Native macOS Terminal.app does NOT support the enhanced keyboard protocol
- Even with `PushKeyboardEnhancementFlags`, only certain terminals work (kitty, WezTerm, iTerm2)
- The OS often intercepts Cmd+key combinations before they reach the terminal

**How to avoid:**
1. **Do NOT rely on Cmd key detection** as the primary/only copy method
2. **Use Ctrl-C for all platforms** (with appropriate SIGINT handling)
3. **Offer configurable keybindings** so users can set their preferred shortcut
4. **Consider vim-style `y` (yank)** which works universally
5. **If you must support Cmd-C**, check for keyboard enhancement support first:
   ```rust
   // Check if terminal supports enhanced keyboard protocol
   if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
       // Can use Cmd/Super modifier
   }
   ```

**Warning signs:**
- "Copy works in iTerm2 but not Terminal.app"
- Users report pressing Cmd-C does nothing
- KeyEvent shows no Super modifier when Command key is pressed

---

### Pitfall 4: Wayland vs X11 Clipboard Incompatibility

**What goes wrong:**
Clipboard operations fail silently on Wayland, or work on X11 but not Wayland (or vice versa). In mixed environments (XWayland), clipboard synchronization between Wayland and X11 apps breaks.

**Why it happens:**
- Wayland is becoming the default on many Linux distros (as of 2025)
- Many clipboard crates default to X11 and don't enable Wayland support
- Wayland requires the `wlr-data-control` or `ext-data-control-v1` protocol extensions
- Not all Wayland compositors support these extensions
- Sandboxed apps (Flatpak, Snap) need special permissions for clipboard access
- X11 tools (`xclip`, `xsel`) don't work in pure Wayland sessions

**How to avoid:**
1. **Enable Wayland feature flag** in arboard:
   ```toml
   arboard = { version = "3.4", features = ["wayland-data-control"] }
   ```
2. **Test on both X11 and Wayland** - don't assume X11
3. **Handle clipboard errors gracefully** - show user message if clipboard unavailable
4. **Document Wayland requirements** for users:
   - Compositor must support data-control protocols
   - XWayland may be needed as fallback
5. **For sandboxed distribution**, expose both X11 socket AND Wayland interface

**Warning signs:**
- "Works in GNOME but not Sway"
- Clipboard operations return `ClipboardOccupied` or similar errors
- Works on user's X11 system but fails on CI (which might use Wayland)

---

### Pitfall 5: Windows Terminal Ctrl-C Special Handling

**What goes wrong:**
On Windows, Ctrl-C behavior varies between cmd.exe, PowerShell, and Windows Terminal. Raw mode may not properly disable the console's built-in Ctrl-C handling.

**Why it happens:**
- Windows console has `ENABLE_PROCESSED_INPUT` flag that gives Ctrl-C special meaning
- Different terminal emulators handle this differently
- ConPTY (used by Windows Terminal) has different behavior than legacy console
- The `SetConsoleCtrlHandler` API is needed to truly disable Ctrl-C signal

**How to avoid:**
1. **Let crossterm handle raw mode** - it deals with Windows console flags
2. **Test on multiple Windows terminals:** cmd.exe, PowerShell, Windows Terminal
3. **Handle the key event** in your app rather than relying on signals
4. **Don't mix** `ctrlc` crate with crossterm raw mode - pick one approach

**Warning signs:**
- App terminates immediately on Ctrl-C in cmd.exe but not Windows Terminal
- Different behavior when running in ConPTY vs legacy console
- `SetConsoleMode` errors in logs

---

### Pitfall 6: Clipboard Thread Safety Issues

**What goes wrong:**
Clipboard operations fail or deadlock when called from multiple threads. Errors like `ClipboardOccupied` appear sporadically.

**Why it happens:**
- Windows clipboard is a global object that can only be opened on one thread at a time
- arboard uses internal synchronization but parallel operations may still fail
- Creating multiple `Clipboard` instances and operating them concurrently is problematic

**How to avoid:**
1. **Create ONE Clipboard instance** and store it in app state
2. **Do NOT create Clipboard in hot paths** or event handlers
3. **Serialize clipboard operations** - don't call from multiple threads
4. **If threading is needed**, use a channel to send clipboard requests to a single worker

**Warning signs:**
- `ClipboardOccupied` errors appearing randomly
- Clipboard operations work in single-threaded tests but fail in app
- Deadlocks when pressing copy key rapidly

---

### Pitfall 7: Bracketed Paste Mode Not Enabled

**What goes wrong:**
When implementing paste (Ctrl-V), pasted text arrives as a stream of individual key events rather than a single paste event. Multi-line pastes break the app, or special characters in pasted text trigger unintended actions.

**Why it happens:**
- By default, terminals send pasted text as if the user typed it character-by-character
- Bracketed paste mode wraps pasted content in escape sequences so the app can detect it
- If not enabled, the app can't distinguish typed input from pasted input

**How to avoid:**
1. **Enable bracketed paste** at app startup:
   ```rust
   use crossterm::event::EnableBracketedPaste;
   crossterm::execute!(stdout, EnableBracketedPaste)?;
   ```
2. **Disable it during cleanup:**
   ```rust
   use crossterm::event::DisableBracketedPaste;
   crossterm::execute!(stdout, DisableBracketedPaste)?;
   ```
3. **Handle `Event::Paste(text)`** in your event loop for paste content

**Warning signs:**
- Pasting multi-line text creates multiple todo items
- Pasting text with special chars (like `[`, escape) triggers random actions
- No way to distinguish rapid typing from paste

---

### Pitfall 8: Copying Formatted Text Instead of Plain Text

**What goes wrong:**
User copies a todo item expecting plain text, but the clipboard contains markdown formatting, checkbox characters, or ANSI escape codes.

**Why it happens:**
- The app stores todos with markdown syntax (`- [ ] task`)
- If you copy `item.content` directly, you might include formatting
- ANSI escape sequences from rendering might leak into copied text
- Checkbox state characters (`[x]`, `[ ]`) are part of internal representation

**How to avoid:**
1. **Extract plain content only** - strip markdown/checkbox when copying:
   ```rust
   let plain_text = item.content.clone(); // Just the text, no "- [ ]"
   ```
2. **Never copy rendered output** - copy from data model, not display buffer
3. **Test by pasting into plain text editor** - should be clean text
4. **Provide format options** if users want markdown (explicit choice, not default)

**Warning signs:**
- Pasted text has `- [ ]` prefixes
- Strange characters appear in pasted text
- Users complain paste looks different than what was shown

---

## Cross-Platform Gotchas

| Platform | Issue | Solution |
|----------|-------|----------|
| macOS | Cmd key not detected in most terminals | Use Ctrl-C or vim-style `y` keybinding; don't rely on Cmd |
| macOS | Terminal.app doesn't support kitty protocol | Document iTerm2/WezTerm as recommended terminals |
| Linux | Clipboard content lost on app exit | Keep Clipboard alive, use `wait()`, recommend clipboard managers |
| Linux | Wayland requires explicit feature flag | Enable `wayland-data-control` feature in arboard |
| Linux | Wayland compositor may lack data-control | Document supported compositors, recommend XWayland fallback |
| Linux | Sandbox (Flatpak/Snap) blocks clipboard | Document permission requirements |
| Windows | Ctrl-C may terminate process in legacy console | Let crossterm handle raw mode; test on cmd.exe |
| Windows | Different behavior in ConPTY vs legacy | Test on Windows Terminal AND cmd.exe |
| All | Multiple threads accessing clipboard | Single Clipboard instance, serialize operations |
| All | Ctrl-C conflicts with SIGINT | Handle Ctrl-C in event loop, not via signal handler |

---

## "Looks Done But Isn't" Checklist

- [ ] **Ctrl-C handling:** Often missing SIGINT conflict resolution -- verify Ctrl-C is handled in event loop AND app doesn't terminate unexpectedly
- [ ] **Linux clipboard lifetime:** Often missing `wait()` or persistent Clipboard -- verify copy persists after app exits (with clipboard manager)
- [ ] **Wayland feature flag:** Often not enabled by default -- verify `wayland-data-control` feature is in Cargo.toml
- [ ] **macOS Cmd key:** Often assumed to work -- verify copy works in Terminal.app (not just iTerm2)
- [ ] **Plain text copy:** Often copies internal representation -- verify pasted text has no markdown/escape codes
- [ ] **Bracketed paste:** Often not enabled -- verify multi-line paste works correctly
- [ ] **Thread safety:** Often creates Clipboard per-operation -- verify single instance, no parallel operations
- [ ] **Error handling:** Often ignores clipboard errors -- verify user sees message if clipboard unavailable
- [ ] **Windows terminals:** Often only tested in one terminal -- verify behavior in cmd.exe, PowerShell, and Windows Terminal

---

## Library Recommendations

Based on research, **arboard** (by 1Password) is the recommended clipboard library:

**Pros:**
- Actively maintained by 1Password
- Supports all three platforms
- Has Wayland support (via feature flag)
- Debug builds warn about lifetime issues
- Has `wait()` method for Linux data persistence

**Configuration:**
```toml
[dependencies]
arboard = { version = "3.4", features = ["wayland-data-control"] }
```

**Alternatives considered:**
- `terminal-clipboard`: Simpler API but less actively maintained, no Wayland
- `cli-clipboard`: Fork of rust-clipboard, has Wayland but less active
- `copypasta` (Alacritty): Good but primarily designed for Alacritty's needs

---

## Sources

### Primary (HIGH confidence)
- [arboard GitHub README](https://github.com/1Password/arboard) - Authoritative documentation from 1Password
- [arboard docs.rs](https://docs.rs/arboard/latest/arboard/struct.Clipboard.html) - API documentation
- [crossterm event module](https://docs.rs/crossterm/latest/crossterm/event/index.html) - Keyboard enhancement flags
- [crossterm GitHub Issue #214](https://github.com/crossterm-rs/crossterm/issues/214) - Windows Ctrl-C handling

### Secondary (MEDIUM confidence)
- [Handling Ctrl-C with crossterm](https://dev.to/plecos/handling-ctrl-c-while-using-crossterm-1kil) - Practical workarounds
- [crossterm GitHub Issue #861](https://github.com/crossterm-rs/crossterm/issues/861) - macOS modifier key issues
- [crossterm KeyModifiers](https://docs.rs/crossterm/latest/crossterm/event/struct.KeyModifiers.html) - Modifier key documentation
- [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) - Enhanced keyboard protocol spec
- [terminal-clipboard GitHub](https://github.com/Canop/terminal-clipboard) - Alternative library documentation
- [ArchWiki Clipboard](https://wiki.archlinux.org/title/Clipboard) - Linux clipboard fundamentals

### Tertiary (LOW confidence)
- [EdTUI crate](https://docs.rs/edtui/latest/edtui/) - Example of ratatui + clipboard integration
- Various WebSearch results about ConPTY changes in Windows Terminal 2024

---

## Metadata

**Confidence breakdown:**
- SIGINT/Ctrl-C pitfall: HIGH - Verified with crossterm docs and GitHub issues
- Linux clipboard lifetime: HIGH - Documented extensively in arboard README
- macOS Cmd key: HIGH - Verified with crossterm issues and kitty protocol docs
- Wayland issues: HIGH - Documented in arboard README
- Windows issues: MEDIUM - Based on crossterm issues, less direct testing data
- Thread safety: HIGH - Documented in arboard API docs
- Bracketed paste: MEDIUM - Based on edtui and crossterm docs

**Research date:** 2026-01-17
**Valid until:** 2026-02-17 (30 days - clipboard ecosystem is relatively stable)
