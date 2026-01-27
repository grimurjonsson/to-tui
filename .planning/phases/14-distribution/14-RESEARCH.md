# Phase 14: Distribution - Research

**Researched:** 2026-01-26
**Domain:** Plugin installation and distribution (local, GitHub, registry)
**Confidence:** HIGH

## Summary

This phase implements a plugin distribution system allowing users to install plugins from local directories or GitHub repositories. The system leverages existing codebase patterns extensively: the upgrade module already demonstrates HTTP downloads with progress, platform detection, and tar.gz extraction. The plugin manager already handles discovery and manifest parsing.

The implementation requires:
1. A `PluginInstaller` module for local and remote installation workflows
2. A `marketplace.toml` manifest format for registry metadata
3. CLI commands (`install`, `list` with extended info)
4. Config extensions for marketplace management

**Primary recommendation:** Extend the existing `reqwest::blocking` download pattern from `utils/upgrade.rs` for GitHub release downloads. Use direct GitHub release URLs (no GitHub API required for public repos) with platform-specific binary naming conventions.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest (blocking) | 0.12 | HTTP downloads | Already in Cargo.toml, proven pattern in upgrade.rs |
| flate2 | 1.0 | Gzip decompression | Already in Cargo.toml for upgrade extraction |
| tar | 0.4 | Tar archive handling | Already in Cargo.toml for upgrade extraction |
| toml | 0.9 | Manifest parsing | Already in Cargo.toml, used throughout codebase |
| semver | 1.0 | Version compatibility | Already in Cargo.toml, used in plugin interface |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs | 6.0 | Cross-platform paths | Already used in `utils/paths.rs` |
| tempfile | 3.13 | Temporary download staging | Already in Cargo.toml |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Direct HTTP | octocrab (GitHub API client) | Adds dependency; GitHub API requires auth for higher rate limits; direct URLs work for public repos |
| Blocking reqwest | Async reqwest | TUI runs without tokio; upgrade.rs already uses blocking client in spawned threads |

**Installation:**
```bash
# Already in Cargo.toml - no new dependencies needed
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── plugin/
│   ├── installer.rs     # NEW: PluginInstaller, local/remote install
│   ├── marketplace.rs   # NEW: Marketplace manifest parsing
│   ├── manager.rs       # EXISTING: Discovery, extend for source tracking
│   ├── manifest.rs      # EXISTING: Plugin manifest parsing
│   └── mod.rs           # Export new modules
├── cli.rs               # EXTEND: Add Install command
├── config.rs            # EXTEND: Add marketplaces config
└── utils/
    └── paths.rs         # EXTEND: Add marketplace cache paths
```

### Pattern 1: Direct GitHub Release URLs (No API Required)
**What:** Download plugin binaries directly from GitHub release URLs without API authentication
**When to use:** Always for public repositories (avoids rate limits, no token needed)
**Example:**
```rust
// Source: Existing pattern in utils/upgrade.rs
// GitHub release asset URL format:
// https://github.com/{owner}/{repo}/releases/download/{tag}/{asset-name}
fn get_plugin_download_url(owner: &str, repo: &str, plugin: &str, version: &str) -> String {
    let target = get_target_triple();
    format!(
        "https://github.com/{}/{}/releases/download/v{}/{}-{}.tar.gz",
        owner, repo, version, plugin, target
    )
}
```

### Pattern 2: Local Install via Directory Copy
**What:** Copy entire plugin directory to plugins folder, preserving structure
**When to use:** Local development, manual installs from directory path
**Example:**
```rust
// Source: Standard Rust fs operations
use std::fs;

fn install_from_local(source_dir: &Path, plugin_name: &str) -> Result<PathBuf> {
    let plugins_dir = get_plugins_dir()?;
    let target_dir = plugins_dir.join(plugin_name);

    // Copy entire directory (manifest + binaries)
    copy_dir_recursive(source_dir, &target_dir)?;

    Ok(target_dir)
}
```

### Pattern 3: Staged Installation with Validation
**What:** Download to temp, validate manifest, then move to plugins directory
**When to use:** All remote installations
**Example:**
```rust
// Download to temp, validate, then install
fn install_from_remote(url: &str, plugin_name: &str) -> Result<PathBuf> {
    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join("plugin.tar.gz");

    // 1. Download archive
    download_file_blocking(&url, &archive_path)?;

    // 2. Extract to temp
    let extracted = extract_tar_gz(&archive_path, temp_dir.path())?;

    // 3. Validate manifest exists and parses
    let manifest_path = extracted.join("plugin.toml");
    let manifest: PluginManifest = load_and_validate_manifest(&manifest_path)?;

    // 4. Check version compatibility
    check_version_compatibility(&manifest)?;

    // 5. Move to plugins directory
    let target = get_plugins_dir()?.join(plugin_name);
    fs::rename(&extracted, &target)?;

    Ok(target)
}
```

### Anti-Patterns to Avoid
- **Using symlinks for local installs:** User decision was explicit copy, not symlinks. Symlinks have cross-platform issues (Windows requires admin or developer mode).
- **Building from source:** User decision was pre-built binaries only. Building requires Rust toolchain on user's machine.
- **Caching marketplace manifests permanently:** Network failures should fail fast; stale cache leads to confusion.
- **Short plugin names:** User decision requires explicit paths like `owner/repo/plugin-name` for clarity.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP downloads | Custom HTTP client | `reqwest::blocking` | Already proven in upgrade.rs |
| Tar extraction | Manual tar parsing | `flate2` + `tar` | Already used in upgrade.rs |
| Platform detection | Runtime OS detection | `cfg` compile-time | Existing pattern in upgrade.rs |
| Version comparison | String comparison | `semver` crate | Handles pre-release, metadata correctly |
| Progress display | Custom progress | Existing `DownloadProgress` enum | Reuse upgrade.rs channel pattern |

**Key insight:** The upgrade module already solves 80% of the distribution problem. Reuse `spawn_download`, `download_file_blocking`, `prepare_binary` (adapted), and the progress channel pattern.

## Common Pitfalls

### Pitfall 1: Platform Binary Not Found
**What goes wrong:** User requests install but no binary exists for their platform
**Why it happens:** Plugin author didn't build for all platforms
**How to avoid:** Fail with clear message listing available platforms from the release
**Warning signs:** 404 response when downloading binary asset

### Pitfall 2: Network Failure During Download
**What goes wrong:** Partial download leaves corrupted file
**Why it happens:** Network instability, timeouts
**How to avoid:** Always download to temp directory first; only move on complete success
**Warning signs:** Small file size, extraction failure

### Pitfall 3: Version Incompatibility Silent Install
**What goes wrong:** Plugin installs but immediately fails to load
**Why it happens:** Plugin requires newer to-tui version
**How to avoid:** Check `min_interface_version` BEFORE moving to plugins directory
**Warning signs:** Plugin appears in `plugin list` but shows "incompatible" status

### Pitfall 4: Duplicate Plugin Installation
**What goes wrong:** Installing same plugin twice overwrites without warning
**Why it happens:** No check for existing installation
**How to avoid:** Check for existing plugin, require `--force` flag to overwrite
**Warning signs:** User installs, previous config lost

### Pitfall 5: Marketplace Manifest Out of Sync
**What goes wrong:** Marketplace lists plugin version that doesn't exist in releases
**Why it happens:** Manifest updated before release created (or release deleted)
**How to avoid:** Verify release URL returns 200 before showing as available
**Warning signs:** Install commands fail with 404

## Code Examples

Verified patterns from official sources and existing codebase:

### Platform Detection (from upgrade.rs)
```rust
// Source: /Users/gimmi/Documents/Sources/rust/to-tui/src/utils/upgrade.rs
fn get_target_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    { "aarch64-apple-darwin" }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    { "x86_64-apple-darwin" }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    { "x86_64-unknown-linux-gnu" }
    // ... fallback with helpful error
}
```

### Blocking Download with Progress (from upgrade.rs)
```rust
// Source: /Users/gimmi/Documents/Sources/rust/to-tui/src/utils/upgrade.rs
fn download_file_blocking(
    url: &str,
    target_path: &PathBuf,
    tx: &mpsc::Sender<DownloadProgress>,
) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("to-tui")
        .build()?;

    let response = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()?;

    // ... streaming with progress updates
}
```

### Tar.gz Extraction (from upgrade.rs)
```rust
// Source: /Users/gimmi/Documents/Sources/rust/to-tui/src/utils/upgrade.rs
use flate2::read::GzDecoder;
use tar::Archive;

let tar_gz = std::fs::File::open(archive_path)?;
let tar = GzDecoder::new(tar_gz);
let mut archive = Archive::new(tar);
archive.unpack(&extract_dir)?;
```

### Marketplace Manifest Schema (Claude's Discretion)
```toml
# marketplace.toml - Root of marketplace repository
[marketplace]
name = "to-tui-plugins"
description = "Official plugin registry for to-tui"
url = "https://github.com/grimurjonsson/to-tui-plugins"

[[plugins]]
name = "jira"
description = "Fetch Jira tickets as todos"
version = "1.0.0"
repository = "https://github.com/grimurjonsson/to-tui-plugins"
# Binary naming: {plugin}-{target}.tar.gz in GitHub Releases
```

### Plugin Install Path Format
```rust
// User decision: explicit path format owner/repo/plugin-name
// Example: grimurjonsson/to-tui-plugins/jira

struct PluginSource {
    owner: String,       // "grimurjonsson"
    repo: String,        // "to-tui-plugins"
    plugin_name: String, // "jira"
    version: Option<String>, // None = latest
}

impl PluginSource {
    fn parse(source: &str) -> Result<Self> {
        let parts: Vec<&str> = source.split('/').collect();
        if parts.len() != 3 {
            anyhow::bail!(
                "Invalid plugin source format. Expected: owner/repo/plugin-name\n\
                 Example: grimurjonsson/to-tui-plugins/jira"
            );
        }
        Ok(Self {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
            plugin_name: parts[2].to_string(),
            version: None,
        })
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single registry | Multiple marketplaces | This phase | Third-party plugins supported |
| Build from source | Pre-built binaries | User decision | No Rust toolchain needed |
| Symlinks | Directory copy | User decision | Cross-platform reliability |

**Deprecated/outdated:**
- None - this is a new feature

## Open Questions

Things that couldn't be fully resolved:

1. **Binary naming convention in releases**
   - What we know: Need `{plugin}-{target}.tar.gz` format
   - What's unclear: Should version be in filename? (e.g., `jira-1.0.0-aarch64-apple-darwin.tar.gz`)
   - Recommendation: Use version in release tag only, not filename (simpler, matches upgrade.rs pattern)

2. **Cache strategy for marketplace manifests**
   - What we know: User decision says no auto-retry on network failure
   - What's unclear: Should we cache at all? For how long?
   - Recommendation: No caching initially. Fetch fresh each time. Consider optional `--offline` mode later if needed.

3. **Windows platform support**
   - What we know: get_target_triple() doesn't support Windows yet
   - What's unclear: Windows binary naming (.dll vs .exe) and distribution
   - Recommendation: Add Windows target triples when implementing, use `.dll` for plugin libraries

## Sources

### Primary (HIGH confidence)
- Existing codebase: `/Users/gimmi/Documents/Sources/rust/to-tui/src/utils/upgrade.rs` - Download, extraction, platform detection patterns
- Existing codebase: `/Users/gimmi/Documents/Sources/rust/to-tui/src/plugin/manager.rs` - Plugin discovery pattern
- [GitHub API Release Assets](https://docs.github.com/en/rest/releases/assets) - Unauthenticated access for public repos
- [The Cargo Book - Manifest Format](https://doc.rust-lang.org/cargo/reference/manifest.html) - TOML manifest patterns
- [Rust Cookbook - Downloads](https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html) - reqwest download patterns

### Secondary (MEDIUM confidence)
- [platforms crate](https://crates.io/crates/platforms) - Target triple information
- [Packaging and distributing a Rust tool](https://rust-cli.github.io/book/tutorial/packaging.html) - Binary distribution patterns

### Tertiary (LOW confidence)
- None - all findings verified against existing codebase or official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in Cargo.toml with proven patterns
- Architecture: HIGH - Extends existing patterns from upgrade.rs and plugin manager
- Pitfalls: HIGH - Derived from user decisions and existing error handling patterns

**Research date:** 2026-01-26
**Valid until:** 2026-02-26 (stable codebase, no fast-moving dependencies)
