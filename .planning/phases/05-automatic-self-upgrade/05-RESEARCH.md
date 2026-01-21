# Phase 5: Automatic Self-Upgrade - Research

**Researched:** 2026-01-21
**Domain:** Binary self-update, HTTP streaming downloads, process replacement
**Confidence:** HIGH

## Summary

This phase implements automatic self-upgrade functionality for the to-tui TUI application. When a user accepts an upgrade in the existing upgrade prompt modal (from Quick-002), the application will download the new version binary from GitHub releases, show download progress, then offer to restart with the new version.

The research identifies that the `self_update` crate is the established solution for Rust binary self-updating from GitHub releases. However, since this is a TUI application (not CLI), we cannot use `self_update`'s built-in `indicatif` progress bars. Instead, we'll use the low-level download utilities from `self_update` combined with ratatui's `Gauge` widget for progress display within the existing modal system.

**Primary recommendation:** Use `self_update` crate for GitHub release asset discovery and binary replacement, but implement custom progress tracking through TUI state updates rather than `indicatif`.

## Standard Stack

The established libraries for self-updating Rust binaries:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| self_update | 0.27+ | GitHub release backend, binary replacement | Most mature and widely used; handles platform detection, archive extraction |
| self-replace | 1.3+ | Cross-platform binary replacement | Re-exported by self_update; handles Unix/Windows differences atomically |
| reqwest | 0.12+ | HTTP client (already in project) | Used internally by self_update; project already has it |
| futures-util | 0.3+ | StreamExt for chunked downloads | Required for `bytes_stream()` progress tracking |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile | 3+ | Temporary file handling (already in project) | For staging downloads before replacement |
| tokio | 1+ | Async runtime (already in project) | Background download without blocking TUI |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| self_update | Manual implementation | Would need to reimplement archive extraction, platform detection, GitHub API |
| self-replace | std::fs::rename | Would not handle Windows edge cases (running binary locked) |
| indicatif | ratatui Gauge | indicatif is CLI-focused; Gauge integrates with existing TUI |

**Installation:**
```bash
cargo add self_update --features "rustls,archive-tar,compression-flate2" --no-default-features
cargo add futures-util
```

Note: `self_update` with `rustls` feature avoids OpenSSL dependencies. The project already uses `reqwest` with `rustls-tls`.

## Architecture Patterns

### Recommended State Machine

The upgrade process follows a state machine pattern, extending the existing `Mode::UpgradePrompt`:

```
                                  User presses Y
UpgradePrompt (view release) ──────────────────────> Downloading
       │                                                  │
       │ User presses N                                   │ progress updates
       v                                                  v
   Navigate                                          Downloading
       ^                                                  │
       │                                                  │ complete
       │ User presses N                                   v
       └─────────────────── RestartPrompt <───────────────┘
                                  │
                                  │ User presses Y
                                  v
                             [Exit + Restart]
```

### Recommended Sub-State Structure

```rust
// Extend existing upgrade handling with sub-states
pub enum UpgradeSubState {
    /// Initial prompt: "New version available, upgrade? (Y/n)"
    Prompt,
    /// Downloading: shows progress bar
    Downloading {
        progress: f64,        // 0.0 to 1.0
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    /// Download failed
    Error { message: String },
    /// Download complete: "Restart now? (Y/n)"
    RestartPrompt {
        downloaded_path: PathBuf,
    },
}
```

### Recommended Project Structure
```
src/
├── app/
│   ├── state.rs           # Add UpgradeSubState, upgrade_state field
│   └── event.rs           # Add upgrade mode event handlers
├── ui/
│   └── components/
│       └── mod.rs         # Extend render_upgrade_overlay with sub-states
├── utils/
│   └── upgrade.rs         # NEW: Download logic, GitHub asset URL construction
```

### Pattern 1: Async Download with Channel Progress Updates
**What:** Download runs in spawned tokio task, sends progress via mpsc channel
**When to use:** For long-running downloads that shouldn't block TUI event loop
**Example:**
```rust
// In utils/upgrade.rs
use tokio::sync::mpsc;
use futures_util::StreamExt;

pub enum DownloadProgress {
    Progress { bytes: u64, total: Option<u64> },
    Complete { path: PathBuf },
    Error { message: String },
}

pub fn spawn_download(
    url: String,
    target_path: PathBuf,
) -> mpsc::Receiver<DownloadProgress> {
    let (tx, rx) = mpsc::channel(32);

    tokio::spawn(async move {
        match download_with_progress(&url, &target_path, &tx).await {
            Ok(()) => {
                let _ = tx.send(DownloadProgress::Complete { path: target_path }).await;
            }
            Err(e) => {
                let _ = tx.send(DownloadProgress::Error { message: e.to_string() }).await;
            }
        }
    });

    rx
}

async fn download_with_progress(
    url: &str,
    target_path: &PathBuf,
    tx: &mpsc::Sender<DownloadProgress>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", "to-tui")
        .send()
        .await?;

    let total = response.content_length();
    let mut downloaded: u64 = 0;
    let mut file = tokio::fs::File::create(target_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        downloaded += chunk.len() as u64;

        let _ = tx.send(DownloadProgress::Progress {
            bytes: downloaded,
            total,
        }).await;
    }

    Ok(())
}
```

### Pattern 2: GitHub Release Asset URL Construction
**What:** Construct correct download URL based on platform and release version
**When to use:** When fetching the binary asset from GitHub releases
**Example:**
```rust
// Source: GitHub releases API convention
fn get_asset_download_url(version: &str) -> String {
    let target = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_os = "linux") {
        "x86_64-unknown-linux-gnu"
    } else {
        panic!("Unsupported platform")
    };

    format!(
        "https://github.com/grimurjonsson/to-tui/releases/download/v{}/totui-{}.tar.gz",
        version, target
    )
}
```

### Pattern 3: Atomic Binary Replacement
**What:** Replace running binary without corruption risk
**When to use:** After download completes, before restart
**Example:**
```rust
// Source: self-replace crate docs
use std::path::PathBuf;

fn replace_binary(downloaded_path: &PathBuf) -> anyhow::Result<()> {
    // Extract if archive (tar.gz)
    let temp_dir = tempfile::tempdir()?;
    let extracted = extract_binary(downloaded_path, temp_dir.path())?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&extracted, std::fs::Permissions::from_mode(0o755))?;
    }

    // Atomic replacement using self-replace
    self_replace::self_replace(&extracted)?;

    Ok(())
}
```

### Pattern 4: Process Restart (Unix)
**What:** Re-execute the updated binary
**When to use:** After user confirms restart
**Example:**
```rust
// Source: std::os::unix::process::CommandExt
#[cfg(unix)]
fn restart_self() -> ! {
    use std::os::unix::process::CommandExt;

    let exe = std::env::current_exe().expect("Failed to get current exe");
    let args: Vec<String> = std::env::args().skip(1).collect();

    // exec() replaces current process, doesn't return on success
    let err = std::process::Command::new(exe)
        .args(&args)
        .exec();

    // Only reaches here on error
    panic!("Failed to restart: {}", err);
}

#[cfg(not(unix))]
fn restart_self() -> ! {
    // On Windows, spawn new process and exit
    let exe = std::env::current_exe().expect("Failed to get current exe");
    let args: Vec<String> = std::env::args().skip(1).collect();

    std::process::Command::new(exe)
        .args(&args)
        .spawn()
        .expect("Failed to spawn new process");

    std::process::exit(0);
}
```

### Anti-Patterns to Avoid
- **Blocking the event loop:** Never call `.await` in the main TUI loop; spawn tasks and poll channels
- **Using indicatif in TUI:** indicatif writes directly to stdout, conflicting with ratatui's terminal control
- **Downloading to final location:** Always download to temp file first, then atomic move
- **Ignoring archive format:** GitHub releases are typically tar.gz; must extract before replace

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Binary replacement | Manual fs::rename with cleanup | self_replace::self_replace | Windows has file locking issues; Unix atomic rename is subtle |
| Archive extraction | Manual tar/gzip handling | self_update::Extract | Handles multiple formats, extraction paths |
| Platform detection | Manual cfg checks | self_update target detection | Consistent naming conventions for release assets |
| GitHub API | Manual reqwest to releases/latest | version_check.rs already does this | Project already fetches latest version |

**Key insight:** Binary self-replacement looks trivial on Unix but has significant edge cases on Windows (locked executables, delayed deletion). The self-replace crate handles these with battle-tested code.

## Common Pitfalls

### Pitfall 1: Permission Denied on Binary Replacement
**What goes wrong:** User installed binary in /usr/local/bin (root-owned); can't write
**Why it happens:** Common installation location requires elevated privileges
**How to avoid:**
- Check write permission before attempting download
- Show clear error message with suggested fix (run with sudo or move binary)
- Consider using user-writable install location (~/.local/bin or ~/.cargo/bin)
**Warning signs:** `Permission denied (os error 13)` during replacement

### Pitfall 2: TUI Rendering Conflict with Download
**What goes wrong:** Download progress updates faster than TUI can render
**Why it happens:** Network chunks arrive rapidly; each update triggers render
**How to avoid:**
- Rate-limit progress updates (e.g., max 10/second)
- Update progress state, let normal tick interval handle render
**Warning signs:** UI stuttering, high CPU during download

### Pitfall 3: Incomplete Download on Network Failure
**What goes wrong:** Partial file left on disk; user thinks download succeeded
**Why it happens:** Network interrupted; temp file exists but incomplete
**How to avoid:**
- Download to temp file with random suffix
- Only move to target on full success
- Clean up temp file on error
**Warning signs:** Binary crashes immediately after "successful" upgrade

### Pitfall 4: Wrong Platform Binary Downloaded
**What goes wrong:** Downloaded x86_64 binary on ARM Mac (or vice versa)
**Why it happens:** Hardcoded platform string or missing arch detection
**How to avoid:**
- Use `cfg!(target_arch)` for architecture
- Verify binary runs before replacing (optional but safer)
**Warning signs:** "Exec format error" or "Bad CPU type in executable"

### Pitfall 5: Blocking Async Runtime
**What goes wrong:** TUI freezes during download
**Why it happens:** Mixing blocking I/O with async, or awaiting in event loop
**How to avoid:**
- Spawn download as separate tokio task
- Poll mpsc channel in non-blocking tick
- Use try_recv() not recv() in TUI loop
**Warning signs:** Application becomes unresponsive during download

## Code Examples

### TUI Progress Bar Rendering
```rust
// Source: ratatui Gauge widget docs
use ratatui::widgets::{Block, Borders, Gauge};
use ratatui::style::{Color, Style};

fn render_download_progress(f: &mut Frame, area: Rect, progress: f64, bytes: u64, total: Option<u64>) {
    let percent = (progress * 100.0) as u16;

    let label = match total {
        Some(t) => format!("{} / {} ({percent}%)",
            format_bytes(bytes), format_bytes(t)),
        None => format!("{} downloaded", format_bytes(bytes)),
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Downloading"))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .percent(percent)
        .label(label);

    f.render_widget(gauge, area);
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}
```

### Polling Download Progress in Event Loop
```rust
// Integration with existing event loop pattern
impl AppState {
    pub fn check_download_progress(&mut self) {
        if let Some(ref rx) = self.download_progress_rx {
            // Non-blocking poll
            match rx.try_recv() {
                Ok(DownloadProgress::Progress { bytes, total }) => {
                    if let Some(ref mut sub) = self.upgrade_sub_state {
                        if let UpgradeSubState::Downloading {
                            ref mut progress,
                            ref mut bytes_downloaded,
                            ref mut total_bytes,
                        } = sub {
                            *bytes_downloaded = bytes;
                            *total_bytes = total;
                            *progress = total.map(|t| bytes as f64 / t as f64).unwrap_or(0.0);
                        }
                    }
                }
                Ok(DownloadProgress::Complete { path }) => {
                    self.download_progress_rx = None;
                    self.upgrade_sub_state = Some(UpgradeSubState::RestartPrompt {
                        downloaded_path: path,
                    });
                }
                Ok(DownloadProgress::Error { message }) => {
                    self.download_progress_rx = None;
                    self.upgrade_sub_state = Some(UpgradeSubState::Error { message });
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No update yet, continue
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.download_progress_rx = None;
                    self.upgrade_sub_state = Some(UpgradeSubState::Error {
                        message: "Download task crashed".to_string(),
                    });
                }
            }
        }
    }
}
```

### Binary Extraction from tar.gz
```rust
// Source: self_update::Extract documentation
use self_update::{Extract, ArchiveKind, Compression};
use std::path::Path;

fn extract_binary(archive_path: &Path, target_dir: &Path) -> anyhow::Result<PathBuf> {
    let bin_name = "totui"; // Binary name without extension

    Extract::from_source(archive_path)
        .archive(ArchiveKind::Tar(Some(Compression::Gz)))
        .extract_file(target_dir, bin_name)?;

    Ok(target_dir.join(bin_name))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Shell scripts for update | Rust-native self_update | 2020+ | No external dependencies |
| indicatif for all progress | TUI-integrated progress | Always for TUI apps | Proper terminal control |
| Separate updater binary | In-process self-replace | self-replace 1.0+ | Simpler deployment |

**Deprecated/outdated:**
- `self_update` versions < 0.27: Older reqwest, missing rustls option
- Manual `execvp` via libc: Use std::os::unix::process::CommandExt instead

## Open Questions

Things that couldn't be fully resolved:

1. **Release Asset Naming Convention**
   - What we know: GitHub releases need platform-specific binaries
   - What's unclear: Exact naming convention used in grimurjonsson/to-tui releases
   - Recommendation: Check existing releases; may need to establish convention like `totui-{version}-{target}.tar.gz`

2. **Graceful Degradation on Windows**
   - What we know: Windows requires different restart logic (spawn + exit vs exec)
   - What's unclear: Project's Windows support priority
   - Recommendation: Implement cross-platform code but focus testing on macOS/Linux

3. **Signature Verification**
   - What we know: self_update supports `signatures` feature using zipsign
   - What's unclear: Whether release signing is currently done
   - Recommendation: Defer to future phase; implement basic update first

## Sources

### Primary (HIGH confidence)
- [self_update crate docs](https://docs.rs/self_update) - API documentation, builder pattern
- [self-replace crate docs](https://docs.rs/self-replace/latest/self_replace/) - Binary replacement mechanics
- [ratatui Gauge widget](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Gauge.html) - Progress rendering
- [std::os::unix::process::CommandExt](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html) - exec for restart

### Secondary (MEDIUM confidence)
- [GitHub jaemk/self_update](https://github.com/jaemk/self_update) - Full README with examples
- [Rust reqwest streaming downloads gist](https://gist.github.com/Tapanhaz/096e299bf060607b572d700e89a62529) - bytes_stream pattern
- [tokio channels tutorial](https://tokio.rs/tokio/tutorial/channels) - mpsc for progress updates

### Tertiary (LOW confidence)
- WebSearch results for permission handling - needs project-specific validation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - self_update is well-documented, widely used
- Architecture: HIGH - follows existing plugin/rollover patterns in codebase
- Pitfalls: MEDIUM - based on common patterns, some project-specific verification needed

**Research date:** 2026-01-21
**Valid until:** 2026-02-21 (30 days - stable libraries, mature patterns)
