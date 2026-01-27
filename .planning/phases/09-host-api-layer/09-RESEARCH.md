# Phase 9: Host API Layer - Research

**Researched:** 2026-01-25
**Domain:** FFI-safe plugin host API with command pattern
**Confidence:** HIGH

## Summary

This phase exposes todo CRUD operations to plugins via a PluginHostApi trait, enabling plugins to query and mutate todos while integrating with the existing undo/redo system. The research reveals that the existing codebase already has well-established patterns for undo/redo (snapshot-based, 50-state history), FFI type conversion (FfiTodoItem bidirectional), and project context that can be extended.

The key architectural decision from CONTEXT.md is using the **command pattern** with all-or-nothing atomicity for batch operations. This aligns with the established undo system which saves full TodoList snapshots. The approach is to queue commands from the plugin, then execute them atomically on the host side, saving one undo snapshot for the entire batch.

**Primary recommendation:** Extend the existing Plugin trait with a new method that receives an FfiHostApi trait object, use a command queue pattern for mutations (commands accumulate then execute atomically), and leverage the existing FFI conversion layer for type safety.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| abi_stable | 0.11 | Stable ABI for trait objects | Already in use, sabi_trait for PluginHostApi |
| uuid | 1.11 | UUID generation/parsing | Already in use, needed for todo IDs |
| chrono | 0.4 | Timestamps | Already in use for modified_at |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| RVec | abi_stable | FFI-safe Vec | Query result collections |
| RString | abi_stable | FFI-safe String | String parameters |
| ROption | abi_stable | FFI-safe Option | Optional fields |
| RResult | abi_stable | FFI-safe Result | Error handling |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Command queue | Direct mutable access | Queue preserves undo atomicity; direct would break undo |
| Full snapshot undo | Incremental undo | Snapshot is simpler, already implemented, 50 states sufficient |
| Sync command execution | Async execution | Sync simpler, plugin already runs in thread; async adds complexity |

**Installation:** No new dependencies needed - uses existing abi_stable and project infrastructure.

## Architecture Patterns

### Recommended Module Structure

```
crates/totui-plugin-interface/src/
    host_api.rs         # FfiHostApi trait with sabi_trait, FfiCommand enum
    types.rs            # Existing + FfiProjectContext, FfiTodoQuery
    plugin.rs           # Existing Plugin trait (extended)
src/plugin/
    host_impl.rs        # PluginHostApiImpl that wraps AppState operations
    command_executor.rs # Executes FfiCommand queue with undo integration
```

### Pattern 1: Command Queue Pattern

**What:** Plugin accumulates FfiCommand objects, then returns them to host for atomic execution
**When to use:** All plugin mutations - ensures single undo point for batch operations

**Design:**
```rust
// In totui-plugin-interface crate

#[repr(C)]
#[derive(StableAbi, Clone)]
pub enum FfiCommand {
    CreateTodo {
        content: RString,
        parent_id: ROption<RString>,  // UUID as string
        state: FfiTodoState,
        priority: ROption<FfiPriority>,
        indent_level: u32,
    },
    UpdateTodo {
        id: RString,  // UUID as string
        content: ROption<RString>,
        state: ROption<FfiTodoState>,
        priority: ROption<FfiPriority>,
        due_date: ROption<RString>,
        description: ROption<RString>,
    },
    DeleteTodo {
        id: RString,  // UUID as string
    },
    MoveTodo {
        id: RString,
        position: FfiMovePosition,
    },
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy)]
pub enum FfiMovePosition {
    Before { target_id: RString },
    After { target_id: RString },
    AtIndex { index: u32 },
}
```

### Pattern 2: Host API Trait Object

**What:** FFI-safe trait object passed to plugin methods for querying
**When to use:** All query operations - provides read-only access to current state

**Design:**
```rust
// In totui-plugin-interface crate

#[sabi_trait]
pub trait HostApi: Send + Sync {
    /// Get current project context
    fn current_project(&self) -> FfiProjectContext;

    /// List all available projects
    fn list_projects(&self) -> RVec<FfiProjectContext>;

    /// Query todos with filters
    fn query_todos(&self, query: &FfiTodoQuery) -> RVec<FfiTodoItem>;

    /// Get a single todo by ID
    fn get_todo(&self, id: RString) -> ROption<FfiTodoItem>;

    /// Get todos as tree structure (children nested)
    fn query_todos_tree(&self) -> RVec<FfiTodoNode>;
}

#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct FfiProjectContext {
    pub id: RString,
    pub name: RString,
    pub created_at: i64,
}

#[repr(C)]
#[derive(StableAbi, Clone, Default)]
pub struct FfiTodoQuery {
    pub project: ROption<RString>,       // None = current project
    pub state_filter: ROption<FfiStateFilter>,
    pub parent_id: ROption<RString>,     // Filter by parent
    pub include_deleted: bool,
    pub date_from: ROption<RString>,     // YYYY-MM-DD
    pub date_to: ROption<RString>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy)]
pub enum FfiStateFilter {
    Done,      // Only Checked
    Pending,   // All non-Checked
    All,       // Include everything
}
```

### Pattern 3: Extended Plugin Trait

**What:** Add new method to Plugin trait for execution with host API
**When to use:** Replace or complement `generate()` for plugins that need host interaction

**Design:**
```rust
// In totui-plugin-interface crate

#[sabi_trait]
pub trait Plugin: Send + Sync + Debug {
    fn name(&self) -> RString;
    fn version(&self) -> RString;
    fn min_interface_version(&self) -> RString;

    // Existing method - kept for backward compatibility
    #[sabi(last_prefix_field)]  // Keep this for now
    fn generate(&self, input: RString) -> RResult<RVec<FfiTodoItem>, RString>;

    // New method with host API access
    // #[sabi(last_prefix_field)]  // Move here after adding
    fn execute_with_host(
        &self,
        input: RString,
        host: HostApi_TO<'_, RBox<()>>,
    ) -> RResult<RVec<FfiCommand>, RString>;
}
```

**Note:** Per prior decision 06-02, `#[sabi(last_prefix_field)]` marks the last method for ABI extensibility. Adding execute_with_host requires moving this annotation. Existing plugins will still work with `generate()`.

### Pattern 4: Tree Structure Response

**What:** Return todos with children already linked for hierarchy traversal
**When to use:** Per CONTEXT.md decision - make hierarchy traversal easy

**Design:**
```rust
#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct FfiTodoNode {
    pub item: FfiTodoItem,
    pub children: RVec<FfiTodoNode>,
    pub position: u32,  // Index in original list
}
```

### Anti-Patterns to Avoid

- **Direct mutable host state access:** Never let plugins hold mutable references to AppState. Use command queue instead.
- **Plugin-side UUID generation:** Host should generate UUIDs in command execution to ensure uniqueness.
- **Unbounded command queues:** Limit batch size (e.g., 1000 commands max) to prevent memory issues.
- **Cross-project mutations without check:** Must verify plugin is enabled for target project before allowing queries.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| FFI-safe trait objects | Custom vtables | abi_stable `sabi_trait` | Complex, error-prone, already working |
| UUID generation | Random IDs | `uuid::Uuid::new_v4()` | Cryptographically random, tested |
| Type conversion | Manual field copying | Existing `From`/`TryFrom` impls | Already tested, handles all edge cases |
| Undo snapshots | Partial state tracking | Full TodoList clone | Simpler, already 50-state limit |

**Key insight:** The existing codebase has mature patterns for all supporting concerns. The new code should integrate with these patterns rather than create parallel mechanisms.

## Common Pitfalls

### Pitfall 1: Undo Granularity

**What goes wrong:** Each command creates separate undo entry, user must undo many times
**Why it happens:** Calling `save_undo()` per command instead of per batch
**How to avoid:** Execute command queue atomically: one `save_undo()` before, process all commands, one persist after
**Warning signs:** Undo stack grows rapidly during plugin execution

### Pitfall 2: Parent ID Invalidation

**What goes wrong:** Plugin creates child with parent_id that doesn't exist yet (queued but not created)
**Why it happens:** Commands reference UUIDs that only exist after execution
**How to avoid:** Two strategies:
1. Host generates UUIDs and returns them to plugin mid-execution (complex)
2. Use placeholder parent_ids that host resolves during execution (simpler)
**Warning signs:** "Invalid parent_id UUID" errors after plugin execution

**Recommended approach:** Use a `temp_id` field in CreateTodo, host generates real UUID and maintains temp->real mapping for subsequent commands in same batch.

### Pitfall 3: Cross-Project Access Violation

**What goes wrong:** Plugin queries project where it's not enabled
**Why it happens:** No enforcement of plugin enablement per project
**How to avoid:** HostApiImpl checks `config.plugins.is_enabled(plugin_name, project_name)` before returning data
**Warning signs:** Plugins accessing unexpected data

### Pitfall 4: FFI Panic Boundary

**What goes wrong:** Plugin panics during HostApi callback, crashes host
**Why it happens:** Callbacks cross FFI boundary without catch_unwind
**How to avoid:** Wrap all HostApi method implementations in `catch_unwind`, return RResult with error on panic
**Warning signs:** Host crash during plugin execution

### Pitfall 5: Position Drift During Batch

**What goes wrong:** MoveTodo uses indices that become invalid after earlier inserts/deletes
**Why it happens:** Batch commands modify list, indices shift
**How to avoid:** Use UUID-based positioning (before/after ID) not index-based, or recalculate indices after each command
**Warning signs:** Items appear in wrong positions after batch

## Code Examples

### Command Execution with Undo Integration

```rust
// src/plugin/command_executor.rs

impl CommandExecutor {
    pub fn execute_batch(
        &self,
        commands: Vec<FfiCommand>,
        app_state: &mut AppState,
    ) -> Result<Vec<Uuid>> {
        // Single undo point for entire batch
        app_state.save_undo();

        let mut created_ids = Vec::new();
        let mut temp_id_map: HashMap<String, Uuid> = HashMap::new();

        for cmd in commands {
            match cmd {
                FfiCommand::CreateTodo { content, parent_id, temp_id, .. } => {
                    let real_parent_id = parent_id
                        .into_option()
                        .and_then(|s| {
                            // Check temp_id_map first, then try as real UUID
                            temp_id_map.get(&s.to_string())
                                .copied()
                                .or_else(|| Uuid::parse_str(&s).ok())
                        });

                    let item = TodoItem::new(content.into(), 0);
                    let real_id = item.id;

                    // Store temp->real mapping if temp_id provided
                    if let ROption::RSome(temp) = temp_id {
                        temp_id_map.insert(temp.to_string(), real_id);
                    }

                    // Insert with proper parent relationship
                    if let Some(pid) = real_parent_id {
                        if let Some((indent, pos)) = app_state.todo_list
                            .find_insert_position_for_child(pid)
                        {
                            item.indent_level = indent;
                            item.parent_id = Some(pid);
                            app_state.todo_list.items.insert(pos, item);
                        }
                    } else {
                        app_state.todo_list.items.push(item);
                    }

                    created_ids.push(real_id);
                }
                FfiCommand::UpdateTodo { id, content, state, .. } => {
                    let uuid = resolve_id(&id, &temp_id_map)?;
                    if let Some(item) = app_state.todo_list.items
                        .iter_mut()
                        .find(|i| i.id == uuid)
                    {
                        if let ROption::RSome(c) = content {
                            item.content = c.into();
                        }
                        if let ROption::RSome(s) = state {
                            item.state = s.into();
                        }
                        item.modified_at = Utc::now();
                    } else {
                        return Err(anyhow!("Todo not found: {}", uuid));
                    }
                }
                FfiCommand::DeleteTodo { id } => {
                    let uuid = resolve_id(&id, &temp_id_map)?;
                    // Soft delete - set deleted_at
                    if let Some(item) = app_state.todo_list.items
                        .iter_mut()
                        .find(|i| i.id == uuid)
                    {
                        item.deleted_at = Some(Utc::now());
                    } else {
                        return Err(anyhow!("Todo not found: {}", uuid));
                    }
                }
                // ... MoveTodo handling
            }
        }

        app_state.todo_list.recalculate_parent_ids();
        app_state.unsaved_changes = true;

        Ok(created_ids)
    }
}
```

### HostApi Implementation

```rust
// src/plugin/host_impl.rs

pub struct PluginHostApiImpl<'a> {
    todo_list: &'a TodoList,
    current_project: &'a Project,
    project_registry: &'a ProjectRegistry,
    plugin_name: String,
    enabled_projects: HashSet<String>,
}

impl HostApi for PluginHostApiImpl<'_> {
    fn current_project(&self) -> FfiProjectContext {
        FfiProjectContext {
            id: self.current_project.id.to_string().into(),
            name: self.current_project.name.clone().into(),
            created_at: self.current_project.created_at.timestamp_millis(),
        }
    }

    fn query_todos(&self, query: &FfiTodoQuery) -> RVec<FfiTodoItem> {
        // Check project access
        let project_name = query.project
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.current_project.name.clone());

        if !self.enabled_projects.contains(&project_name) {
            // Return empty - plugin not enabled for this project
            return RVec::new();
        }

        let items: Vec<FfiTodoItem> = self.todo_list.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                // Apply filters
                if !query.include_deleted && item.deleted_at.is_some() {
                    return false;
                }
                if let ROption::RSome(ref filter) = query.state_filter {
                    match filter {
                        FfiStateFilter::Done => if !item.state.is_complete() { return false; }
                        FfiStateFilter::Pending => if item.state.is_complete() { return false; }
                        FfiStateFilter::All => {}
                    }
                }
                if let ROption::RSome(ref parent_str) = query.parent_id {
                    let parent_uuid = Uuid::parse_str(parent_str).ok();
                    if item.parent_id != parent_uuid { return false; }
                }
                true
            })
            .map(|(pos, item)| {
                let mut ffi: FfiTodoItem = item.into();
                // Position could be added to FfiTodoItem if needed
                ffi
            })
            .collect();

        items.into_iter().collect()
    }

    fn get_todo(&self, id: RString) -> ROption<FfiTodoItem> {
        Uuid::parse_str(&id)
            .ok()
            .and_then(|uuid| {
                self.todo_list.items.iter().find(|i| i.id == uuid)
            })
            .map(|item| item.into())
            .into()
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| WASM plugins | Native FFI via abi_stable | v2.0 (2026-01) | Better performance, Rust-native |
| Unload-reload plugins | Never unload (proxy pattern) | v2.0 Phase 8 | Avoids TLS destructor issues |
| Direct state mutation | Command queue | v2.0 Phase 9 | Preserves undo/redo |

**Deprecated/outdated:**
- libloading alone: abi_stable wraps it with ABI safety
- WASM for Rust-to-Rust plugins: Per user decision, native FFI preferred

## Open Questions

### 1. Position Field in FfiTodoItem

**What we know:** CONTEXT.md says "Query results include position/order index for each todo"
**What's unclear:** Should position be added to FfiTodoItem or returned separately?
**Recommendation:** Add `position: u32` field to FfiTodoItem. It's useful context and FfiTodoItem already excludes some fields (collapsed, deleted_at) so it's precedented to have different fields than native TodoItem.

### 2. Archived Todo Access

**What we know:** CONTEXT.md says "Archived todos queryable via separate method"
**What's unclear:** Whether to include in Phase 9 or defer
**Recommendation:** Include `query_archived_todos()` method in HostApi for completeness, but implementation can be simple (load from database, convert to FFI).

### 3. Cross-Project Mutation

**What we know:** CONTEXT.md says "Cross-project queries allowed when both projects have the plugin enabled"
**What's unclear:** Does this extend to mutations (creating todos in other projects)?
**Recommendation:** Phase 9 should support query across projects but mutations only in current project. Cross-project mutations add significant complexity (loading/saving multiple todo lists) - defer to future phase if needed.

## Sources

### Primary (HIGH confidence)

- Codebase analysis: `src/app/state.rs` - Existing undo/redo system using full TodoList snapshots
- Codebase analysis: `crates/totui-plugin-interface/src/` - Existing FFI types and Plugin trait
- Codebase analysis: `src/plugin/loader.rs` - Plugin loading with call_safely pattern
- [abi_stable documentation](https://docs.rs/abi_stable/latest/abi_stable/) - sabi_trait usage

### Secondary (MEDIUM confidence)

- [NullDeref Plugin Article](https://nullderef.com/blog/plugin-abi-stable/) - sabi_trait patterns with state passing
- [Refactoring.guru Command Pattern](https://refactoring.guru/design-patterns/command/rust/example) - Rust command pattern with undo
- Phase CONTEXT.md - User decisions on query interface, mutation contract, project context

### Tertiary (LOW confidence)

- [Arroyo Plugin Systems](https://www.arroyo.dev/blog/rust-plugin-systems/) - General plugin architecture patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Uses only existing dependencies
- Architecture: HIGH - Follows established codebase patterns
- Pitfalls: HIGH - Based on codebase analysis of existing edge cases

**Research date:** 2026-01-25
**Valid until:** 2026-02-25 (stable domain, no fast-moving external deps)
