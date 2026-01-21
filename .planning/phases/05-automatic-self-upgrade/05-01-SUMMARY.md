---
phase: 05-automatic-self-upgrade
plan: 01
subsystem: upgrade-infrastructure
tags: [download, async, progress-tracking, dependencies]

dependency-graph:
  requires: []
  provides: [upgrade-types, download-function, progress-channel]
  affects: [05-02, 05-03]

tech-stack:
  added: [self_update, futures-util, tempfile]
  patterns: [async-streaming, mpsc-channel, platform-detection]

key-files:
  created:
    - src/utils/upgrade.rs
  modified:
    - Cargo.toml
    - src/utils/mod.rs

decisions:
  - id: reqwest-stream-feature
    choice: Added stream feature to reqwest for bytes_stream()
    rationale: Required for chunked streaming downloads with progress

metrics:
  duration: 2 min
  completed: 2026-01-21
---

# Phase 5 Plan 1: Upgrade Download Infrastructure Summary

Async binary download with progress tracking via mpsc channel, plus state types for TUI integration.

## What Was Built

### UpgradeSubState Enum
State machine for the upgrade workflow UI:
- `Prompt` - Initial Y/N/S options (existing behavior)
- `Downloading { progress, bytes_downloaded, total_bytes }` - Progress bar state
- `Error { message }` - Download failure with retry option
- `RestartPrompt { downloaded_path }` - Download complete, prompt to restart

### DownloadProgress Messages
Channel messages for async download communication:
- `Progress { bytes, total }` - Incremental progress updates
- `Complete { path }` - Download finished successfully
- `Error { message }` - Download failed

### Download Functions

**`get_asset_download_url(version: &str) -> String`**
- Uses compile-time platform detection (`cfg!(target_os)`, `cfg!(target_arch)`)
- Supports: macOS ARM, macOS Intel, Linux x86_64
- Returns GitHub release asset URL: `https://github.com/grimurjonsson/to-tui/releases/download/v{version}/totui-{target}.tar.gz`

**`spawn_download(url: String, target_path: PathBuf) -> mpsc::Receiver<DownloadProgress>`**
- Creates tokio mpsc channel (buffer 32)
- Spawns async task with reqwest streaming download
- Rate-limits progress updates to every 100KB
- Returns receiver for TUI event loop integration

**`format_bytes(bytes: u64) -> String`**
- Human-readable formatting: "1.5 MB", "256 KB", "100 B"

### Dependencies Added
- `self_update = "0.41"` - Archive extraction utilities (tar.gz handling for Plan 03)
- `futures-util = "0.3"` - StreamExt trait for reqwest bytes_stream()
- `tempfile = "3.13"` - Moved from dev-deps to deps (needed for extract_binary in Plan 03)
- `reqwest` - Added `stream` feature for bytes_stream() method

## Key Implementation Details

```rust
// Platform-specific URL construction
pub fn get_asset_download_url(version: &str) -> String {
    let target = get_target_triple();  // e.g., "aarch64-apple-darwin"
    format!(
        "https://github.com/{}/releases/download/v{}/totui-{}.tar.gz",
        GITHUB_REPO, version, target
    )
}

// Async streaming download with progress
async fn download_file(url: &str, target_path: &PathBuf, tx: &mpsc::Sender<DownloadProgress>) {
    let response = client.get(url).send().await?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk).await?;
        // Rate-limited progress updates
        if downloaded - last_progress_at >= 100KB {
            tx.send(DownloadProgress::Progress { ... }).await;
        }
    }
    tx.send(DownloadProgress::Complete { path }).await;
}
```

## Tests Added

3 new tests in `src/utils/upgrade.rs`:
- `test_format_bytes` - Byte formatting edge cases (0 B through GB)
- `test_get_asset_download_url` - URL structure validation
- `test_get_target_triple` - Platform detection verification

Plus 1 doc test for `format_bytes()` examples.

## Deviations from Plan

**[Rule 3 - Blocking] Added reqwest stream feature**
- **Issue:** `bytes_stream()` method not available without `stream` feature
- **Fix:** Added `stream` to reqwest features in Cargo.toml
- **Commit:** cb2b49c

## Commits

| Hash | Type | Description |
|------|------|-------------|
| c233e9e | chore | Add upgrade dependencies (self_update, futures-util, tempfile) |
| cb2b49c | feat | Add upgrade module with download infrastructure |

## Files Changed

| File | Change |
|------|--------|
| Cargo.toml | Added 3 dependencies, 1 feature |
| Cargo.lock | Updated with new dependency tree |
| src/utils/mod.rs | Added `pub mod upgrade` |
| src/utils/upgrade.rs | New file (230 lines) |

## Next Phase Readiness

Ready for Plan 02 (TUI Integration):
- `UpgradeSubState` ready to integrate with existing `UpgradePrompt` mode
- `spawn_download()` returns receiver compatible with async TUI polling
- `format_bytes()` available for progress display

Plan 03 (Binary Replacement) will use:
- `self_update` crate for tar.gz extraction
- `tempfile` for safe temporary file handling
- `DownloadProgress::Complete` path for extraction source
