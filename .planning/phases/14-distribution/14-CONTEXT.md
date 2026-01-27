# Phase 14: Distribution - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Install plugins from local directories or GitHub repositories. CLI commands for install and list. Support for multiple marketplaces (default + third-party). Binaries served via GitHub Releases.

</domain>

<decisions>
## Implementation Decisions

### Install Workflow
- Pre-built binaries downloaded from GitHub Releases (no build from source)
- Latest version by default; `--version` flag for specific version
- Local installs copy files to plugins directory (no symlinks)
- Plugins enabled by default after install (matches existing behavior)

### Registry/Marketplace Structure
- Directory-per-plugin layout in marketplace repos (plugins/foo/, plugins/bar/)
- Each plugin directory contains manifest.toml + README
- Binaries live in GitHub Releases (not committed to repo)
- Third-party marketplaces supported via marketplace.toml manifest
- Marketplace manifest contains plugin list with basic info (name, description, latest version)
- Default registry (grimurjonsson/to-tui-plugins) is configurable, not hardcoded
- Marketplaces managed via config.toml `[marketplaces]` section and CLI commands
- Install requires explicit path: `plugin install grimurjonsson/to-tui-plugins/jira` (no short names)

### Plugin List Display
- `totui plugin list` shows installed plugins only
- Columns: name, version, status, source (marketplace)
- Status indicators: enabled, disabled, error, incompatible
- Update checking via optional `--check-updates` flag (not default)

### Error Handling
- Missing platform binary: fail with clear message listing available platforms
- Network failures: fail immediately (no automatic retry)
- Version incompatibility: refuse to install if plugin requires newer to-tui
- Detailed progress output: step-by-step (Downloading... Verifying... Installing... Done)

### Claude's Discretion
- Exact marketplace.toml schema
- Platform detection logic
- Binary naming conventions in releases
- Cache strategy for marketplace manifests

</decisions>

<specifics>
## Specific Ideas

- Third-party marketplaces work like the official one — any GitHub repo with marketplace.toml becomes a plugin source
- `totui marketplace add user/repo` command edits config file
- Install path format: `owner/repo/plugin-name` for clarity and uniqueness

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 14-distribution*
*Context gathered: 2026-01-26*
