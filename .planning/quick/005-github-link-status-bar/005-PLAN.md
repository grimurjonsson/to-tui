---
phase: quick-005
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/ui/components/status_bar.rs
  - src/app/event.rs
autonomous: true

must_haves:
  truths:
    - "GitHub octopus emoji link visible in status bar"
    - "Clicking the link opens browser to to-tui repository"
  artifacts:
    - path: "src/ui/components/status_bar.rs"
      provides: "GitHub link rendering in status bar"
    - path: "src/app/event.rs"
      provides: "Click handler for GitHub link"
  key_links:
    - from: "src/app/event.rs"
      to: "open::that()"
      via: "mouse click on GitHub link area"
---

<objective>
Add a GitHub link with octopus emoji to the status bar that opens a browser to the to-tui GitHub repository when clicked.

Purpose: Provide quick access to the project's GitHub page directly from the TUI.
Output: Clickable GitHub link in status bar that opens https://github.com/gtunes/to-tui
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/ui/components/status_bar.rs - Current status bar implementation with version display
@src/app/event.rs - Mouse event handling, including existing version text click detection (lines 106-127)
@Cargo.toml - Dependencies (need to add `open` crate)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add open crate dependency and render GitHub link</name>
  <files>Cargo.toml, src/ui/components/status_bar.rs</files>
  <action>
1. Add `open = "5"` to Cargo.toml dependencies (lightweight cross-platform URL opener)

2. In status_bar.rs, modify the `render` function to add a GitHub link:
   - Define constant: `const GITHUB_URL: &str = "https://github.com/gtunes/to-tui";`
   - Add GitHub link text with octopus emoji before the version text
   - Use format like " \u{1F419} " (octopus emoji, Unicode codepoint) - note: user mentioned "similar" so octopus works as a GitHub-ish icon
   - Position: Between nav_hint and version_text, right-aligned with version

3. Update padding calculation to account for the GitHub link width

4. The status bar layout should be:
   `[left_content] [nav_hint] ... [github_link] [version_text]`
   Where github_link is something like " <octopus> " (clickable area)
  </action>
  <verify>Run `cargo build` to confirm compilation succeeds</verify>
  <done>Status bar displays octopus emoji link before version text</done>
</task>

<task type="auto">
  <name>Task 2: Add click handler for GitHub link</name>
  <files>src/app/event.rs</files>
  <action>
1. In handle_mouse_event, within the status bar click detection block (around line 111), add logic to detect clicks on the GitHub link area:

   - Calculate github_link position (it's to the left of version_text)
   - The github_link text is something like " \u{1F419} " (about 3-4 chars wide)
   - When clicked, call `open::that(GITHUB_URL)` to open the browser

2. Add the constant at the top of event.rs:
   `const GITHUB_URL: &str = "https://github.com/gtunes/to-tui";`

3. The click detection should work similar to the existing version text click detection pattern already in the code (lines 112-126)

4. Handle the Result from open::that() - if it fails, optionally show a status message, but don't crash
  </action>
  <verify>Run `cargo test` to ensure no regressions; `cargo clippy` for lint check</verify>
  <done>Clicking the octopus emoji in status bar opens browser to GitHub repo</done>
</task>

</tasks>

<verification>
1. `cargo build` - compiles without errors
2. `cargo clippy` - no warnings
3. `cargo test` - all tests pass
4. Manual test: Run `cargo run`, observe octopus in status bar, click it to open browser
</verification>

<success_criteria>
- Octopus emoji visible in status bar (right side, before version)
- Clicking emoji opens https://github.com/gtunes/to-tui in default browser
- No clippy warnings, all tests pass
</success_criteria>

<output>
After completion, create `.planning/quick/005-github-link-status-bar/005-SUMMARY.md`
</output>
