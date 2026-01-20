# Quick Task Summary: Install Script Version Changelog

## Completed

1. **Created CHANGELOG.md** - Standard Keep a Changelog format
   - Documents all versions from 0.2.0 to 0.2.8
   - Grouped by Added/Changed/Fixed/Improved sections
   - Includes dates for each release

2. **Updated scripts/install.sh** - Added changelog display on upgrades
   - `show_changelog()` function fetches CHANGELOG.md from GitHub
   - `version_gt()` helper uses `sort -V` for semantic version comparison
   - Parses and displays only entries between user's old version and new version
   - Shows formatted output with colors for headers

3. **Tested** - Verified all logic works correctly
   - Syntax check passed
   - Version comparison: 0.2.8 > 0.2.5, 0.2.8 > 0.1.17
   - Changelog parsing correctly extracts entries between versions

## Files Changed

- `CHANGELOG.md` (new) - Version history
- `scripts/install.sh` (modified) - Added ~50 lines for changelog display

## Behavior

When users upgrade from an older version:
```
Installation complete!

What's new since v0.2.5:

## [0.2.8] - 2026-01-19
### Added
  - Todo priority system (P0/P1/P2) with `p` key to cycle priorities
...
```
