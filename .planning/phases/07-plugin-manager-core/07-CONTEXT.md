# Phase 7: Plugin Manager Core - Context

**Gathered:** 2026-01-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Plugin discovery, manifest parsing, registration, and lifecycle management. Plugins can be discovered, registered, enabled/disabled, and their status queried — but no actual dynamic loading yet (that's Phase 8).

</domain>

<decisions>
## Implementation Decisions

### Manifest format
- Filename: `plugin.toml`
- Full metadata required: name, version, description, author, license, homepage, repository
- Implicit versioning: no manifest_version field, parse what we can, ignore unknown fields
- No permissions system yet — all plugins get same access (permissions deferred to later)

### Discovery behavior
- Startup only: scan plugin directory once when to-tui launches
- Single directory: `~/.local/share/to-tui/plugins/` only
- Flat structure: `plugins/<plugin-name>/plugin.toml` (one level, no versioned subdirs)
- Malformed manifests: show visible warning in TUI status bar, skip plugin, continue startup

### Enable/disable UX
- Both config file and CLI: `totui plugin enable <name>` / `totui plugin disable <name>` updates config
- Global with per-project override: global default in `~/.config/to-tui/config.toml`, projects can override
- Enabled by default: newly discovered plugins are active unless explicitly disabled
- Immediate effect: enable/disable takes effect without restart

### Status reporting
- CLI + TUI view: `totui plugin list` command plus TUI-accessible view (keybinding TBD in Phase 12)
- Basic info in list: name, version, enabled/disabled status
- Both error surfaces: brief error state in `plugin list`, detailed diagnostics via `totui plugin status <name>`
- Basic availability check: show if plugin binary exists and manifest is valid

### Claude's Discretion
- Exact TOML parsing library choice
- Config file format for enabled/disabled state
- TUI view keybinding (Phase 12 will define, but manager should expose API)
- Log verbosity for discovery process

</decisions>

<specifics>
## Specific Ideas

- Follow the flat plugin directory pattern similar to VS Code extensions
- Warnings should be brief but visible — user shouldn't have to check logs to know something's wrong
- Enable/disable should feel instant, like toggling an extension

</specifics>

<deferred>
## Deferred Ideas

- Permissions system for plugins — future phase
- Multiple plugin directories (system + user) — potential enhancement
- Versioned plugin structure (multiple versions installed) — future consideration
- Plugin refresh command without restart — could add later if startup-only proves limiting

</deferred>

---

*Phase: 07-plugin-manager-core*
*Context gathered: 2026-01-24*
