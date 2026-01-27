# Phase 9: Host API Layer - Context

**Gathered:** 2026-01-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Expose todo CRUD operations to plugins via PluginHostApi with undo/redo support. Plugins can query, create, update, delete, and reorder todos. Plugin receives project context and can operate across projects.

</domain>

<decisions>
## Implementation Decisions

### Query Interface
- Cross-project queries allowed when both projects have the plugin enabled
- Query results are immutable snapshots (not live references)
- Basic filtering available: by state (done/pending), parent_id, date range
- Archived todos queryable via separate method
- Soft-deleted todos visible via opt-in `include_deleted` flag
- Pre-built tree structure returned (children already linked to parents)
- Single-item lookup via `get_by_id(uuid)` method
- Query results include position/order index for each todo

### Mutation Contract
- Batch operations supported with all-or-nothing atomicity
- Mutations return the created/updated todo item (with UUID, timestamps)
- Reordering supported via move operation (move_before, move_after, or set_position)

### Project Context
- Current project passed on invoke AND queryable via method
- Plugins can switch project context explicitly during execution
- Plugins can list all available projects via `list_projects()` method
- Plugins can create new projects via `create_project()` method

### Error Handling
- Invalid todo UUIDs return NotFound error (not silent no-op)
- All plugin errors logged by host for debugging

### Claude's Discretion
- Mutation submission style (command queue vs direct API calls) — choose based on undo/redo integration
- Error type granularity (generic vs typed enum) — choose based on FFI complexity
- Batch failure behavior (rollback vs partial results) — choose based on undo integration

</decisions>

<specifics>
## Specific Ideas

- Cross-project access respects plugin enablement — plugin can only query projects where it's enabled
- Tree structure should make hierarchy traversal easy for plugins that generate nested todos

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 09-host-api-layer*
*Context gathered: 2026-01-25*
