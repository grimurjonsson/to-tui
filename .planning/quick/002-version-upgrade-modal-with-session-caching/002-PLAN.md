---
plan: 002
type: quick
scope: version-upgrade-modal
files_modified:
  - src/app/mode.rs
  - src/app/state.rs
  - src/app/event.rs
  - src/ui/components/mod.rs
  - src/config.rs
  - src/utils/paths.rs
autonomous: true

must_haves:
  truths:
    - "When new version is detected, modal appears prompting user to upgrade"
    - "Clicking 'No' dismisses modal for session (re-prompts on app restart)"
    - "Checking 'Don't remind me again for this version' skips future reminders for that version"
    - "Clicking version text in status bar opens upgrade modal"
  artifacts:
    - path: "src/app/mode.rs"
      provides: "Mode::UpgradePrompt variant"
    - path: "src/app/state.rs"
      provides: "UpgradePromptState, session_dismissed_upgrade flag, skipped_version config"
    - path: "src/app/event.rs"
      provides: "handle_upgrade_prompt_mode key handler"
    - path: "src/ui/components/mod.rs"
      provides: "render_upgrade_overlay function"
    - path: "src/config.rs"
      provides: "skipped_version field persisted to config"
  key_links:
    - from: "src/app/state.rs"
      to: "src/config.rs"
      via: "load/save skipped_version"
      pattern: "skipped_version"
---

<objective>
Add a version upgrade modal that prompts users when a new version is detected. The modal offers:
- "Yes" to acknowledge (shows install instructions or opens release page)
- "No" to dismiss for this session (prompts again on restart)
- Checkbox: "Don't remind me again for this version" to skip this version permanently

Clicking the "v0.3.0 -> v0.3.1" text in the status bar also opens this modal.

Purpose: Improve UX for version notifications - give users control over upgrade reminders.
Output: Working modal with session caching and persistent version skip.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
# Existing patterns to follow:
@src/app/mode.rs           # Mode enum - add UpgradePrompt variant
@src/app/state.rs          # AppState - version check already exists (new_version_available)
@src/app/event.rs          # Event handlers - follow rollover_mode pattern
@src/ui/components/mod.rs  # Overlays - follow render_rollover_overlay pattern
@src/config.rs             # Config struct - add skipped_version field
@src/utils/paths.rs        # Path helpers
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add UpgradePrompt mode and state management</name>
  <files>
    src/app/mode.rs
    src/app/state.rs
    src/config.rs
  </files>
  <action>
1. In `src/app/mode.rs`:
   - Add `UpgradePrompt` variant to Mode enum
   - Add Display impl: `Mode::UpgradePrompt => write!(f, "UPGRADE")`

2. In `src/config.rs`:
   - Add `skipped_version: Option<String>` field to Config struct with `#[serde(default)]`
   - Add `save()` method to Config that writes to config file (use get_config_path())
   - Note: Config currently only has `load()`, need to add `save()` for persisting skipped_version

3. In `src/app/state.rs`:
   - Add `session_dismissed_upgrade: bool` field to AppState (default false)
   - Add `skipped_version: Option<String>` field to AppState (loaded from Config)
   - Add `show_upgrade_prompt: bool` field to AppState (default false)
   - Modify `check_version_update()` to:
     - Check if `new_version_available` matches `skipped_version` (if so, ignore)
     - Check if `session_dismissed_upgrade` is true (if so, don't auto-show)
     - Otherwise set `show_upgrade_prompt = true` and `mode = Mode::UpgradePrompt`
   - Add method `open_upgrade_modal()` - sets mode to UpgradePrompt, show_upgrade_prompt = true
   - Add method `dismiss_upgrade_session()` - sets session_dismissed_upgrade = true, mode = Navigate
   - Add method `skip_version_permanently(version: String)` - saves to config, sets skipped_version
  </action>
  <verify>
    cargo check 2>&1 | head -50
  </verify>
  <done>
    Mode::UpgradePrompt exists, AppState has upgrade-related fields, Config can save skipped_version
  </done>
</task>

<task type="auto">
  <name>Task 2: Add upgrade modal rendering and event handling</name>
  <files>
    src/ui/components/mod.rs
    src/app/event.rs
  </files>
  <action>
1. In `src/ui/components/mod.rs`:
   - Add rendering for upgrade modal in the `render()` function (after rollover check):
     ```rust
     if state.mode == Mode::UpgradePrompt {
         render_upgrade_overlay(f, state);
     }
     ```
   - Create `render_upgrade_overlay(f: &mut Frame, state: &AppState)` function:
     - Use centered_rect(50, 35, f.area()) for modal size
     - Title: " New Version Available "
     - Show current version and new version: "v{current} -> v{new}"
     - Show release URL hint: "https://github.com/grimurjonsson/to-tui/releases"
     - Footer with options similar to rollover modal:
       - `[Y]es - View release page (prints URL to terminal after quit)`
       - `[N]o - Remind me later`
       - `[S]kip - Don't remind me for this version`
     - Style: Use state.theme colors, similar to other overlays

2. In `src/app/event.rs`:
   - Add `Mode::UpgradePrompt => handle_upgrade_prompt_mode(key, state)?` in handle_key_event match
   - Create `handle_upgrade_prompt_mode(key: KeyEvent, state: &mut AppState) -> Result<()>`:
     - 'y' | 'Y' | Enter: Set a flag to show release URL after quit, close modal
     - 'n' | 'N' | Esc: Call `state.dismiss_upgrade_session()`, close modal
     - 's' | 'S': Call `state.skip_version_permanently()` with the new version, close modal

3. Make status bar version text clickable:
   - In handle_mouse_event, detect click on version area (right side of status bar)
   - If new_version_available.is_some() and clicked on version text, call state.open_upgrade_modal()
   - Calculate version text position from terminal_width (version text is right-aligned)
  </action>
  <verify>
    cargo check && cargo test 2>&1 | tail -20
  </verify>
  <done>
    Upgrade modal renders when Mode::UpgradePrompt, handles Y/N/S keys, status bar version is clickable
  </done>
</task>

<task type="auto">
  <name>Task 3: Integration and testing</name>
  <files>
    src/main.rs (if needed for release URL handling)
    src/app/state.rs (refinements)
  </files>
  <action>
1. Integration refinements:
   - Ensure the upgrade modal only auto-opens ONCE per detected version (not on every tick)
   - When 'Y' is pressed, store the release URL in AppState for printing after quit
   - In main.rs, after the main loop ends, if there's a pending release URL, print it

2. Add simple test in state.rs:
   ```rust
   #[test]
   fn test_session_dismiss_upgrade() {
       // Verify session_dismissed_upgrade prevents auto-showing modal
   }
   ```

3. Manual verification checklist:
   - Run `cargo run`, verify no modal if no new version
   - Temporarily modify CURRENT_VERSION in version_check.rs to test modal appears
   - Test Y/N/S keys work correctly
   - Test clicking version in status bar opens modal
   - Test that after 'N', restarting app shows modal again
   - Test that after 'S', restarting app does NOT show modal for same version
  </action>
  <verify>
    cargo build --release && cargo test
  </verify>
  <done>
    Full integration working: modal appears on new version, respects session/permanent dismiss, clickable status bar
  </done>
</task>

</tasks>

<verification>
- `cargo build --release` succeeds with no warnings
- `cargo test` all tests pass
- Manual test: Modal appears when new version detected (can test by modifying CURRENT_VERSION)
- Manual test: 'N' dismisses for session, reopen app shows modal again
- Manual test: 'S' skips version permanently, reopen app does NOT show modal
- Manual test: Click on "v0.3.0 -> v0.3.1" in status bar opens modal
</verification>

<success_criteria>
1. New version detected -> modal appears with Y/N/S options
2. 'N' dismisses for session only (restart prompts again)
3. 'S' skips this version permanently (config persisted)
4. Status bar version text clickable to open modal
5. No clippy warnings, all tests pass
</success_criteria>

<output>
After completion, create `.planning/quick/002-version-upgrade-modal-with-session-caching/002-SUMMARY.md`
</output>
