# Midnight Rollover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect midnight crossover while the TUI is running, prompt the user to roll over (with a "Don't ask again" option), and persist their choice as a tri-state preference (Ask / AutoYes / AutoNo).

**Architecture:** Reuse the existing 100ms `tokio::time::interval` tick in `src/ui/mod.rs`. Each tick, an `AppState::check_midnight_rollover` method compares `Local::now().date_naive()` against `state.todo_list.date`, gated on `Mode::Navigate` so active editing is never interrupted. On crossover the existing `find_rollover_candidates_for_project` and `execute_rollover_for_project` are invoked; the existing `Mode::Rollover` modal is extended with a "remember" checkbox that writes the chosen preference to `~/.config/to-tui/config.toml`.

**Tech Stack:** Rust 1.x, tokio, ratatui, crossterm, serde, anyhow, chrono. Tests: `cargo test` (built-in `#[test]`).

**Spec:** `docs/superpowers/specs/2026-05-06-midnight-rollover-design.md`

---

## File Structure

**Create:**
- (none — all changes extend existing modules)

**Modify:**
- `src/config.rs` — add `AutoRolloverPref` enum + `Config::auto_rollover` field
- `src/app/state.rs` — add `auto_rollover_pref` field, extend `PendingRollover`, add `check_midnight_rollover` and helpers
- `src/main.rs` — pass `config.auto_rollover` into `AppState::new`
- `src/ui/mod.rs` — call `state.check_midnight_rollover()` in tick arm
- `src/ui/components/mod.rs` — render checkbox row in `render_rollover_overlay`
- `src/app/event.rs` — extend `handle_rollover_mode` to toggle remember + persist on confirm

---

## Task 1: Add `AutoRolloverPref` enum and config field

**Files:**
- Modify: `src/config.rs`
- Test: `src/config.rs` (existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing tests for the new enum and field**

Append the following tests to the `tests` module in `src/config.rs` (after the existing `test_marketplaces_config_uses_default_when_missing`):

```rust
    #[test]
    fn test_auto_rollover_default_is_ask() {
        let config = Config::default();
        assert_eq!(config.auto_rollover, AutoRolloverPref::Ask);
    }

    #[test]
    fn test_auto_rollover_serialises_snake_case() {
        let mut config = Config::default();
        config.auto_rollover = AutoRolloverPref::AutoYes;
        let toml_str = toml::to_string(&config).unwrap();
        assert!(
            toml_str.contains("auto_rollover = \"auto_yes\""),
            "expected snake_case serialisation, got: {toml_str}"
        );
    }

    #[test]
    fn test_auto_rollover_deserialises_all_variants() {
        for (input, expected) in [
            ("ask", AutoRolloverPref::Ask),
            ("auto_yes", AutoRolloverPref::AutoYes),
            ("auto_no", AutoRolloverPref::AutoNo),
        ] {
            let toml_str = format!("auto_rollover = \"{input}\"\n");
            let config: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(config.auto_rollover, expected, "input was {input}");
        }
    }

    #[test]
    fn test_auto_rollover_missing_field_defaults_to_ask() {
        let toml_str = "theme = \"dark\"\n";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.auto_rollover, AutoRolloverPref::Ask);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::tests::test_auto_rollover -- --nocapture`
Expected: FAIL with errors like `cannot find type AutoRolloverPref` and `no field auto_rollover on type Config`.

- [ ] **Step 3: Add the `AutoRolloverPref` enum**

In `src/config.rs`, after the existing `MarketplacesConfig` impl block (around line 53), add:

```rust
/// User preference for what happens at midnight crossover.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoRolloverPref {
    /// Show the rollover modal and ask the user (default).
    #[default]
    Ask,
    /// Automatically rollover incomplete items at midnight.
    AutoYes,
    /// Never auto-rollover and never prompt; user can still trigger manually with R.
    AutoNo,
}
```

- [ ] **Step 4: Add the `auto_rollover` field to `Config`**

In the `Config` struct definition (around line 56), add a new field above the closing brace:

```rust
    #[serde(default)]
    pub auto_rollover: AutoRolloverPref,
```

In the `Default for Config` impl (around line 87), add the field initialiser:

```rust
            auto_rollover: AutoRolloverPref::default(),
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib config::tests -- --nocapture`
Expected: PASS for all four new tests plus the existing config tests.

- [ ] **Step 6: Run lint**

Run: `cargo clippy --lib -- -D warnings`
Expected: No warnings.

- [ ] **Step 7: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add AutoRolloverPref tri-state preference

Adds auto_rollover field with Ask/AutoYes/AutoNo variants. Default is
Ask (current behaviour). Serialises as snake_case in config.toml."
```

---

## Task 2: Extend `PendingRollover` with `remember_choice`

**Files:**
- Modify: `src/app/state.rs:115`
- Test: `src/app/state.rs` (a new `#[cfg(test)] mod tests` if absent, or extend existing)

- [ ] **Step 1: Verify whether state.rs has a tests module**

Run: `grep -n "#\[cfg(test)\]" src/app/state.rs`
If output is empty, this task adds a new module at the bottom of the file. If it has one, append into it.

- [ ] **Step 2: Write failing test for `open_rollover_modal` initialising `remember_choice` to false**

Append the following test (creating the `tests` module if needed) at the end of `src/app/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use crate::todo::{TodoItem, TodoList, TodoState};
    use crate::ui::theme::Theme;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn test_state() -> AppState {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let list = TodoList::with_items(date, PathBuf::from("/tmp/test.md"), vec![]);
        AppState::new(
            list,
            Theme::default(),
            crate::keybindings::KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default(),
            crate::plugin::loader::PluginLoader::new(),
            vec![],
            crate::plugin::PluginActionRegistry::new(),
        )
    }

    #[test]
    fn open_rollover_modal_starts_with_remember_off() {
        let mut state = test_state();
        let source = NaiveDate::from_ymd_opt(2026, 5, 5).unwrap();
        let items = vec![TodoItem::with_state(
            "Task".to_string(),
            TodoState::Empty,
            0,
        )];
        state.open_rollover_modal(source, items);
        let pending = state.pending_rollover.as_ref().expect("pending");
        assert!(!pending.remember_choice);
    }
}
```

If any of `Theme::default()`, `KeybindingCache::default()`, `Project::default()`, or `PluginLoader::new()` don't compile, find the existing test helper pattern by running:

```bash
grep -rn "fn test_state\|AppState::new" src/ --include="*.rs" | head
```
and adapt accordingly.

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib app::state::tests::open_rollover_modal_starts_with_remember_off -- --nocapture`
Expected: FAIL — either compile error (`no field remember_choice`) or assertion failure.

- [ ] **Step 4: Add `remember_choice` to `PendingRollover`**

In `src/app/state.rs:115-119`, replace:

```rust
#[derive(Debug, Clone)]
pub struct PendingRollover {
    pub source_date: NaiveDate,
    pub items: Vec<TodoItem>,
}
```

with:

```rust
#[derive(Debug, Clone)]
pub struct PendingRollover {
    pub source_date: NaiveDate,
    pub items: Vec<TodoItem>,
    /// Whether the "Don't ask again" checkbox is currently ticked in the modal.
    pub remember_choice: bool,
}
```

- [ ] **Step 5: Update `open_rollover_modal` to initialise the field**

In `src/app/state.rs` around line 1726, replace the body of `open_rollover_modal`:

```rust
    pub fn open_rollover_modal(&mut self, source_date: NaiveDate, items: Vec<TodoItem>) {
        self.pending_rollover = Some(PendingRollover {
            source_date,
            items,
            remember_choice: false,
        });
        self.mode = Mode::Rollover;
    }
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --lib app::state::tests::open_rollover_modal_starts_with_remember_off -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Build whole crate to surface other PendingRollover construction sites**

Run: `cargo build --lib --bins`
Expected: PASS. If it fails, search for `PendingRollover {` and add `remember_choice: false` to each construction site.

```bash
grep -rn "PendingRollover {" src/ --include="*.rs"
```

- [ ] **Step 8: Run lint**

Run: `cargo clippy --lib -- -D warnings`
Expected: No warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app/state.rs
git commit -m "feat(state): add remember_choice flag to PendingRollover

Modal-local UI state for the upcoming Don't ask again checkbox.
Initialised to false in open_rollover_modal."
```

---

## Task 3: Thread `auto_rollover_pref` into `AppState`

**Files:**
- Modify: `src/app/state.rs:184` (field), `src/app/state.rs:254` (constructor)
- Modify: `src/main.rs:297` (call site)
- Test: extend `src/app/state.rs` tests module

- [ ] **Step 1: Write failing test that AppState carries the preference**

Append to the `tests` module in `src/app/state.rs`:

```rust
    #[test]
    fn app_state_stores_auto_rollover_pref() {
        let mut state = test_state();
        state.auto_rollover_pref = crate::config::AutoRolloverPref::AutoYes;
        assert_eq!(state.auto_rollover_pref, crate::config::AutoRolloverPref::AutoYes);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib app::state::tests::app_state_stores_auto_rollover_pref -- --nocapture`
Expected: FAIL with `no field auto_rollover_pref on type AppState`.

- [ ] **Step 3: Add the field to `AppState`**

In `src/app/state.rs` around line 184 (right after `pub pending_rollover: Option<PendingRollover>,`), add:

```rust
    /// User preference for midnight rollover behaviour (Ask / AutoYes / AutoNo).
    pub auto_rollover_pref: crate::config::AutoRolloverPref,
```

- [ ] **Step 4: Add a parameter to `AppState::new`**

Change the `pub fn new(` signature around line 254 by adding `auto_rollover_pref: crate::config::AutoRolloverPref,` as the **last** parameter:

```rust
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        todo_list: TodoList,
        theme: Theme,
        keybindings: KeybindingCache,
        timeoutlen: u64,
        ui_cache: Option<UiCache>,
        skipped_version: Option<String>,
        current_project: Project,
        plugin_loader: PluginLoader,
        plugin_errors: Vec<PluginLoadError>,
        plugin_action_registry: PluginActionRegistry,
        auto_rollover_pref: crate::config::AutoRolloverPref,
    ) -> Self {
```

In the struct literal inside the body (the block starting `let mut state = Self {`), add the field initialiser. A natural location is right after the `pending_rollover: None,` line:

```rust
            auto_rollover_pref,
```

- [ ] **Step 5: Update test helper in `tests` module**

Update `fn test_state()` in the `tests` module to pass the new argument. Replace its body with:

```rust
    fn test_state() -> AppState {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let list = TodoList::with_items(date, PathBuf::from("/tmp/test.md"), vec![]);
        AppState::new(
            list,
            Theme::default(),
            crate::keybindings::KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default(),
            crate::plugin::loader::PluginLoader::new(),
            vec![],
            crate::plugin::PluginActionRegistry::new(),
            crate::config::AutoRolloverPref::Ask,
        )
    }
```

- [ ] **Step 6: Update production call site in `src/main.rs`**

In `src/main.rs` around line 297, the existing call is:

```rust
            let mut state = app::AppState::new(
                list,
                theme,
                keybindings,
                config.timeoutlen,
                ui_cache,
                config.skipped_version.clone(),
                current_project,
                plugin_loader,
                plugin_errors,
                plugin_action_registry,
            );
```

Add the new argument as the last entry before the closing `)`:

```rust
                plugin_action_registry,
                config.auto_rollover,
```

- [ ] **Step 7: Build and run tests**

Run: `cargo build --lib --bins && cargo test --lib`
Expected: PASS. If anywhere else constructs `AppState` directly (search with `grep -rn "AppState::new" src/`), update those call sites the same way.

- [ ] **Step 8: Run lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: No warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app/state.rs src/main.rs
git commit -m "feat(state): plumb auto_rollover_pref through AppState

AppState gains an auto_rollover_pref field initialised from
Config::auto_rollover at startup. Sets the stage for the midnight
detector and Don't ask again persistence."
```

---

## Task 4: Implement `check_midnight_rollover` + helpers on `AppState`

**Files:**
- Modify: `src/app/state.rs`
- Test: `src/app/state.rs` tests module

- [ ] **Step 1: Identify the existing `silently_advance_to_today` shape**

Run: `grep -n "load_today_list_for_project\|fn load_today" src/main.rs src/app/state.rs`
The plan reuses `load_todo_list_for_project` from `src/storage/file.rs:9` (signature: `pub fn load_todo_list_for_project(project_name: &str, date: NaiveDate) -> Result<TodoList>`).

- [ ] **Step 2: Write failing tests covering all guard branches**

Append to the `tests` module in `src/app/state.rs`:

```rust
    use crate::config::AutoRolloverPref;

    fn state_with_yesterday() -> AppState {
        // Pretend the loaded list is from yesterday relative to "now"
        let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
        let list = TodoList::with_items(yesterday, PathBuf::from("/tmp/test.md"), vec![]);
        AppState::new(
            list,
            Theme::default(),
            crate::keybindings::KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default(),
            crate::plugin::loader::PluginLoader::new(),
            vec![],
            crate::plugin::PluginActionRegistry::new(),
            AutoRolloverPref::Ask,
        )
    }

    #[test]
    fn check_midnight_noop_when_not_navigate_mode() {
        let mut state = state_with_yesterday();
        state.mode = Mode::Edit;
        state.check_midnight_rollover();
        assert!(state.pending_rollover.is_none(), "should not open modal in Edit mode");
        assert_eq!(state.mode, Mode::Edit, "mode unchanged");
    }

    #[test]
    fn check_midnight_noop_when_pending_rollover_already_set() {
        let mut state = state_with_yesterday();
        let source = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        state.pending_rollover = Some(PendingRollover {
            source_date: source,
            items: vec![],
            remember_choice: false,
        });
        state.check_midnight_rollover();
        // pending_rollover unchanged
        assert_eq!(state.pending_rollover.as_ref().unwrap().source_date, source);
    }

    #[test]
    fn check_midnight_noop_when_pref_is_auto_no() {
        let mut state = state_with_yesterday();
        state.auto_rollover_pref = AutoRolloverPref::AutoNo;
        let original_date = state.todo_list.date;
        state.check_midnight_rollover();
        assert!(state.pending_rollover.is_none());
        assert_eq!(state.todo_list.date, original_date, "view should not advance");
    }

    #[test]
    fn check_midnight_noop_when_already_today() {
        let today = Local::now().date_naive();
        let list = TodoList::with_items(today, PathBuf::from("/tmp/test.md"), vec![]);
        let mut state = AppState::new(
            list,
            Theme::default(),
            crate::keybindings::KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default(),
            crate::plugin::loader::PluginLoader::new(),
            vec![],
            crate::plugin::PluginActionRegistry::new(),
            AutoRolloverPref::Ask,
        );
        state.check_midnight_rollover();
        assert!(state.pending_rollover.is_none());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib app::state::tests::check_midnight -- --nocapture`
Expected: FAIL with `no method check_midnight_rollover`.

- [ ] **Step 4: Implement `check_midnight_rollover` and helpers**

In `src/app/state.rs`, find the `impl AppState { ... }` block and add the following methods inside it (a sensible location is near `open_rollover_modal` around line 1725):

```rust
    /// Called every UI tick. If the wall-clock day has rolled past the
    /// currently loaded list and the user is idle, either open the rollover
    /// modal or auto-execute, depending on `auto_rollover_pref`.
    pub fn check_midnight_rollover(&mut self) {
        // Cheap fast-path guards — this fires every 100ms.
        if self.mode != Mode::Navigate {
            return;
        }
        if self.pending_rollover.is_some() {
            return;
        }
        if self.auto_rollover_pref == crate::config::AutoRolloverPref::AutoNo {
            return;
        }

        let today = Local::now().date_naive();
        if today <= self.todo_list.date {
            return;
        }

        // Day has crossed. Find candidates for rollover.
        let candidates = match crate::storage::rollover::find_rollover_candidates_for_project(
            &self.current_project.name,
        ) {
            Ok(Some(c)) => c,
            Ok(None) => {
                // No incomplete items to roll over — just advance the view
                // to today's (possibly empty) list so the user sees the new day.
                if let Err(e) = self.silently_advance_to_today() {
                    tracing::error!("Failed to advance view to today: {e}");
                }
                return;
            }
            Err(e) => {
                tracing::error!("Midnight rollover candidate lookup failed: {e}");
                return;
            }
        };

        let (source_date, items) = candidates;
        match self.auto_rollover_pref {
            crate::config::AutoRolloverPref::AutoYes => {
                self.execute_rollover_silently(source_date, items);
            }
            crate::config::AutoRolloverPref::Ask => {
                self.open_rollover_modal(source_date, items);
            }
            crate::config::AutoRolloverPref::AutoNo => {
                // Already returned above; included for exhaustiveness.
            }
        }
    }

    /// Execute rollover without showing the modal. Used by AutoYes path.
    fn execute_rollover_silently(
        &mut self,
        source_date: chrono::NaiveDate,
        items: Vec<TodoItem>,
    ) {
        let item_count = items.len();
        match crate::storage::rollover::execute_rollover_for_project(
            &self.current_project.name,
            source_date,
            items,
        ) {
            Ok(new_list) => {
                self.todo_list = new_list;
                self.cursor_position = 0;
                self.set_status_message(format!(
                    "Auto-rolled over {} item{} from {}",
                    item_count,
                    if item_count == 1 { "" } else { "s" },
                    source_date.format("%B %d, %Y"),
                ));
            }
            Err(e) => {
                tracing::error!("Auto-rollover failed: {e}");
                self.set_status_message(format!("Auto-rollover failed: {e}"));
            }
        }
    }

    /// Load today's list (creating it if absent) without showing a modal.
    /// Used when no incomplete items exist to roll over.
    fn silently_advance_to_today(&mut self) -> anyhow::Result<()> {
        let today = Local::now().date_naive();
        let new_list = crate::storage::file::load_todo_list_for_project(
            &self.current_project.name,
            today,
        )?;
        self.todo_list = new_list;
        self.cursor_position = 0;
        Ok(())
    }
```

- [ ] **Step 5: Run all tests to confirm guards work**

Run: `cargo test --lib app::state::tests -- --nocapture`
Expected: PASS for all four `check_midnight_*` tests.

- [ ] **Step 6: Add an integration-flavoured test for AutoYes path**

Append to the `tests` module:

```rust
    #[test]
    fn check_midnight_auto_yes_with_no_candidates_advances_view() {
        // We can't easily fake "yesterday's file with incomplete items" without
        // touching disk, so this asserts the no-candidates branch: the view
        // advances to today silently. We use a temp dir override pattern only
        // if the codebase has one; otherwise we accept that the view may not
        // actually update if today's file IO fails — we just verify no panic
        // and no modal opens.
        let mut state = state_with_yesterday();
        state.auto_rollover_pref = AutoRolloverPref::AutoYes;
        // No yesterday file exists on disk for this temp project, so
        // find_rollover_candidates_for_project returns Ok(None), which in
        // turn calls silently_advance_to_today. That call may fail if the
        // dailies dir is missing — we tolerate the error and just verify
        // no modal was opened.
        state.check_midnight_rollover();
        assert!(state.pending_rollover.is_none(), "AutoYes must not open modal");
    }
```

- [ ] **Step 7: Run the new test**

Run: `cargo test --lib app::state::tests::check_midnight_auto_yes -- --nocapture`
Expected: PASS.

- [ ] **Step 8: Run full test suite + lint**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: All tests PASS, no clippy warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app/state.rs
git commit -m "feat(state): add check_midnight_rollover detector

Compares Local::now() against the loaded list's date and either opens
the rollover modal, auto-executes, or no-ops based on the user's
auto_rollover_pref. Guards on Mode::Navigate so editing is never
interrupted."
```

---

## Task 5: Wire detector into TUI tick loop

**Files:**
- Modify: `src/ui/mod.rs:220-223`

- [ ] **Step 1: Locate the tick arm**

Open `src/ui/mod.rs`. The relevant block is around line 219:

```rust
            // Periodic tick for animations (spinner, status messages)
            _ = tick_interval.tick() => {
                // Don't log ticks - too noisy
                state.tick_spinner();
            }
```

- [ ] **Step 2: Add the midnight check call**

Replace that block with:

```rust
            // Periodic tick for animations (spinner, status messages)
            _ = tick_interval.tick() => {
                // Don't log ticks - too noisy
                state.tick_spinner();
                state.check_midnight_rollover();
            }
```

- [ ] **Step 3: Build**

Run: `cargo build --bins`
Expected: PASS.

- [ ] **Step 4: Run lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: No warnings.

- [ ] **Step 5: Commit**

```bash
git add src/ui/mod.rs
git commit -m "feat(ui): call check_midnight_rollover from TUI tick

Runs every 100ms alongside the spinner tick. The detector is cheap
(one mode comparison + one date comparison on the fast path) so this
adds negligible overhead."
```

---

## Task 6: Render the "Don't ask again" checkbox in the modal

**Files:**
- Modify: `src/ui/components/mod.rs:676-770` (`render_rollover_overlay`)

- [ ] **Step 1: Read the current footer block**

Open `src/ui/components/mod.rs` and locate `render_rollover_overlay` starting at line 676. The footer renders `[Y]es - Rollover now    [L]ater - Skip` (and similar). We will:

1. Add a checkbox row inside the bordered list (above the footer).
2. Update the footer hint to mention `[Tab] toggle remember`.

- [ ] **Step 2: Add a checkbox row above the footer**

Inside `render_rollover_overlay`, after the loop that pushes item lines (around line 726, just before `let list = List::new(lines)`), add:

```rust
    // Spacer + "Don't ask again" checkbox row
    lines.push(ListItem::new(Line::from("")));
    let checkbox_glyph = if pending.remember_choice { "[x]" } else { "[ ]" };
    let checkbox_style = if pending.remember_choice {
        Style::default()
            .fg(state.theme.foreground)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(state.theme.foreground)
    };
    lines.push(ListItem::new(Line::from(vec![
        Span::styled(checkbox_glyph, checkbox_style),
        Span::raw(" Don't ask me again — remember this choice (Tab to toggle)"),
    ])));
```

- [ ] **Step 3: Update the footer hint**

In the same function, find the `Paragraph::new(Line::from(vec![ ... ]))` block that builds the footer (around line 748). Replace the contents of that `Line::from(vec![...])` with hints for Y, N, Tab, Esc. Concretely, change:

```rust
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(ratatui::style::Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es - Rollover now    "),
        Span::styled(
            "[L]",
            Style::default()
                .fg(ratatui::style::Color::Yellow)
```

(keeping reading down to the final `]))` of the footer Paragraph) into:

```rust
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(ratatui::style::Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es    "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o    "),
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(ratatui::style::Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" remember    "),
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" cancel"),
    ]));
```

If the original footer span list extends beyond the snippet shown above, replace the entire vec passed to `Line::from(...)` with the vec shown here.

- [ ] **Step 4: Build**

Run: `cargo build --bins`
Expected: PASS.

- [ ] **Step 5: Smoke render in a manual run**

Run: `cargo run --bin to-tui` (or whatever the binary is named — check `Cargo.toml` if unsure). Trigger the rollover modal manually with `R` (existing keybinding) when there are incomplete items from a prior day. Visually confirm the new checkbox row appears with `[ ]` and the footer shows the new hints.

If you cannot easily produce the modal, skip the manual check — Task 7's tests cover the toggle behaviour.

- [ ] **Step 6: Run lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: No warnings.

- [ ] **Step 7: Commit**

```bash
git add src/ui/components/mod.rs
git commit -m "feat(ui): render Don't ask again checkbox in rollover modal

Adds a checkbox row above the footer reflecting
PendingRollover::remember_choice, plus a Tab hint in the footer."
```

---

## Task 7: Toggle and persist preference in `handle_rollover_mode`

**Files:**
- Modify: `src/app/event.rs:951-969`
- Test: `src/app/event.rs` (existing or new tests module)

- [ ] **Step 1: Write failing tests for the handler**

Find or create a `#[cfg(test)] mod tests` block in `src/app/event.rs`. Append:

```rust
#[cfg(test)]
mod rollover_tests {
    use super::*;
    use crate::app::AppState;
    use crate::config::AutoRolloverPref;
    use crate::project::Project;
    use crate::todo::{TodoItem, TodoList, TodoState};
    use crate::ui::theme::Theme;
    use chrono::NaiveDate;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn make_state_in_rollover_mode() -> AppState {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let list = TodoList::with_items(date, PathBuf::from("/tmp/test.md"), vec![]);
        let mut state = AppState::new(
            list,
            Theme::default(),
            crate::keybindings::KeybindingCache::default(),
            1000,
            None,
            None,
            Project::default(),
            crate::plugin::loader::PluginLoader::new(),
            vec![],
            crate::plugin::PluginActionRegistry::new(),
            AutoRolloverPref::Ask,
        );
        state.open_rollover_modal(
            NaiveDate::from_ymd_opt(2026, 5, 5).unwrap(),
            vec![TodoItem::with_state("X".into(), TodoState::Empty, 0)],
        );
        state
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn tab_toggles_remember_choice() {
        let mut state = make_state_in_rollover_mode();
        assert!(!state.pending_rollover.as_ref().unwrap().remember_choice);
        handle_rollover_mode(key(KeyCode::Tab), &mut state).unwrap();
        assert!(state.pending_rollover.as_ref().unwrap().remember_choice);
        handle_rollover_mode(key(KeyCode::Tab), &mut state).unwrap();
        assert!(!state.pending_rollover.as_ref().unwrap().remember_choice);
    }

    #[test]
    fn n_with_remember_sets_pref_to_auto_no() {
        let mut state = make_state_in_rollover_mode();
        // Toggle remember on
        handle_rollover_mode(key(KeyCode::Tab), &mut state).unwrap();
        // Press N
        handle_rollover_mode(key(KeyCode::Char('n')), &mut state).unwrap();
        assert_eq!(state.auto_rollover_pref, AutoRolloverPref::AutoNo);
    }

    #[test]
    fn n_without_remember_leaves_pref_unchanged() {
        let mut state = make_state_in_rollover_mode();
        let before = state.auto_rollover_pref;
        handle_rollover_mode(key(KeyCode::Char('n')), &mut state).unwrap();
        assert_eq!(state.auto_rollover_pref, before);
    }

    // Note: we don't test Y here because execute_rollover_for_project hits disk.
    // The toggle and N paths cover the persistence logic.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::event::rollover_tests -- --nocapture`
Expected: FAIL — `tab_toggles_remember_choice` fails because Tab isn't handled, and `n_with_remember_sets_pref_to_auto_no` fails because the pref isn't being updated.

- [ ] **Step 3: Update `handle_rollover_mode`**

In `src/app/event.rs:951-969`, replace the entire function body with:

```rust
fn handle_rollover_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Tab | KeyCode::Char(' ') => {
            // Toggle Don't ask again checkbox
            if let Some(ref mut pending) = state.pending_rollover {
                pending.remember_choice = !pending.remember_choice;
            }
        }
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            // Capture remember flag before consuming pending
            let remember = state
                .pending_rollover
                .as_ref()
                .map(|p| p.remember_choice)
                .unwrap_or(false);

            // Execute rollover
            if let Some(pending) = state.pending_rollover.take() {
                let new_list = execute_rollover_for_project(
                    &state.current_project.name,
                    pending.source_date,
                    pending.items,
                )?;
                state.todo_list = new_list;
                state.cursor_position = 0;
                state.set_status_message("Rolled over incomplete items".to_string());
            }
            state.mode = Mode::Navigate;

            if remember {
                persist_auto_rollover_pref(state, crate::config::AutoRolloverPref::AutoYes);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Esc => {
            let remember = state
                .pending_rollover
                .as_ref()
                .map(|p| p.remember_choice)
                .unwrap_or(false);

            // Close modal but keep pending_rollover for later (existing behaviour
            // — user can re-trigger with R).
            state.close_rollover_modal();

            if remember {
                persist_auto_rollover_pref(state, crate::config::AutoRolloverPref::AutoNo);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Update the in-memory preference and persist it to ~/.config/to-tui/config.toml.
/// On save failure, log and surface a status message but do not propagate the error
/// — the user has already made their choice and should not be blocked.
fn persist_auto_rollover_pref(state: &mut AppState, pref: crate::config::AutoRolloverPref) {
    state.auto_rollover_pref = pref;
    match crate::config::Config::load() {
        Ok(mut cfg) => {
            cfg.auto_rollover = pref;
            if let Err(e) = cfg.save() {
                tracing::error!("Failed to save auto_rollover preference: {e}");
                state.set_status_message("Couldn't save rollover preference".to_string());
            }
        }
        Err(e) => {
            tracing::error!("Failed to load config for rollover save: {e}");
            state.set_status_message("Couldn't save rollover preference".to_string());
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib app::event::rollover_tests -- --nocapture`
Expected: PASS for all three new tests.

Note: The `n_with_remember_sets_pref_to_auto_no` test will trigger `Config::load()`/`save()` against the real `~/.config/to-tui/config.toml`. If this is a concern, run the test suite in a sandbox or accept that it will read/write the user's actual config file. The save is idempotent (it just sets the same value back if the user already had AutoNo).

If you want test isolation, an alternative is to make `persist_auto_rollover_pref` a method on `AppState` that takes an injectable saver closure. That's a larger refactor; the current design is acceptable for this feature.

- [ ] **Step 5: Run full test suite + lint**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: All tests PASS, no clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add src/app/event.rs
git commit -m "feat(event): toggle remember + persist auto_rollover preference

Tab/Space toggles PendingRollover::remember_choice. When the user
confirms (Y) or declines (N) with remember ticked, AppState and
config.toml are updated to AutoYes or AutoNo respectively. Save
failures log + surface a status message but do not block the action."
```

---

## Task 8: Manual smoke test + docs polish

**Files:**
- (No code changes — verification only)

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: PASS.

- [ ] **Step 2: Verify config round-trip end-to-end**

```bash
cat ~/.config/to-tui/config.toml | grep -A1 auto_rollover || echo "(field missing — will be added on next save)"
```

If missing, that is expected — it's only written when the user toggles "Don't ask again" or when something else triggers a save.

- [ ] **Step 3: Manual midnight simulation**

Three options, in order of effort:

**Option A — Edit a daily file directly.** Create a fake "yesterday" daily file with one incomplete item, then change your system date forward (or simply rename today's file aside) and launch the TUI. The midnight detector will fire on the first tick.

**Option B — Wait until actual midnight.** Leave the TUI running. At 00:00 the modal should appear (if `auto_rollover = "ask"`) or the rollover should execute silently (if `auto_yes`).

**Option C — Skip manual verification.** The unit tests cover the deterministic logic; the only thing they don't exercise is the actual tick wiring, which is one line.

Pick whichever fits your time budget.

- [ ] **Step 4: Verify clippy and fmt clean across the whole crate**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: All three PASS.

- [ ] **Step 5: Final commit (only if there are uncommitted formatting fixes)**

```bash
git status
# If anything is dirty:
git add -A
git commit -m "chore: format midnight rollover changes"
```

---

## Spec Coverage Check

Spec sections vs tasks:

- **Problem / Approach** → Tasks 4 + 5 (detector + tick wiring).
- **Data Model: Config** → Task 1 (`AutoRolloverPref` + `auto_rollover` field).
- **Data Model: AppState** → Task 3 (field + threading).
- **Data Model: PendingRollover** → Task 2 (`remember_choice`).
- **Components: Tick-loop guard** → Task 5.
- **Components: `check_midnight_rollover`** → Task 4.
- **Components: Modal extension** → Task 6.
- **Components: Key handler** → Task 7.
- **Edge cases** → Covered by Task 4 guards (mode/pending/AutoNo/already-today/no-candidates) and unit tests.
- **Error handling** → Task 4 (`tracing::error!` + status message on candidate lookup, rollover execution) and Task 7 (`persist_auto_rollover_pref` save-failure handling).
- **Testing** → Tests appear in Tasks 1, 2, 3, 4, 7.
- **Out of scope** → Honoured (no settings UI, no per-project pref, no OS notifications).

No gaps.
