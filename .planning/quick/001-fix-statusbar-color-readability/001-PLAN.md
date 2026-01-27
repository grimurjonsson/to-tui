---
phase: quick
plan: 001
type: execute
wave: 1
depends_on: []
files_modified:
  - src/ui/theme.rs
autonomous: true

must_haves:
  truths:
    - "Status bar text is clearly readable in default and dark themes"
    - "Light theme status bar contrast remains good"
  artifacts:
    - path: "src/ui/theme.rs"
      provides: "Theme status bar colors"
      contains: "status_bar_bg"
  key_links: []
---

<objective>
Fix status bar color readability by improving foreground/background contrast

Purpose: The current DarkGray background with White foreground has insufficient contrast ratio, making the status bar hard to read. This fix improves usability across all terminal emulators.

Output: Updated theme.rs with better status bar color contrast
</objective>

<execution_context>
@/Users/gimmi/.claude/get-shit-done/workflows/execute-plan.md
@/Users/gimmi/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/ui/theme.rs
@src/ui/components/status_bar.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Improve status bar color contrast</name>
  <files>src/ui/theme.rs</files>
  <action>
Update the status bar colors in theme.rs for better contrast:

1. In `default_theme()`:
   - Change `status_bar_bg` from `Color::DarkGray` to `Color::Rgb(40, 40, 40)` (darker gray)
   - Keep `status_bar_fg` as `Color::White` (good contrast against darker bg)

2. In `dark()`:
   - Change `status_bar_bg` from `Color::DarkGray` to `Color::Rgb(40, 40, 40)` (darker gray)
   - Keep `status_bar_fg` as `Color::White`

3. In `light()`:
   - Keep current values (LightBlue bg, Black fg) as they already have good contrast

The RGB(40, 40, 40) provides much better contrast with White text compared to DarkGray (which renders as ~128,128,128 in most terminals).
  </action>
  <verify>
Run `cargo build --release` to verify code compiles without errors.
Run `cargo run` briefly to visually confirm status bar is more readable.
  </verify>
  <done>
Status bar text has high contrast ratio and is clearly readable in default and dark themes.
  </done>
</task>

</tasks>

<verification>
- cargo build --release succeeds
- Visual inspection confirms improved status bar readability
</verification>

<success_criteria>
- Theme status bar colors updated with better contrast values
- All three theme variants compile and render correctly
- Status bar text is clearly legible across terminal emulators
</success_criteria>

<output>
After completion, create `.planning/quick/001-fix-statusbar-color-readability/001-SUMMARY.md`
</output>
