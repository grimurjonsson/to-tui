# Quick Task Plan: Install Script Version Changelog

**Task:** Update the install script to show changes since the user's last installed version

## Tasks

1. **Create CHANGELOG.md** - Standard changelog file with Keep a Changelog format
2. **Update install.sh** - Add changelog display logic after successful installation
3. **Test parsing logic** - Verify version comparison and changelog extraction

## Approach

- Follow Keep a Changelog convention (standard practice)
- Fetch CHANGELOG.md from GitHub raw content during install
- Parse and display only entries between old and new versions
- Use `sort -V` for semantic version comparison
