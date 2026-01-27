# Phase 12: Keybinding Integration - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Plugins can define custom actions triggered by keybindings. Users can discover available actions, configure custom bindings, and handle conflicts. This phase covers action registration, key routing, and user configuration — not async execution (that's event hooks).

</domain>

<decisions>
## Implementation Decisions

### Action Discovery
- Plugin actions appear in existing help panel (? keybinding)
- Grouped by plugin with separate section at bottom: "Plugin Actions" followed by per-plugin groupings
- Every action must have a description in manifest (validation fails without)
- Disabled plugins are hidden entirely from help — no grayed/marked entries

### Conflict Resolution
- Host keybinding conflicts: host wins + startup warning that plugin action has no binding
- Plugin-to-plugin conflicts: first loaded wins + warning about conflict
- Users can unbind host keybindings to give them to plugins (fully configurable)
- No reserved keys — everything is rebindable (including q, Ctrl-C, Esc)

### Invocation Feedback
- Status bar shows custom message from plugin while action runs (e.g., "Fetching JIRA-123...")
- Errors displayed in same popup as config errors (reuse existing error popup)
- Plugin actions block UI but show spinner for visual feedback
- Success shows brief status bar message for 2-3 seconds (e.g., "Done" or action-specific)

### Override Experience
- Plugin keybinding overrides live in main config.toml under [keybindings.plugins] section
- Format: nested tables — `[keybindings.plugins.jira]` with `fetch = "Ctrl+j"`
- Key sequence format matches existing to-tui keybindings format
- Users can disable actions entirely by setting to empty string or 'none'

### Claude's Discretion
- Exact spinner implementation
- Warning message formatting
- Default key assignment strategy for new plugins
- Validation error messages for invalid key sequences

</decisions>

<specifics>
## Specific Ideas

- Help panel expansion should feel natural — plugin section at bottom, grouped by plugin name
- Namespace format from roadmap: `plugin:name:action` — use this for internal routing
- Reuse existing patterns from Phase 11 error popup for action errors

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 12-keybinding-integration*
*Context gathered: 2026-01-26*
