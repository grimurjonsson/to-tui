# Phase 13: Event Hooks - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Plugins can respond to todo lifecycle events asynchronously. This enables reactive plugins that auto-tag, enrich, sync, or log todo changes. Distribution, installation, and plugin migration are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Event Types
- Four event types: on-add, on-modify, on-complete, on-delete
- on-add fires for every new todo (manual, rollover, plugin-generated)
- on-modify fires for any property change (content, state, due date, priority, indent, parent)
- on-delete fires when todos are soft-deleted
- Individual events only (no batch aggregation) — one event per todo change
- All events are opt-in — plugins subscribe only to events they need

### Hook Responses
- Hooks can modify the todo that triggered them (return changes)
- No cascade — hook-triggered modifications don't fire new events (prevents infinite loops)
- No veto power — hooks react after the fact, operation always completes
- Hook receives current (after) state only, not before/after diff

### Execution Timing
- Async (non-blocking) — UI continues immediately, hooks run in background
- Silent UI refresh when async hooks modify todos (no notification)
- Sequential execution in plugin load order when multiple plugins subscribe to same event
- Optional on-load event for startup (plugins can subscribe if needed)

### Failure Handling
- Configurable timeout per plugin (declared in manifest)
- Error popup for hook failures (use existing plugin error popup infrastructure)
- No automatic retry — one attempt per event
- Auto-disable hook after N consecutive failures (session-disable)

### Claude's Discretion
- Default timeout value if plugin doesn't specify
- Exact N for auto-disable threshold
- on-load event implementation details
- Hook registration mechanism (manifest vs Plugin trait method)

</decisions>

<specifics>
## Specific Ideas

- Events are opt-in — some plugins only generate todos and don't need any hooks
- Reuse existing error popup infrastructure from Phase 8
- Sequential execution preserves predictable behavior when multiple plugins modify same todo

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 13-event-hooks*
*Context gathered: 2026-01-26*
