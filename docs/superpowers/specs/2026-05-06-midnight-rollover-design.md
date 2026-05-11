# Midnight Rollover Design

**Date:** 2026-05-06
**Status:** Draft

## Problem

Today, rollover only fires at TUI startup or project switch (`src/main.rs:311`,
`src/app/state.rs:1780`). A user who leaves the TUI open across midnight stays
on yesterday's file until they restart the app. They want the TUI to detect
the day change live, prompt them to roll over, and optionally remember the
choice so future midnights happen automatically.

Two non-functional constraints:

- The prompt must not interrupt active editing — wait until the user is in
  Navigate mode (which guarantees the file is saved, since saves are eager
  in `handle_key_event`).
- The "Don't ask again" choice must persist across restarts.

## Approach

Reuse the existing 100ms tick in `src/ui/mod.rs:120`. Each tick, compare
`Local::now().date_naive()` against `state.todo_list.date`. When a date
crossover is detected and the user is idle, either:

- **Ask** (default) — open the existing rollover modal, augmented with a
  "Don't ask again" checkbox.
- **AutoYes** — execute the rollover silently, leave a status-bar confirmation.
- **AutoNo** — do nothing; the user can still trigger rollover manually with
  `R` (existing keybinding).

This reuses `find_rollover_candidates_for_project` and
`execute_rollover_for_project` unchanged. The new code is a tick-loop guard,
a config field, and modal extensions.

## Data Model

### Config (`src/config.rs`)

New field on `Config`:

```rust
#[serde(default)]
pub auto_rollover: AutoRolloverPref,
```

New enum (same file):

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoRolloverPref {
    #[default]
    Ask,
    AutoYes,
    AutoNo,
}
```

Persisted in `~/.config/to-tui/config.toml` alongside `theme`, `last_used_project`, etc.

### AppState (`src/app/state.rs`)

New field:

```rust
pub auto_rollover_pref: AutoRolloverPref,
```

Threaded through `AppState::new(...)` from `Config` at startup
(`src/main.rs` around line 297).

### PendingRollover (`src/app/state.rs:115`)

Extend with checkbox state:

```rust
pub struct PendingRollover {
    pub source_date: NaiveDate,
    pub items: Vec<TodoItem>,
    pub remember_choice: bool,  // toggled by user inside the modal
}
```

`open_rollover_modal` initialises `remember_choice: false`.

## Components

### Tick-loop guard (`src/ui/mod.rs`)

In the `tick_interval.tick()` arm of the `tokio::select!` (currently just
calls `state.tick_spinner()`), add a call:

```rust
state.check_midnight_rollover();
```

The function lives on `AppState` so it has access to mode, project, and
pending state.

### `AppState::check_midnight_rollover` (`src/app/state.rs`)

```rust
pub fn check_midnight_rollover(&mut self) {
    // Cheap fast-path checks first — this fires every 100ms.
    if self.mode != Mode::Navigate { return; }
    if self.pending_rollover.is_some() { return; }
    if self.auto_rollover_pref == AutoRolloverPref::AutoNo { return; }

    let today = Local::now().date_naive();
    if today <= self.todo_list.date { return; }

    // Day has crossed — find candidates.
    let candidates = match find_rollover_candidates_for_project(&self.current_project.name) {
        Ok(Some(c)) => c,
        Ok(None) => {
            // Nothing to rollover — but we still need to advance the view.
            // Load today's (possibly empty) list silently.
            self.silently_advance_to_today();
            return;
        }
        Err(e) => {
            tracing::error!("Midnight rollover candidate lookup failed: {e}");
            return;
        }
    };

    let (source_date, items) = candidates;
    match self.auto_rollover_pref {
        AutoRolloverPref::AutoYes => {
            self.execute_rollover_silently(source_date, items);
        }
        AutoRolloverPref::Ask => {
            self.open_rollover_modal(source_date, items);
        }
        AutoRolloverPref::AutoNo => unreachable!(), // guarded above
    }
}
```

Helpers:

- `execute_rollover_silently(source_date, items)` — calls
  `execute_rollover_for_project`, replaces `self.todo_list`, sets a status
  message ("Auto-rolled over N items from <date>"), resets cursor.
- `silently_advance_to_today()` — loads today's file (creating it if absent
  via existing `load_todo_list_for_project`) without showing a modal.
  Needed because otherwise the AutoNo path would leave the user stuck on
  yesterday's file forever; for AutoYes/Ask it's only used when there are
  no incomplete items.

For AutoNo we don't auto-advance the view — the user explicitly opted out.
They can hit `R` or restart to advance.

### Modal extension (`src/ui/components/mod.rs:676`)

`render_rollover_overlay` gains a checkbox line above the footer:

```
[x] Don't ask me again — remember this choice
```

Box state reads from `pending.remember_choice`. Footer gets a hint:

```
[Y]es - Rollover now    [N]o - Skip    [Tab] toggle remember    [Esc] close
```

### Key handler (`src/app/event.rs:951`)

`handle_rollover_mode` grows two changes:

1. `Tab` (or `Space`) toggles `pending.remember_choice`.
2. On `Y`/`Enter` and `N`/`L`/`Esc`, if `remember_choice` is true, persist
   the user's answer to config:
   - Y + remember → `AutoRolloverPref::AutoYes`
   - N + remember → `AutoRolloverPref::AutoNo`
   Then update `state.auto_rollover_pref` and call `Config::save()`. On
   save failure, log and show a status message but don't block the action.

## Data Flow

```
TUI tick (every 100ms)
  └─> AppState::check_midnight_rollover
        ├─ guard: Navigate mode? no pending modal? not AutoNo?
        ├─ today > loaded date?
        ├─ find_rollover_candidates_for_project
        ├─ branch on auto_rollover_pref:
        │    ├─ Ask     → open_rollover_modal (existing)
        │    └─ AutoYes → execute_rollover_silently
        │
        └─ on Ask path, user resolves modal in handle_rollover_mode:
              ├─ Y + remember=true  → pref := AutoYes, save config
              ├─ N + remember=true  → pref := AutoNo,  save config
              └─ pref unchanged otherwise
```

## Edge Cases

- **System sleep across midnight.** Tokio interval is wall-clock based via
  `tokio::time`; on wake, `tick_interval.tick()` resumes and the next tick
  detects the date difference. Treated identically to "user just opened the
  app" — handled correctly.
- **No incomplete items at midnight.** `find_rollover_candidates_for_project`
  returns `Ok(None)`. We silently advance to today's file (Ask/AutoYes)
  rather than spamming a modal with zero items. AutoNo leaves the view alone.
- **Already in `Mode::Rollover` from startup.** The first guard
  (`pending_rollover.is_some()`) skips the tick-driven check, so we don't
  double-open.
- **Project switch during the day.** Existing project-switch rollover logic
  in `state.rs:1780` is unchanged; it already opens the modal if needed.
- **Multi-day gap (laptop closed Friday, opened Monday).** Same as today —
  `find_rollover_candidates_for_project` already walks back up to 30 days
  for the most recent file with incomplete items.
- **Today's file does not yet exist at 00:00.** `find_rollover_candidates_for_project`
  short-circuits when today's file *does* exist (`storage/rollover.rs:20`).
  At midnight while the TUI has been running, today's file has not been
  created — so the function proceeds normally to find yesterday's
  incomplete items. After rollover runs, today's file is written.
- **Config save races.** Saves are infrequent (only when toggling remember).
  No locking needed; last-write-wins is acceptable.

## Error Handling

- Candidate lookup failure (`find_rollover_candidates_for_project` returns
  `Err`): log via `tracing::error!`, do nothing, retry naturally on the
  next tick. Don't crash the UI.
- Rollover execution failure (`execute_rollover_for_project` returns
  `Err`): log, set a status message, leave `pending_rollover` cleared so
  the user can re-trigger with `R`.
- Config save failure: log, set a status message ("Couldn't save rollover
  preference"). The in-memory `auto_rollover_pref` still updates so the
  user's choice applies for the rest of the session.

## Testing

Unit tests in `src/app/state.rs` (or a new `tests/midnight_rollover.rs`):

- `check_midnight_rollover` no-ops when `mode != Navigate`.
- No-ops when `pending_rollover` is already set.
- No-ops when `auto_rollover_pref == AutoNo`.
- No-ops when `todo_list.date == today`.
- When `AutoYes` and date has crossed, items are rolled over and a status
  message is set — drive via injectable "today" function or fake clock.
- When `Ask` and date has crossed, modal opens with `remember_choice: false`.

Modal interaction tests in `src/app/event.rs`:

- `Tab` toggles `pending.remember_choice`.
- Y + remember=true updates `state.auto_rollover_pref` to `AutoYes`.
- N + remember=true updates it to `AutoNo`.
- Y + remember=false leaves `auto_rollover_pref` unchanged.

Config round-trip in `src/config.rs`:

- `AutoRolloverPref::AutoYes` serialises as `auto_rollover = "auto_yes"`
  and round-trips.
- Missing field deserialises as `Ask`.

Manual verification: run `cargo run`, manipulate system clock (or shim
`Local::now`), confirm modal appears, checkbox toggles, choice persists
across restart.

## Out of Scope

- Per-project rollover preferences (decided global is sufficient).
- A settings UI to flip the preference back to Ask after the user picks
  AutoYes/AutoNo. The user can edit `~/.config/to-tui/config.toml`
  directly; a settings UI is a separate feature.
- Notifying via OS notification at midnight — TUI only.
- Network/cloud sync of the preference.
