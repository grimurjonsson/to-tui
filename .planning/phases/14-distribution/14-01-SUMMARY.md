---
phase: 14-distribution
plan: 01
subsystem: plugin-distribution
tags: [installation, cli, local-install]

dependency_graph:
  requires: []
  provides: [local-plugin-install, plugin-source-parsing, install-cli]
  affects: [14-02]

tech_stack:
  added: []
  patterns: [directory-copy-install, source-parsing]

files:
  created:
    - src/plugin/installer.rs
  modified:
    - src/plugin/mod.rs
    - src/plugin/manager.rs
    - src/cli.rs
    - src/main.rs

decisions:
  - id: local-copy-install
    choice: "Copy directory contents instead of symlinks"
    reason: "User decision - cross-platform reliability, avoids Windows admin requirements"
  - id: source-parsing
    choice: "Detect local paths by prefix (/, ./, ../, ~) or filesystem existence"
    reason: "Intuitive UX - users can use standard path formats"
  - id: force-flag
    choice: "Require --force flag to overwrite existing installations"
    reason: "Prevents accidental data loss from config or customizations"

metrics:
  duration: 3min
  completed: 2026-01-26
---

# Phase 14 Plan 01: Local Plugin Installation Summary

Local plugin installation with directory copy and source path parsing for development and manual installs.

## What Was Built

### PluginInstaller Module (`src/plugin/installer.rs`)
New module providing:

1. **PluginSource struct** - Parses install sources to detect local vs remote:
   - Local paths detected by: `/`, `./`, `../`, `~` prefixes or filesystem existence
   - Remote format: `owner/repo/plugin-name` (parsed but not yet implemented)
   - `is_local()` method for routing install flow

2. **InstallResult struct** - Return type with:
   - `plugin_name`: Name from manifest
   - `version`: Version string
   - `path`: Target installation path

3. **PluginInstaller::install_from_local()** - Core installation logic:
   - Validates source directory contains plugin.toml
   - Loads and validates manifest using `PluginManager::load_plugin_info`
   - Checks min_interface_version compatibility
   - Detects existing installation (errors unless `--force`)
   - Recursively copies directory contents to plugins folder

4. **copy_dir_recursive helper** - Copies files and subdirectories

### CLI Command Extension
- Added `PluginCommand::Install` variant with `source`, `--version`, `--force` args
- Local paths install immediately; remote paths error with "not yet implemented"
- Success message shows plugin name, version, and installed path

### API Changes
- Made `PluginManager::load_plugin_info` public for installer validation

## Key Implementation Details

### Path Detection Logic
```rust
// Local paths detected by prefix
source.starts_with('/') || source.starts_with("./") ||
source.starts_with("../") || source.starts_with('~')

// Or filesystem existence check
Path::new(source).exists() && path.is_dir()

// Otherwise parse as remote format
"owner/repo/plugin-name"
```

### Install Flow
1. Parse source string into PluginSource
2. Expand ~ paths using `dirs::home_dir()`
3. Canonicalize path to absolute
4. Validate manifest exists and parses
5. Check version compatibility
6. Check for existing installation
7. Remove existing if --force
8. Copy directory recursively

## Deviations from Plan

None - plan executed exactly as written.

## Commit History

| Commit | Type | Description |
|--------|------|-------------|
| f40582c | feat | Add PluginInstaller module with local installation |
| 8e75aac | feat | Wire CLI plugin install command for local paths |

## Files Changed

| File | Change |
|------|--------|
| src/plugin/installer.rs | Created - PluginInstaller, PluginSource, InstallResult |
| src/plugin/mod.rs | Added installer module export |
| src/plugin/manager.rs | Made load_plugin_info public |
| src/cli.rs | Added Install variant to PluginCommand |
| src/main.rs | Added install command handling |

## Test Coverage

6 unit tests added:
- `test_plugin_source_parse_local_absolute` - Absolute path parsing
- `test_plugin_source_parse_remote_format` - Remote format parsing
- `test_plugin_source_parse_invalid` - Invalid format handling
- `test_install_missing_manifest` - Missing plugin.toml error
- `test_install_invalid_manifest` - Invalid TOML error
- `test_copy_dir_recursive` - Directory copy with subdirs

Manual testing verified:
- Install from local directory works
- Plugin appears in `plugin list`
- Duplicate install without --force fails with helpful message
- Duplicate install with --force succeeds

## Next Phase Readiness

Ready for 14-02 (Remote Plugin Installation):
- PluginSource already parses remote format (owner/repo/plugin-name)
- Install infrastructure in place
- Just needs HTTP download and tar.gz extraction (patterns exist in upgrade.rs)
