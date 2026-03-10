---
phase: quick
plan: 7
type: execute
wave: 1
depends_on: []
files_modified:
  - src/keybindings/mod.rs
  - src/app/event.rs
  - src/ui/components/mod.rs
autonomous: true
requirements: ["QUICK-7"]

must_haves:
  truths:
    - "User can press a key in Navigate mode to copy the log file path to clipboard"
    - "Status bar shows confirmation message after copying"
    - "Copied path points to the current day's log file"
  artifacts:
    - path: "src/keybindings/mod.rs"
      provides: "CopyLogPath action variant"
      contains: "CopyLogPath"
    - path: "src/app/event.rs"
      provides: "CopyLogPath action handler"
      contains: "Action::CopyLogPath"
  key_links:
    - from: "src/app/event.rs"
      to: "src/utils/paths.rs"
      via: "get_logs_dir() call"
      pattern: "get_logs_dir"
    - from: "src/app/event.rs"
      to: "src/clipboard.rs"
      via: "copy_to_clipboard() call"
      pattern: "copy_to_clipboard"
---

<objective>
Add a keybinding that copies the log file path to the system clipboard so users can easily open/tail logs.

Purpose: Users running into issues need quick access to the log file location without memorizing paths.
Output: New `L` keybinding in Navigate mode that copies `~/.to-tui/logs/` path to clipboard with status message.
</objective>

<execution_context>
@/Users/gimmi/.claude/get-shit-done/workflows/execute-plan.md
@/Users/gimmi/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@CLAUDE.md

<interfaces>
<!-- Key types and contracts the executor needs -->

From src/keybindings/mod.rs:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // ... existing variants ...
    Yank,           // Clipboard example - "y" keybinding
    CyclePriority,
    SortByPriority,
    // Edit mode variants ...
}
```

From src/app/event.rs (Action::Yank handler as pattern):
```rust
Action::Yank => {
    if let Some(item) = state.selected_item() {
        let text = item.content.clone();
        match copy_to_clipboard(&text) {
            Ok(CopyResult::SystemClipboard) => {
                state.set_status_message(format!("Copied: {}", display_text));
            }
            Ok(CopyResult::InternalBuffer { file_path }) => { ... }
            Err(e) => {
                state.set_status_message(format!("Copy failed: {}", e));
            }
        }
    }
}
```

From src/utils/paths.rs:
```rust
pub fn get_logs_dir() -> Result<PathBuf>  // Returns ~/.to-tui/logs/
```

From src/clipboard.rs:
```rust
pub fn copy_to_clipboard(text: &str) -> Result<CopyResult>
pub enum CopyResult {
    SystemClipboard,
    InternalBuffer { file_path: Option<PathBuf> },
}
```

Logging uses `tracing_appender::rolling::daily(&logs_dir, "totui.log")` which produces files like `totui.log.2026-03-10`.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add CopyLogPath action and keybinding</name>
  <files>src/keybindings/mod.rs</files>
  <action>
Add a `CopyLogPath` variant to the `Action` enum (place it after `Yank` in the Clipboard section).

Add the string mapping in both `Display` impl (`Action::CopyLogPath => "copy_log_path"`) and `FromStr` impl (`"copy_log_path" => Ok(Action::CopyLogPath)`).

Add default keybinding in `default_navigate_bindings()`: `m.insert("L".to_string(), "copy_log_path".to_string());` — capital L is currently unbound and mnemonic for "Logs".
  </action>
  <verify>
    <automated>cd /Users/gimmi/Documents/Sources/rust/to-tui && cargo test --lib keybindings -- --quiet 2>&1 | tail -5</automated>
  </verify>
  <done>CopyLogPath action exists in enum, has Display/FromStr mappings, default bound to "L" in navigate mode</done>
</task>

<task type="auto">
  <name>Task 2: Implement CopyLogPath handler and add to help overlay</name>
  <files>src/app/event.rs, src/ui/components/mod.rs</files>
  <action>
In `src/app/event.rs`, in the match block where other actions are handled (near `Action::Yank`), add the `Action::CopyLogPath` arm:

1. Call `get_logs_dir()` to get the logs directory path.
2. Copy the directory path string to clipboard using `copy_to_clipboard(&logs_dir.display().to_string())`.
3. On success, show status message: `"Log path copied: {path}"` (or `"Log path copied to buffer: {path}"` for headless fallback).
4. On error, show `"Could not copy log path: {error}"`.
5. Add `use crate::utils::paths::get_logs_dir;` at top of file if not already imported.

Follow the exact same pattern as `Action::Yank` for clipboard handling (SystemClipboard vs InternalBuffer branches).

In `src/ui/components/mod.rs`, in `render_help_overlay`, add a line in the "Other" section (before the `?` help toggle line):
```rust
lines.push(Line::from(vec![
    Span::styled("    L               ", key_style),
    Span::styled("Copy log file path to clipboard", desc_style),
]));
```

Update `HELP_TOTAL_LINES` constant (grep for it — it controls scrolling) by incrementing it by 1 to account for the new line.
  </action>
  <verify>
    <automated>cd /Users/gimmi/Documents/Sources/rust/to-tui && cargo clippy 2>&1 | tail -10</automated>
  </verify>
  <done>Pressing L in Navigate mode copies log directory path to clipboard, shows confirmation in status bar, and the help overlay documents the keybinding</done>
</task>

</tasks>

<verification>
```bash
# Build succeeds with no warnings
cargo clippy 2>&1 | grep -E "warning|error" | grep -v "Compiling\|Finished\|Checking"

# All tests pass
cargo test 2>&1 | tail -5

# Action roundtrip works (existing test pattern covers new variant via exhaustive match)
cargo test --lib keybindings -- --quiet
```
</verification>

<success_criteria>
- `L` key in Navigate mode copies log directory path to system clipboard
- Status bar shows confirmation with the copied path
- Help overlay includes the new keybinding under "Other" section
- `cargo clippy` passes with no warnings
- `cargo test` passes
</success_criteria>

<output>
After completion, create `.planning/quick/7-add-logs-link-to-copy-logfile-path-to-cl/7-SUMMARY.md`
</output>
