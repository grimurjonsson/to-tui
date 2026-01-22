---
phase: quick
plan: 004
type: summary
subsystem: release-infrastructure
tags:
  - github-actions
  - installation
  - self-upgrade
  - tar-gz
  - archive-format

requires:
  - "GitHub release workflow"
  - "Installation scripts"
  - "TUI self-upgrade module"

provides:
  - "tar.gz release archives for Unix platforms"
  - "zip release archives for Windows platforms"
  - "Archive extraction in all installation methods"

affects:
  - "future releases (all binaries now distributed as archives)"

tech-stack:
  added:
    - "flate2 (gzip decompression)"
    - "tar (tar archive extraction)"
  patterns:
    - "archive-based distribution"
    - "temp directory extraction pattern"

key-files:
  created:
    - ".planning/quick/004-tar-gz-release-binaries/004-SUMMARY.md"
  modified:
    - ".github/workflows/release.yml"
    - "scripts/install.sh"
    - "scripts/install-binary.sh"
    - "src/utils/upgrade.rs"
    - "Cargo.toml"

decisions:
  - id: "archive-format-by-platform"
    what: "Use .tar.gz for Unix, .zip for Windows"
    why: "Platform conventions - Unix uses tar.gz, Windows uses zip"
    alternatives:
      - "Use .tar.gz for all platforms (rejected: zip is Windows standard)"
      - "Use .zip for all platforms (rejected: tar.gz is Unix standard)"

  - id: "binary-only-archives"
    what: "Archives contain only the binary, no additional files"
    why: "Simplifies extraction logic, minimal archive size"
    alternatives:
      - "Include README/LICENSE in archives (rejected: adds complexity, rarely used)"

  - id: "temp-directory-extraction"
    what: "Extract archives to temp directories before moving binary"
    why: "Clean separation of download/extract/install phases, better error handling"
    alternatives:
      - "Extract directly to install directory (rejected: partial extraction could leave broken state)"

metrics:
  duration: "3 minutes"
  completed: "2026-01-22"
---

# Quick Task 004: tar.gz Release Binaries Summary

**One-liner:** Changed release format from raw binaries to tar.gz/zip archives across CI, install scripts, and TUI self-upgrade.

## What Was Done

Migrated the entire release and installation pipeline from raw executables to archive-based distribution:

### Release Workflow Changes
- **Unix platforms:** Create `.tar.gz` archives using `tar -czvf`
- **Windows platform:** Create `.zip` archives using `zip`
- Archive naming: `{binary_name}-{target}.tar.gz` (e.g., `totui-x86_64-apple-darwin.tar.gz`)
- Archives contain only the binary (no subdirectories, README, or LICENSE)

### Installation Script Updates (scripts/install.sh)
- Download `.tar.gz` (Unix) or `.zip` (Windows) archives instead of raw binaries
- Extract to temporary directory using `tar -xzf` or `unzip`
- Validate extracted binary exists before moving to install directory
- Clean up temp directory after installation
- Improved error handling for extraction failures

### MCP Plugin Install Script (scripts/install-binary.sh)
- Same archive extraction pattern as main install script
- Downloads to temp directory, extracts, validates, moves binary
- Clean error messages for extraction failures
- Proper cleanup on all error paths

### TUI Self-Upgrade Module (src/utils/upgrade.rs)
- Updated `get_asset_download_url()` to append `.tar.gz` extension
- Rewrote `prepare_binary()` to extract tar.gz archives:
  - Uses `flate2::read::GzDecoder` for gzip decompression
  - Uses `tar::Archive` for tar extraction
  - Extracts to temp directory adjacent to download
  - Validates binary exists and has reasonable size (>1MB)
  - Sets executable permissions on extracted binary
- Added `flate2` and `tar` dependencies to Cargo.toml
- Updated tests to expect `.tar.gz` URLs

## Technical Details

### Dependency Additions
```toml
flate2 = "1.0"
tar = "0.4"
```

These crates were already transitive dependencies through `self_update` but needed to be added explicitly for direct use.

### Archive Extraction Pattern
All extraction logic follows the same pattern:
1. Download archive to temp file/directory
2. Extract using platform-appropriate tool
3. Validate binary exists in extraction directory
4. Move binary to final installation location
5. Set executable permissions (Unix)
6. Clean up temp directory

This pattern ensures clean error handling and no partial installations.

### Archive Contents
Archives are minimal - they contain only the binary file:
- `totui-{target}.tar.gz` → contains `totui` binary
- `totui-mcp-{target}.tar.gz` → contains `totui-mcp` binary
- Windows: `.zip` equivalent with `.exe` binaries

No subdirectories, README, LICENSE, or other files. This keeps extraction logic simple.

## Testing

- ✅ All unit tests pass (`cargo test`)
- ✅ Upgrade module tests updated and passing
- ✅ URL format test expects `.tar.gz` extension
- ✅ Code compiles without errors
- ✅ No new clippy warnings in changed files

## Deviations from Plan

None - plan executed exactly as written.

## Next Steps

### For Next Release
When the next release is created (v0.3.4 or later), the GitHub workflow will:
1. Build binaries for all targets
2. Create tar.gz/zip archives
3. Upload archives to GitHub Releases
4. Users can install via:
   - `curl -sSL https://to-tui.dev | bash` (uses install.sh)
   - TUI self-upgrade (uses upgrade.rs)
   - Claude Code MCP plugin (uses install-binary.sh)

All installation methods will automatically download and extract the new archive format.

### Verification Needed
After next release:
- [ ] Verify archives are created correctly in CI
- [ ] Test install.sh with real release archives
- [ ] Test TUI self-upgrade with real release
- [ ] Test MCP plugin installation

## Impact

**Breaking changes:** None for end users. Installation methods remain the same, only the underlying format changed.

**Benefits:**
- Follows Rust ecosystem conventions (most Rust tools distribute as tar.gz)
- Allows future inclusion of additional files in archives (README, LICENSE)
- More robust installation (extraction validation before moving files)
- Better error messages during installation

**Files changed:** 5
**Lines added/removed:** +146/-32
**Commits:** 4

## Commits

| Commit | Message | Files |
|--------|---------|-------|
| 73fda28 | feat(quick-004): create tar.gz archives in release workflow | .github/workflows/release.yml |
| 420cfa4 | feat(quick-004): update install.sh to extract tar.gz archives | scripts/install.sh |
| b1776a8 | feat(quick-004): update install-binary.sh to extract tar.gz | scripts/install-binary.sh |
| 6ce5333 | feat(quick-004): extract tar.gz archives in TUI self-upgrade | Cargo.toml, src/utils/upgrade.rs |
