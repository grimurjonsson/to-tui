# Plan 05-03 Summary: Complete UI rendering and binary replacement/restart

## Status: Complete

## Duration: ~15 min (including debugging)

## Tasks Completed

| # | Task | Commit |
|---|------|--------|
| 1 | Update render_upgrade_overlay for all sub-states | 0c49da0 |
| 2 | Add binary preparation and restart functions | 8879f57 |
| 3 | Wire up restart in event handler | 9ecda43 |
| 4 | Human verification checkpoint | APPROVED |

## Additional Fixes

| Issue | Fix | Commit |
|-------|-----|--------|
| Release assets are raw binaries, not .tar.gz | Changed URL format, replaced extract_binary with prepare_binary | 3f0eee6 |
| No crash logging for debugging | Added panic hook writing to ~/.to-tui/crash.log | ce078f3 |
| tokio::spawn requires runtime (TUI is sync) | Changed to std::thread with blocking reqwest | 05d398a |
| Redundant self-replace dependency | Use self_update's re-exported self_replace | 9d62863 |

## Files Modified

- `src/ui/components/mod.rs` - Sub-state rendering (Gauge progress bar, error modal, restart prompt)
- `src/utils/upgrade.rs` - prepare_binary(), replace_and_restart(), check_write_permission()
- `src/app/event.rs` - RestartPrompt handler calls prepare + replace_and_restart
- `src/app/state.rs` - Changed to std::sync::mpsc for thread-based download
- `src/main.rs` - Crash handler with backtrace logging
- `src/utils/paths.rs` - get_crash_log_path()
- `Cargo.toml` - Removed self-replace (use self_update's re-export)

## Key Decisions

- Raw binary download (no archive extraction needed) - matches actual GitHub release assets
- Crash logs at ~/.to-tui/crash.log with timestamp, message, location, and backtrace
- Blocking download in std::thread (TUI has no tokio runtime)
- Use self_update::self_replace instead of separate dependency

## Verification

- [x] `cargo check` passes
- [x] `cargo test` passes (108 tests)
- [x] Progress bar renders during download
- [x] Error messages display correctly
- [x] Restart prompt appears after download
- [x] Binary replacement works (atomic via self_replace)
- [x] Application restarts instantly (exec() replaces process)
- [x] Crash handler logs panics with full backtrace

## What Was Built

Complete automatic self-upgrade workflow:
1. Y in upgrade modal checks write permissions, starts background download
2. Progress bar shows bytes downloaded / total with Gauge widget
3. Download errors show red modal with retry/dismiss options
4. After download, restart prompt confirms before replacing
5. Y at restart: prepare binary (set permissions) → atomic replace → exec() restart
6. N at restart: clean up downloaded file, dismiss modal
7. Esc during download: cancel and return to normal mode
8. Crash handler captures any panics to log file for debugging
