# Phase 6: FFI-Safe Type Layer - Context

**Gathered:** 2026-01-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish stable ABI foundation with FFI-safe type definitions using abi_stable. Define FfiTodoItem, FfiTodoState, FfiPriority types and Plugin trait. Enable bidirectional conversion between native and FFI types with version compatibility protocol.

</domain>

<decisions>
## Implementation Decisions

### Type Exposure
- Full fidelity: expose all TodoItem fields to plugins (id, content, state, priority, due_date, description, parent_id, indent_level, timestamps)
- Plugins can set parent_id and indent_level directly for creating nested hierarchies
- UUIDs exposed as RString (simple, host controls generation)
- Timestamps (created_at, updated_at, completed_at) are read-only — plugins can see but not modify

### Version Protocol
- Semver compatibility: major.minor matching (1.2.x compatible with 1.3.x if same major)
- Separate interface version from to-tui app version (plugin-interface has its own versioning)
- Plugins declare minimum interface version only ("I need at least 1.2.0")
- Incompatible plugins: warn user in TUI, skip loading, continue startup

### Error Handling
- Use RResult<T, RString> at FFI boundary — simple string error messages
- Catch panics at FFI boundary with catch_unwind — convert to error, plugin stays loaded
- 3-strike system: after 3 panics in a session, disable plugin and warn user
- Plugin errors logged to standard log file (RUST_LOG controls verbosity)

### String Handling
- RString (owned) as primary string type — plugins own their data
- Reject non-UTF8 at boundary — validate and return error if invalid
- Max string length: 64KB for plugin-provided content (prevent memory issues)
- Use ROption<RString> for optional fields — explicit None, type-safe

### Claude's Discretion
- Exact StableAbi derive configurations
- Internal conversion implementation details
- Specific error message formatting
- Panic message extraction from catch_unwind

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard abi_stable approaches as documented.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-ffi-safe-type-layer*
*Context gathered: 2026-01-24*
