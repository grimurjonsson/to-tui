# Phase 10: Metadata & Database - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable plugins to persist custom JSON metadata attached to todos and projects. Metadata is namespaced per-plugin, persists in SQLite with JSON1 support, and integrates with undo/redo. This phase covers storage and CRUD operations — plugin configuration files are Phase 11.

</domain>

<decisions>
## Implementation Decisions

### Metadata Scoping
- Auto-namespaced by plugin name (from manifest)
- Strictly isolated — plugins cannot read other plugins' metadata
- Metadata attaches to both todos AND projects
- Plugin name from `plugin.toml` serves as namespace key

### API Design
- Both individual and batch operations supported
  - `get_metadata(todo_id)`, `set_metadata(todo_id, data, merge: bool)`
  - `get_metadata_batch(todo_ids)`, `set_metadata_batch(items)`
- `set_metadata` takes explicit `merge` parameter — caller chooses replace vs merge
- `get_metadata` returns empty object `{}` when no metadata exists (never null)
- All metadata operations go through undo/redo system (consistent with todo mutations)

### Lifecycle Behavior
- Soft-delete: Metadata kept with deleted todo (recoverable on restore)
- Duplicate: Metadata copies to new todo
- Archive: Metadata moves to archive table with todo
- Rollover: Metadata preserved when incomplete todos roll to new day

### Schema Constraints
- No size limit — trust plugins, SQLite handles it
- Validate as JSON before storage — reject invalid JSON with error
- Reserve `_` prefix for future host use — plugins cannot use keys starting with `_`
- Database column uses JSON type with SQLite JSON1 functions enabled

### Query Capabilities
- `query_todos_by_metadata(key, value)` — find todos by metadata values
- `list_projects_with_metadata()` — discover projects with plugin's metadata
- Prefix queries supported — query all keys matching a prefix pattern
- Projects require explicit creation before attaching metadata

### Claude's Discretion
- Exact batch operation return types
- Index strategy for metadata queries
- Error message formatting for JSON validation failures
- Whether to use json_extract or json_each for queries

</decisions>

<specifics>
## Specific Ideas

- JSON1 enables powerful queries like finding all Jira tickets by ticket_id
- Prefix queries enable hierarchical metadata organization within a plugin's namespace
- Explicit project creation aligns with future project management features

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 10-metadata-database*
*Context gathered: 2026-01-26*
