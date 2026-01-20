# Changelog

All notable changes to to-tui will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
