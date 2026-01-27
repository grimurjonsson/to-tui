# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-21)

**Core value:** Fast, keyboard-driven todo management that lives in the terminal and integrates with the tools I already use.
**Current focus:** v2.0 Plugin Framework — Phase 15 in progress

## Current Position

Milestone: v2.0 Plugin Framework
Phase: 15 of 15 (Migration)
Plan: 2 of 3 in current phase
Status: In progress
Last activity: 2026-01-26 — Completed 15-02-PLAN.md

Progress: [██████████████░] 96%

## Performance Metrics

**Velocity:**
- Total plans completed: 15 (v1.0) + 24 (v2.0) = 39
- Average duration: ~45 min (v1.0 baseline)
- Total execution time: ~11 hours (v1.0)

**By Phase (v2.0):**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 6 | 2/2 | 15min | 7.5min |
| 7 | 3/3 | 9min | 3min |
| 8 | 2/2 | 12min | 6min |
| 9 | 4/4 | 19min | 4.75min |
| 10 | 3/3 | 11min | 3.7min |
| 11 | 2/2 | 13min | 6.5min |
| 12 | 2/2 | 23min | 11.5min |
| 13 | 3/3 | 29min | 9.7min |
| 14 | 3/3 | 18min | 6min |
| 15 | 2/3 | 14min | 7min |

**Recent Trend:**
- v2.0 Phase 15 plan 2 complete: Removed built-in Jira generator, LoadedPlugin with version/description
- v2.0 Phase 15 plan 1 complete: jira-claude plugin crate in to-tui-plugins repo
- v2.0 Phase 14 plan 3 complete: Marketplace support with version lookup and source tracking
- v2.0 Phase 14 plan 2 complete: Remote plugin installation from GitHub releases
- v2.0 Phase 14 plan 1 complete: Local plugin installation with directory copy
- v2.0 Phase 13 plan 3 complete: TUI integration with cascade prevention
- v2.0 Phase 13 plan 2 complete: HookDispatcher with async dispatch and failure tracking
- v2.0 Phase 13 plan 1 complete: FFI event types and Plugin trait extension
- v2.0 Phase 12 plan 2 complete: TUI integration (config overrides, event routing, help panel)
- v2.0 Phase 12 plan 1 complete: ActionDefinition manifest, PluginActionRegistry
- v2.0 Phase 11 plan 2 complete: PluginLoader integration, CLI commands, TUI config errors
- v2.0 Phase 11 plan 1 complete: FFI config types and host-side loader
- v2.0 Phase 10 plan 3 complete: Host implementation wiring (CommandExecutor metadata handling)
- v2.0 Phase 10 plan 2 complete: FFI metadata extensions
- v2.0 Phase 10 plan 1 complete: Storage layer CRUD operations (metadata.rs)
- v2.0 Phase 9 plan 4 complete: Gap closure - calling convention with undo integration
- v2.0 Phase 9 plan 3 complete: CommandExecutor with temp ID mapping
- v2.0 Phase 9 plan 2 complete: PluginHostApiImpl query implementation
- v2.0 Phase 9 plan 1 complete: Host API types (FfiCommand, HostApi trait)
- v2.0 Phase 8 plan 2 complete: TUI plugin integration with error popup
- v2.0 Phase 8 plan 1 complete: PluginLoader with abi_stable loading and panic safety
- v2.0 Phase 7 complete: Plugin management infrastructure
- v2.0 Phase 6 complete: FFI-safe type layer finished

*Updated after each plan completion*

## Completed Milestones

| Milestone | Phases | Plans | Shipped |
|-----------|--------|-------|---------|
| v1.0 TUI Enhancements | 5 | 15 | 2026-01-21 |

## Quick Tasks

Quick tasks are ad-hoc improvements outside milestone planning:

| Task ID | Description | Status | Completed |
|---------|-------------|--------|-----------|
| quick-001 | Simplify README installation and MCP sections | Done | 2026-01-21 |
| 001-fix-row-highlighting | Fix visual row highlighting on new item creation | Done | 2026-01-22 |
| 003-cascading-x-toggle | Cascading 'x' toggle for parent and children with undo | Done | 2026-01-22 |
| 004-tar-gz-release-binaries | Change release format to tar.gz/zip archives | Done | 2026-01-22 |
| 005-github-link-status-bar | Add clickable GitHub octopus link to status bar | Done | 2026-01-22 |
| 006-move-item-and-subtree-to-another-project | Move todos between projects with 'm' keybinding | Done | 2026-01-22 |
| 007-project-filter-mcp-api | Update plugin to v0.4.0 (project filtering) | Done | 2026-01-23 |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- v2.0: Use abi_stable for stable ABI (not WASM)
- v2.0: Three-crate architecture (interface, host, plugins)
- v2.0: Command pattern for plugin mutations (preserve undo/redo)
- v2.0: Never unload plugins (TLS safety)
- 06-01: Use u32 for indent_level (usize not FFI-safe)
- 06-01: Use i64 timestamps instead of DateTime<Utc>
- 06-01: Exclude collapsed/deleted_at from FFI types
- 06-02: Use #[sabi(last_prefix_field)] on generate() for future extensibility
- 06-02: Semver compatibility: same major + host >= plugin_min
- 06-02: Plugin_TO naming follows abi_stable underscore convention
- 07-01: Default impl with placeholders for manifest error cases
- 07-01: Unknown TOML fields ignored for forward compatibility
- 07-02: Error capture in PluginInfo.error, not panicked
- 07-02: Availability separate from error (version mismatch vs parse failure)
- 07-02: Case-insensitive plugin name lookup via to_lowercase()
- 07-03: Plugins enabled by default (disabled set excludes)
- 07-03: Moved config/keybindings modules to lib.rs for library access
- 08-01: Use abi_stable's library leaking as proxy pattern implementation
- 08-01: Session-disabled plugins return SessionDisabled error on subsequent calls
- 08-01: call_safely() wraps all plugin method calls in catch_unwind
- 08-02: Test-only methods for future phases (handle_plugin_panic, plugin_loader_mut)
- 08-02: Popup dismissal via event loop interception before handle_key_event
- 09-01: HostApi uses sabi_trait generating HostApi_TO for FFI-safe trait object
- 09-01: FfiCommand uses repr(C) enum for mutation operations
- 09-01: query_todos_tree marked with last_prefix_field for future extensibility
- 09-01: position field added to FfiTodoItem (host-assigned during query)
- 09-02: PluginHostApiImpl requires both plugin name and project for scope
- 09-02: query_todos filters deleted items by default (unless include_deleted)
- 09-02: Tree building uses parent_id to nest children under parents
- 09-03: Temp ID mapping - check temp_id_map first, then parse as UUID
- 09-03: Soft delete for DeleteTodo command (deleted_at timestamp)
- 09-03: Return error on not-found UUIDs (fail fast, don't silently skip)
- 09-04: Calling convention in AppState (not CommandExecutor) - owns undo_stack and todo_list
- 10-01: Metadata indexed by (entity_id, plugin_name) unique constraint
- 10-01: Return {} for non-existent metadata (not null/error)
- 10-01: Reserved key prefix _ rejected at validation layer
- 10-01: Shallow JSON merge (new keys overwrite existing)
- 10-02: Metadata stored as JSON string (validated by host)
- 10-02: Merge flag allows incremental vs full replacement
- 10-02: Batch metadata query returns FfiTodoMetadata vec
- 10-02: query_todos_by_metadata takes key/value for flexible filtering
- 10-03: CommandExecutor gets plugin_name field for metadata namespace
- 10-03: Default impl uses empty string for plugin_name
- 11-01: Schema via Plugin trait method (config_schema()), not manifest
- 11-01: Defaults in schema via FfiConfigField.default
- 11-01: RHashMap for FFI config values (natural key-value access)
- 11-02: ConfigError separate from PluginLoadError for clean separation
- 11-02: Convert ConfigError to PluginLoadError for unified popup display
- 11-02: Store config_errors in PluginLoader for retrieval after loading
- 12-01: Action names must be valid identifiers (alphanumeric + underscore)
- 12-01: Keybinding validation via KeySequence::parse at manifest validation
- 12-01: Plugin-to-plugin keybinding conflicts: first wins, second gets warning
- 12-01: Action namespace format: plugin:{plugin_name}:{action_name}
- 12-02: Plugin keybinding overrides under [keybindings.plugins.{name}] section
- 12-02: Build PluginActionRegistry in main.rs before AppState construction
- 12-02: Check plugin actions only when host keybinding returns None (host wins)
- 12-02: Use existing error popup infrastructure for plugin action errors
- 13-01: FfiEventType separate enum (not derive from FfiEvent discriminant)
- 13-01: FfiFieldChange indicates single modified field or Multiple
- 13-01: FfiEventSource tracks event origin (Manual, Rollover, Plugin, Api)
- 13-01: on_event() is last_prefix_field (ABI extensibility)
- 13-02: Synchronous hook call with watchdog timeout (Plugin_TO not clonable)
- 13-02: Auto-disable after 3 consecutive failures (AUTO_DISABLE_THRESHOLD)
- 13-02: Event subscriptions populated at plugin load time
- 13-03: Hook commands applied without undo (secondary effects)
- 13-03: OnDelete fires before deletion (capture item data)
- 13-03: in_hook_apply flag for cascade prevention
- 14-01: Copy directory contents instead of symlinks for local install
- 14-01: Detect local paths by prefix (/, ./, ../, ~) or filesystem existence
- 14-01: Require --force flag to overwrite existing installations
- 14-02: Reuse get_target_triple() from upgrade.rs for platform detection
- 14-02: Version required for remote install (latest lookup in 14-03)
- 14-02: Download URL format: owner/repo/releases/download/v{version}/plugin-{target}.tar.gz
- 14-02: 404 response produces helpful platform error message
- 14-03: Source tracking via .source file (simple persistence, survives plugin updates)
- 14-03: Case-insensitive plugin lookup in marketplace
- 14-03: Tabular plugin list format with NAME, VERSION, STATUS, SOURCE columns
- 14-03: Default marketplace configurable but hardcoded fallback
- 15-01: Use local path dependency for plugin development
- 15-01: Direct subprocess execution via std::process::Command
- 15-01: Generator-only pattern (no config, no events)
- 15-02: Remove plugin_registry field from AppState entirely
- 15-02: Call plugins synchronously instead of spawning thread
- 15-02: Add description field alongside version for UI display

### Pending Todos

None yet.

### Blockers/Concerns

- **Plugin release blocked:** totui-plugin-interface not pushed to GitHub. Must push Phases 6-14 work before jira-claude plugin can be released/installed.

## Session Continuity

Last session: 2026-01-26
Stopped at: Completed 15-02-PLAN.md
Resume file: None
Next action: Execute 15-03-PLAN.md (cleanup and documentation)
