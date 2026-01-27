# Phase 11: Plugin Configuration - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Each plugin has isolated configuration with schema validation. Plugins define their expected config, users provide config files, host validates and passes parsed values to plugins at initialization. Hot reload is out of scope.

</domain>

<decisions>
## Implementation Decisions

### Config Schema Format
- Plugin exposes schema programmatically via API method (for tooling like `totui plugin config <name>`)
- Supported types: basic only — string, integer, boolean, array of strings
- Schema is strictly enforced — invalid config prevents plugin from loading

### Validation & Errors
- Error messages are specific with field names: "api_key: expected string, got integer"
- Errors surfaced via TUI popup on startup (same pattern as plugin panics)
- CLI command `totui plugin validate <name>` checks config without starting TUI
- Multiple plugin errors shown in single popup listing all affected plugins

### Default Values
- Plugin defines whether config is required or optional in schema
- Per-field optionality — schema marks each field as required or optional with default
- Missing config file: if plugin requires config, fail with clear error

### Config Lifecycle
- Config read once at plugin startup, never re-read during session
- No hot reload — config changes require TUI restart
- Host parses TOML, passes typed values via FFI (not raw string)
- Plugin receives `on_config_loaded()` callback after config is set

### Claude's Discretion
- Schema definition location (inline in manifest vs separate file)
- Where default values are defined (schema vs plugin code)
- Whether `totui plugin config <name> --init` generates template from schema
- Exact FFI representation for passing typed config values

</decisions>

<specifics>
## Specific Ideas

- Config directory structure: `~/.config/to-tui/plugins/<name>/config.toml`
- Follow existing error popup pattern from Phase 8 for consistency
- Validation CLI useful for CI/scripting — checking config before deployment

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 11-plugin-configuration*
*Context gathered: 2026-01-26*
