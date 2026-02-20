# Changelog

All notable changes to to-tui will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.5] - 2026-02-20
Plugin detection and updates now visible in the TUI, todo descriptions expand properly, clipboard works in headless environments, and undo no longer causes save conflicts.

### Added
- preparing 0.5.4 (#19)
- detecting plugins and plugin updates in tui (#18)
- comprehensive debug logging for TUI storage and MCP

### Fixed
- expanding todos with huge descriptions now actually expand and show the expanded description
- fixing justfile indents (#21)
- add clipboard fallback for headless environments (#12)
- use UPSERT for save to handle undo gracefully

## [0.5.4] - 2026-02-03
Added plugin detection with update notifications in the TUI. Fixed clipboard operations for headless environments. Improved undo reliability with database saves. Added comprehensive debug logging for troubleshooting.

### Added
- detecting plugins and plugin updates in tui (#18)
- comprehensive debug logging for TUI storage and MCP

### Fixed
- add clipboard fallback for headless environments (#12)
- use UPSERT for save to handle undo gracefully

## [0.5.3] - 2026-01-28
Upgraded to interface v0.3.0 and improved status bar color contrast for better readability.

### Added
- bumping interface to 0.3.0

### Fixed
- quick-001): improve status bar color contrast

## [0.5.2] - 2026-01-28
Bumped plugin interface to 0.3.0 and improved status bar color contrast for better readability.

### Added
- bumping interface to 0.3.0

### Fixed
- quick-001): improve status bar color contrast

## [0.5.1] - 2026-01-27
## [0.5.0] - 2026-01-27
The v2.0 release introduces a plugin framework that allows external sources (like Jira) to generate todos, along with project support for organizing todos into separate workspaces.

### Added
- v2.0 Plugin Framework

## [0.4.0] - 2026-01-23
Added project support, priority sorting, and UI improvements. Fixed Windows zip issues and minor install script adjustments.

### Added
- add project support, priority sorting, and UI improvements

### Fixed
- windows zip issues

## [0.3.4] - 2026-01-22
Use tarballs instead of raw binaries in releases for easier installation.

## [0.3.3] - 2026-01-22
Cleaned up README documentation.

## [0.3.2] - 2026-01-21
Added automatic self-upgrade feature with in-app download progress UI, binary replacement, and automatic restart. Includes crash handler with log file support.

### Added
- 05-03): add crash handler with log file
- 05-03): wire up restart in event handler
- 05-03): add binary extraction and restart functions
- 05-03): add upgrade UI rendering for all sub-states
- 05-02): add download progress polling to event loop
- 05-02): update event handling for upgrade mode sub-states
- 05-02): add upgrade sub-state tracking to AppState
- 05-01): add upgrade module with download infrastructure
- quick-002): integration and post-quit URL printing
- quick-002): add upgrade modal rendering and event handling
- quick-002): add UpgradePrompt mode and state management

### Fixed
- 05-03): restore terminal before exec() in upgrade restart
- 05-03): use std::thread instead of tokio for download
- 05-03): download raw binaries instead of non-existent tar.gz archives

### Changed
- 05-03): use self_update's re-exported self_replace

## [0.3.1] - 2026-01-21
A version checker now notifies you when a new release is available.

### Added
- adding a version checker to notify of new releases

## [0.3.0] - 2026-01-21
MCP plugin now finds the binary via PATH instead of hardcoded paths, improving portability across different installations.

### Fixed
- use PATH-based binary lookup for MCP plugin

## [0.2.12] - 2026-01-20
Fixes plugin marketplace configuration.

### Fixed
- Fix the plugin marketplace config

## [0.2.11] - 2026-01-20
Improved changelog display with better section spacing and added automatic TL;DR generation using Claude CLI.

### Added
- add generate-changelog-test command
- generate TL;DR for changelog using Claude CLI

### Fixed
- relax TL;DR prompt constraints
- remove blank line between version header and first section
- preserve blank lines in changelog display
- add blank lines between changelog sections

## [0.2.10] - 2026-01-20
### Added
- auto-update CHANGELOG.md during release

### Fixed
- fetch changelog from main branch instead of tag

## [0.2.9] - 2026-01-20
### Added
- Install script now shows changelog when upgrading

## [0.2.8] - 2026-01-19
### Added
- Todo priority system (P0/P1/P2) with `p` key to cycle priorities
- Priority badges displayed with colored indicators

### Fixed
- Install script now handles upgrades from v0.1.x properly

## [0.2.7] - 2026-01-18
### Improved
- Help window redesigned with better organization and readability

## [0.2.6] - 2026-01-17
### Fixed
- Internal state caching so startup reloads previous state correctly
- Top item is now selected on startup

## [0.2.5] - 2026-01-17
### Fixed
- Scrollbar status display
- Background color and status cycling behavior

## [0.2.4] - 2026-01-17
### Added
- Clipboard support with `y` key to yank todo text
- Mouse wheel scrolling (3 items per scroll)
- Click-to-select items
- Scroll position indicator in title bar

## [0.2.3] - 2026-01-16
### Added
- Clipboard foundation with arboard integration
- Yank action with vim-style `y` keybinding
- Status bar shows "Copied!" feedback

## [0.2.2] - 2026-01-15
### Improved
- Installer script improvements and bug fixes
- Internal refactoring

## [0.2.1] - 2026-01-14
### Added
- Install script for easy binary installation
- Push-to-remote prompt after release commands

## [0.2.0] - 2026-01-13
### Changed
- Project renamed and restructured
- GitHub Actions workflow for automated releases
- Pre-built binaries for all major platforms

### Added
- Marketplace support for Claude Code plugin
