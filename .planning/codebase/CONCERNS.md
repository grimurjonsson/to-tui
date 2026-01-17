# Codebase Concerns

**Analysis Date:** 2026-01-17

## Tech Debt

**Undo History Memory Usage:**
- Issue: Full `TodoList` clones stored for each undo step (up to 50 entries)
- Files: `src/app/state.rs` (lines 153-159)
- Impact: Memory grows linearly with undo depth. For large todo lists, 50 full clones can consume significant memory
- Fix approach: Implement command pattern with reversible operations instead of full snapshots

**Database Initialization Called Repeatedly:**
- Issue: `init_database()` called on every load/save operation, creating tables if not exists each time
- Files: `src/storage/file.rs` (lines 11, 39), `src/storage/database.rs` (line 99-180)
- Impact: Unnecessary overhead on every file operation, executes 8+ SQL statements per save/load
- Fix approach: Initialize once at app startup, store connection in AppState or use lazy_static

**Clone-Heavy Code Patterns:**
- Issue: 38 `.clone()` calls across 10 files, many potentially avoidable
- Files: `src/ui/components/todo_list.rs` (12 clones), `src/app/event.rs` (10 clones)
- Impact: Unnecessary allocations in hot paths (UI rendering loop)
- Fix approach: Use references where possible, implement Copy for small types, use Cow<str>

**Plugin Thread Communication:**
- Issue: Plugin execution spawns new thread and recreates PluginRegistry inside thread
- Files: `src/app/event.rs` (lines 827-836)
- Impact: Redundant initialization, thread spawned without proper lifecycle management
- Fix approach: Pass generator reference via Arc, or use async task with tokio runtime

## Known Bugs

**API Delete Does Not Soft Delete:**
- Symptoms: REST API delete bypasses soft-delete, directly drains items from list
- Files: `src/api/handlers.rs` (lines 83-89)
- Trigger: Call DELETE /api/todos/:id via REST API
- Workaround: Use MCP server or TUI for deletions that need soft-delete

**Missing Redo Functionality:**
- Symptoms: No way to redo after undo; undo stack is cleared but no redo stack exists
- Files: `src/app/state.rs` (lines 161-170)
- Trigger: Accidentally undo an action
- Workaround: None - change is lost unless you manually recreate it

**Key Sequence Timeout Edge Case:**
- Symptoms: If sequence starter key (like 'd') also has single action, it always returns Pending
- Files: `src/keybindings/mod.rs` (lines 482-487)
- Trigger: Single 'd' press waits for timeout even when 'd' could have standalone action
- Workaround: Wait for timeout to expire

## Security Considerations

**Process Spawning Without Input Validation:**
- Risk: Plugin subprocess executes arbitrary commands with user-provided input
- Files: `src/plugin/subprocess.rs`, `src/plugin/generators/jira_claude.rs` (line 159)
- Current mitigation: Commands are hardcoded (`acli`, `claude`), but input passed directly to args
- Recommendations: Validate/sanitize ticket IDs, limit allowed characters, add input length limits

**PID File Race Condition:**
- Risk: TOCTOU race between checking server running and writing PID file
- Files: `src/main.rs` (lines 176-196)
- Current mitigation: None
- Recommendations: Use file locking (flock) for PID file

**API Server Binds to 0.0.0.0:**
- Risk: API server accessible from any network interface, no authentication
- Files: `src/main.rs` (line 267)
- Current mitigation: Default port 48372 is obscure
- Recommendations: Bind to 127.0.0.1 by default, add optional authentication

## Performance Bottlenecks

**Full List Serialization on Every Save:**
- Problem: Every edit triggers full markdown serialization and database save
- Files: `src/storage/file.rs` (lines 37-57)
- Cause: No dirty tracking, no batching/debouncing
- Improvement path: Debounce saves, track changed items only, batch database updates

**Hidden Indices Recalculated Per Render:**
- Problem: `build_hidden_indices()` called on every frame render
- Files: `src/ui/components/todo_list.rs` (line 49)
- Cause: No caching of collapsed state hierarchy
- Improvement path: Cache hidden set, invalidate only on collapse/expand

**Markdown Parsing Allocations:**
- Problem: Multiple string allocations during parsing
- Files: `src/storage/markdown.rs` (lines 20-100)
- Cause: Using owned Strings instead of slices
- Improvement path: Use Cow<str>, parse into pre-allocated buffers

## Fragile Areas

**Parent ID Recalculation:**
- Files: `src/todo/hierarchy.rs`, `src/todo/list.rs`
- Why fragile: Parent IDs must be recalculated after many operations (indent, outdent, move, delete)
- Safe modification: Always call `recalculate_parent_ids()` after structural changes
- Test coverage: Good coverage (hierarchy tests), but edge cases with deep nesting untested

**Edit Mode State Machine:**
- Files: `src/app/event.rs` (lines 511-715)
- Why fragile: Complex interaction between `is_creating_new_item`, `insert_above`, `pending_indent_level`
- Safe modification: Test all combinations of: new item, existing item, insert above/below, with/without indent
- Test coverage: No dedicated tests for edit mode transitions

**Keybinding Normalization:**
- Files: `src/keybindings/mod.rs` (lines 190-217)
- Why fragile: Special handling for shifted keys, BackTab, angle brackets
- Safe modification: Add tests for any new special key handling
- Test coverage: Some tests exist but edge cases like platform differences untested

## Scaling Limits

**Single-File Database:**
- Current capacity: SQLite handles millions of rows fine
- Limit: Single connection, no concurrent writes from API and TUI
- Scaling path: Use connection pooling, or switch to write-through cache

**In-Memory Todo List:**
- Current capacity: Works well for hundreds of items
- Limit: UI performance degrades with thousands of items (full list iteration per render)
- Scaling path: Virtualized list rendering, load items on demand

## Dependencies at Risk

**rmcp (MCP SDK):**
- Risk: Relatively new crate, API may change
- Impact: MCP server functionality
- Migration plan: Monitor for breaking changes, consider vendoring if abandoned

**ratatui Ecosystem:**
- Risk: Low (well-maintained), but major versions sometimes break widgets
- Impact: All UI rendering
- Migration plan: Lock to specific version, test on upgrade

## Missing Critical Features

**No Concurrent Edit Protection:**
- Problem: TUI and API server can both modify the same list without coordination
- Blocks: Reliable multi-client usage
- Impact: Last writer wins, data loss possible

**No Data Backup/Export:**
- Problem: No built-in way to export data or create backups
- Blocks: Safe data portability
- Note: Markdown files can be manually copied, but database has no export

**No Search/Filter:**
- Problem: Cannot search across todos or filter by state/date
- Blocks: Finding items in large lists or historical data

## Test Coverage Gaps

**UI Rendering:**
- What's not tested: All rendering code in `src/ui/`
- Files: `src/ui/components/todo_list.rs`, `src/ui/components/status_bar.rs`, `src/ui/mod.rs`
- Risk: Visual regressions unnoticed
- Priority: Medium (manual testing catches most issues)

**API Handlers:**
- What's not tested: HTTP request/response handling
- Files: `src/api/handlers.rs`, `src/api/routes.rs`
- Risk: API contract changes unnoticed
- Priority: High (API is used by external clients)

**MCP Server:**
- What's not tested: MCP tool implementations
- Files: `src/mcp/server.rs`
- Risk: LLM integration breaks silently
- Priority: High (critical for AI integration)

**Plugin System:**
- What's not tested: Plugin execution, error handling
- Files: `src/plugin/generators/jira_claude.rs`, `src/plugin/subprocess.rs`
- Risk: Jira integration failures
- Priority: Medium (requires external services)

**File Watcher:**
- What's not tested: File change detection and reload
- Files: `src/ui/mod.rs` (lines 65-90)
- Risk: External file changes not detected
- Priority: Low

## Additional Observations

**Unwrap Usage in Non-Test Code:**
- Files: `src/main.rs` (line 159), `src/keybindings/mod.rs` (lines 292-332)
- Issue: `unwrap()` calls can panic in production
- Recommendation: Replace with proper error handling or `expect()` with descriptive message

**No Logging in TUI:**
- Issue: TUI runs without structured logging, debugging requires println
- Files: All TUI code
- Recommendation: Add tracing subscriber that writes to file, queryable after TUI exits

**Hardcoded Magic Numbers:**
- Issue: Timeouts, limits scattered through code
- Files: `src/main.rs` (500ms sleeps), `src/app/state.rs` (MAX_UNDO_HISTORY = 50)
- Recommendation: Centralize configuration constants

---

*Concerns audit: 2026-01-17*
