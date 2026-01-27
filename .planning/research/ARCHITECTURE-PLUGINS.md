# Architecture Research: Dynamic Plugin System

**Domain:** Dynamic Rust plugin system for TUI application
**Researched:** 2026-01-24
**Overall Confidence:** HIGH (based on analysis of existing codebase + verified external sources)

## Executive Summary

This research analyzes how dynamic plugins should integrate with the existing to-tui architecture. The codebase already has a basic plugin system (`src/plugin/`) with a `TodoGenerator` trait and `PluginRegistry`, but this is compile-time only. Extending to dynamic loading requires careful consideration of Rust's ABI instability.

**Recommendation:** Use **WebAssembly (WASM)** for dynamic plugins rather than native dynamic linking. WASM provides:
1. Sandboxed execution (plugins cannot crash or compromise the host)
2. Cross-platform portability (compile once, run anywhere)
3. Language flexibility (plugins can be written in Rust, Go, TypeScript, etc.)
4. Stable interface (WASM has a defined ABI, unlike Rust)

Native dynamic linking with `libloading` or `abi_stable` is viable but introduces significant complexity around Rust ABI stability, version mismatches, and security concerns.

## Integration Points

### With AppState (`src/app/state.rs`)

Current state exposes plugins via `plugin_registry: PluginRegistry` and `plugin_state: Option<PluginSubState>`.

**Integration approach:**
```rust
// New fields needed in AppState
pub struct AppState {
    // ... existing fields ...
    pub plugin_manager: PluginManager,        // NEW: manages dynamic plugins
    pub plugin_host_api: Arc<PluginHostApi>,  // NEW: shared API for plugin calls
}
```

**Plugin access to state:**
- Plugins should NOT get direct mutable access to AppState
- Instead, provide a controlled API through `PluginHostApi`
- Changes go through a message/command queue that AppState processes

**Data flow for plugin modifications:**
```
Plugin calls HostApi::create_todo(...)
  -> PluginHostApi queues PluginCommand::CreateTodo { ... }
  -> Event loop processes command
  -> AppState applies change via existing save_undo() + todo_list mutation
  -> UI re-renders
```

### With Keybindings (`src/keybindings/mod.rs`)

Current system has `Action` enum with hardcoded actions and `KeybindingCache` for lookup.

**Integration approach:**
```rust
// Extend Action enum
pub enum Action {
    // ... existing variants ...

    // NEW: Plugin-registered action
    PluginAction {
        plugin_id: String,
        action_name: String
    },
}

// Plugin registration adds to KeybindingsConfig dynamically
impl PluginManager {
    pub fn register_keybinding(
        &mut self,
        plugin_id: &str,
        key: &str,      // e.g. "<C-x>p"
        action: &str,   // e.g. "sync_jira"
    ) -> Result<()>;
}
```

**Key routing:**
1. `KeybindingCache::lookup_navigate()` returns `Action::PluginAction { ... }`
2. `execute_navigate_action()` dispatches to `PluginManager::invoke()`
3. Plugin executes, returns commands to host
4. Host applies commands to AppState

### With Storage (`src/storage/database.rs`, `src/storage/file.rs`)

**Read-only access is safe:**
- Plugins can query todos via `PluginHostApi::query_todos(filter)`
- Returns cloned data, no mutable references

**Plugin metadata storage:**
- New table: `plugin_metadata` for plugin-specific persistent data
- Keyed by (plugin_id, todo_id, key) for item-level metadata
- Keyed by (plugin_id, key) for global metadata

```sql
CREATE TABLE plugin_metadata (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    todo_id TEXT,  -- NULL for global metadata
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(plugin_id, todo_id, key)
);
CREATE INDEX idx_plugin_metadata_plugin ON plugin_metadata(plugin_id);
CREATE INDEX idx_plugin_metadata_todo ON plugin_metadata(todo_id);
```

### With Event System (`src/app/event.rs`)

Current event handling is in `handle_key_event()` and `handle_mouse_event()`.

**Plugin event hooks:**
```rust
// Event hook trait for plugins
pub trait PluginEventHandler {
    fn on_todo_created(&self, todo: &TodoItem) -> Vec<PluginCommand>;
    fn on_todo_completed(&self, todo: &TodoItem) -> Vec<PluginCommand>;
    fn on_todo_deleted(&self, todo: &TodoItem) -> Vec<PluginCommand>;
    fn on_key_press(&self, key: &KeyEvent) -> Option<Vec<PluginCommand>>;
    fn on_tick(&self) -> Vec<PluginCommand>;
}
```

**Event flow:**
```
KeyEvent received
  -> handle_key_event()
  -> If Action::PluginAction: invoke plugin
  -> After state change: broadcast to all plugins' on_* hooks
  -> Collect PluginCommands
  -> Process commands (may trigger more hooks - limit recursion depth)
```

## New Components

### PluginManager (`src/plugin/manager.rs`)

**Responsibilities:**
1. Load plugins from `~/.config/to-tui/plugins/`
2. Manage plugin lifecycle (init, run, shutdown)
3. Route events to plugins
4. Execute plugin commands
5. Handle plugin errors gracefully

```rust
pub struct PluginManager {
    plugins: HashMap<String, LoadedPlugin>,
    config_dir: PathBuf,
    host_api: Arc<PluginHostApi>,
    command_queue: VecDeque<PluginCommand>,
}

struct LoadedPlugin {
    id: String,
    manifest: PluginManifest,
    instance: Box<dyn Plugin>,  // WASM instance or trait object
    registered_keys: Vec<KeyBinding>,
    permissions: PluginPermissions,
}

impl PluginManager {
    pub fn load_all(&mut self) -> Result<()>;
    pub fn invoke(&mut self, plugin_id: &str, action: &str, ctx: &PluginContext) -> Result<()>;
    pub fn broadcast_event(&mut self, event: PluginEvent) -> Vec<PluginCommand>;
    pub fn process_commands(&mut self, state: &mut AppState) -> Result<()>;
}
```

### PluginHostApi (`src/plugin/host_api.rs`)

**What the host exposes to plugins (safe subset):**

```rust
pub struct PluginHostApi {
    // Read operations (safe)
    pub fn query_todos(&self, filter: TodoFilter) -> Vec<TodoItemSnapshot>;
    pub fn get_current_date(&self) -> NaiveDate;
    pub fn get_current_project(&self) -> String;
    pub fn get_selected_todo(&self) -> Option<TodoItemSnapshot>;

    // Read metadata
    pub fn get_metadata(&self, key: &str) -> Option<String>;
    pub fn get_todo_metadata(&self, todo_id: Uuid, key: &str) -> Option<String>;

    // Write operations (return commands, don't mutate directly)
    pub fn create_todo(&self, content: String, indent: usize) -> PluginCommand;
    pub fn update_todo(&self, id: Uuid, updates: TodoUpdates) -> PluginCommand;
    pub fn set_metadata(&self, key: &str, value: &str) -> PluginCommand;
    pub fn show_notification(&self, msg: &str) -> PluginCommand;
    pub fn register_keybinding(&self, key: &str, action: &str) -> PluginCommand;
}

// Immutable snapshot for safe sharing
pub struct TodoItemSnapshot {
    pub id: Uuid,
    pub content: String,
    pub state: TodoState,
    pub indent_level: usize,
    pub parent_id: Option<Uuid>,
    pub due_date: Option<NaiveDate>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
}
```

### PluginCommand (`src/plugin/command.rs`)

**Commands plugins return to host:**

```rust
pub enum PluginCommand {
    // Todo mutations
    CreateTodo { content: String, indent_level: usize, parent_id: Option<Uuid> },
    UpdateTodo { id: Uuid, updates: TodoUpdates },
    DeleteTodo { id: Uuid },

    // Metadata
    SetMetadata { key: String, value: String },
    SetTodoMetadata { todo_id: Uuid, key: String, value: String },

    // UI
    ShowNotification { message: String, duration_secs: u8 },
    SetStatusMessage { message: String },

    // Keybindings (registration phase only)
    RegisterKeybinding { key: String, action: String },
}
```

### Plugin Manifest (`~/.config/to-tui/plugins/<name>/manifest.toml`)

```toml
[plugin]
name = "jira-sync"
version = "0.1.0"
description = "Sync todos with Jira tickets"
author = "Your Name"
entry = "plugin.wasm"  # or "plugin.so" for native

[permissions]
network = true         # Can make HTTP requests
filesystem = false     # No filesystem access beyond plugin dir
execute = false        # Cannot spawn processes

[keybindings]
"<C-j>s" = "sync_ticket"      # Ctrl+j then s
"<C-j>c" = "create_ticket"    # Ctrl+j then c

[hooks]
on_todo_created = true
on_todo_completed = true
```

## Data Flow

### Key Press to Plugin Action

```
1. User presses <C-x>p (registered by plugin)
   |
2. handle_key_event() in src/app/event.rs
   |
3. KeybindingCache::lookup_navigate() returns Action::PluginAction
   |
4. execute_navigate_action() matches PluginAction:
   |  plugin_manager.invoke("myplugin", "my_action", ctx)
   |
5. PluginManager::invoke():
   |  - Finds LoadedPlugin by id
   |  - Creates PluginContext with current state snapshot
   |  - Calls plugin.handle_action("my_action", ctx)
   |
6. Plugin executes (in WASM sandbox):
   |  - Queries host_api for current todo
   |  - Decides what to do
   |  - Returns Vec<PluginCommand>
   |
7. PluginManager::process_commands():
   |  for cmd in commands:
   |    match cmd {
   |      CreateTodo { .. } => state.todo_list.add_item(...),
   |      ShowNotification { msg } => state.set_status_message(msg),
   |      ...
   |    }
   |
8. Event loop continues, UI re-renders
```

### Todo Change Hook

```
1. User toggles todo state (x key)
   |
2. AppState::toggle_current_item_state()
   |  - Saves undo
   |  - Mutates state
   |  - Sets unsaved_changes = true
   |
3. After mutation, broadcast to plugins:
   |  let event = PluginEvent::TodoCompleted(todo.clone());
   |  let commands = plugin_manager.broadcast_event(event);
   |
4. Each plugin with on_todo_completed = true receives event
   |  - Plugin can return commands (e.g., sync to external service)
   |
5. process_commands() handles any returned commands
   |
6. Save and render
```

## Technology Comparison

| Approach | Pros | Cons | Recommendation |
|----------|------|------|----------------|
| **WASM (wasmtime)** | Sandboxed, portable, multi-language, stable ABI | Slightly more complex setup, some std features unavailable | **Recommended** |
| **abi_stable** | Native performance, familiar Rust | ABI fragile across versions, complex setup, no sandbox | Not recommended |
| **libloading** | Simple for C ABI | Very limited Rust types, manual FFI, security risk | Not recommended |
| **Script (Lua/Rhai)** | Easy embedding, simple interface | Limited capabilities, another language to learn | Consider for config only |

**WASM Runtimes:**
- **Wasmtime** (recommended): Best component model support, strong security focus, Bytecode Alliance backed
- **Wasmer**: Good for ahead-of-time compilation, WASIX extensions

## Build Order

Suggested phase structure based on dependencies:

### Phase 1: Plugin Infrastructure Foundation
- [ ] Create `src/plugin/command.rs` - PluginCommand enum
- [ ] Create `src/plugin/manifest.rs` - Manifest parsing
- [ ] Create `src/plugin/permissions.rs` - Permission model
- [ ] Extend `src/storage/database.rs` - plugin_metadata table

**Rationale:** These are data structures with no runtime dependencies. Can be tested in isolation.

### Phase 2: Host API Layer
- [ ] Create `src/plugin/host_api.rs` - PluginHostApi struct
- [ ] Create `src/plugin/snapshots.rs` - TodoItemSnapshot and filters
- [ ] Create `src/plugin/context.rs` - PluginContext for invocations

**Rationale:** Depends on Phase 1 structures. Still no WASM runtime needed for testing.

### Phase 3: Plugin Manager Core
- [ ] Create `src/plugin/manager.rs` - PluginManager skeleton
- [ ] Implement plugin discovery (scan ~/.config/to-tui/plugins/)
- [ ] Implement manifest loading and validation
- [ ] Add command processing logic

**Rationale:** Can be tested with mock plugins before WASM integration.

### Phase 4: WASM Runtime Integration
- [ ] Add wasmtime dependency to Cargo.toml
- [ ] Create `src/plugin/wasm_runtime.rs` - WASM loading and execution
- [ ] Define host functions exportable to WASM
- [ ] Implement Plugin trait for WasmPlugin

**Rationale:** Core infrastructure must be in place first.

### Phase 5: Keybinding Integration
- [ ] Extend `Action` enum with PluginAction variant
- [ ] Modify `KeybindingCache` to handle dynamic registrations
- [ ] Update `execute_navigate_action()` to dispatch to plugins
- [ ] Add plugin keybindings to help overlay

**Rationale:** Requires working plugin manager to test.

### Phase 6: Event Hooks
- [ ] Define PluginEvent enum
- [ ] Add broadcast points in AppState mutation methods
- [ ] Implement hook filtering based on manifest
- [ ] Add recursion depth limit for safety

**Rationale:** Requires all previous phases working.

### Phase 7: Migration & Polish
- [ ] Migrate existing JiraClaudeGenerator to WASM plugin
- [ ] Add plugin management commands (list, enable, disable)
- [ ] Create example plugin template
- [ ] Documentation

**Rationale:** Proves the system works end-to-end.

## Thread Safety

**Current architecture:**
- TUI runs on main thread (blocking event loop)
- Plugin execution via thread::spawn (see `handle_plugin_input`)
- Results returned via mpsc channel (`plugin_result_rx`)

**Considerations for dynamic plugins:**

1. **WASM is single-threaded:** WASM modules run in a single thread. Host can invoke on main thread (blocking) or spawn worker thread.

2. **Async operations:** For network/IO plugins, use the existing pattern:
   ```rust
   // Spawn plugin execution
   let (tx, rx) = mpsc::channel();
   thread::spawn(move || {
       let result = plugin.execute_async_action(...);
       tx.send(result).ok();
   });
   state.plugin_result_rx = Some(rx);

   // Poll in tick loop
   pub fn check_plugin_result(&mut self) { ... }
   ```

3. **HostApi thread safety:**
   - Read operations: Can use Arc<RwLock<HostState>> or just clone snapshots
   - Write operations: Return commands to main thread, don't mutate directly
   - **Never give plugins &mut AppState**

4. **Plugin isolation:**
   - Each plugin gets its own WASM instance
   - Plugins cannot directly communicate (prevent dependency hell)
   - Cross-plugin communication only through host events

## Security Considerations

1. **WASM sandboxing:** Plugins cannot access filesystem, network, or system calls unless explicitly granted via WASI capabilities

2. **Permission model:**
   ```rust
   pub struct PluginPermissions {
       pub network: bool,      // HTTP requests
       pub filesystem: bool,   // Read/write plugin directory
       pub execute: bool,      // Spawn processes (DANGEROUS)
   }
   ```

3. **Resource limits:**
   - Execution timeout (prevent infinite loops)
   - Memory limit per plugin
   - Rate limiting for host API calls

4. **User consent:** First-run prompt when loading new plugin with elevated permissions

## Existing Code to Modify

### Files Requiring Changes

| File | Change Type | Description |
|------|-------------|-------------|
| `src/plugin/mod.rs` | EXTEND | Re-export new modules, keep existing TodoGenerator |
| `src/app/state.rs` | EXTEND | Add plugin_manager field, integrate into lifecycle |
| `src/app/event.rs` | EXTEND | Handle PluginAction in execute_navigate_action |
| `src/keybindings/mod.rs` | EXTEND | Add PluginAction variant to Action enum |
| `src/storage/database.rs` | EXTEND | Add plugin_metadata table and CRUD operations |
| `src/config.rs` | EXTEND | Add plugin configuration options |
| `Cargo.toml` | EXTEND | Add wasmtime dependency |

### Files to Create

| File | Purpose |
|------|---------|
| `src/plugin/manager.rs` | PluginManager struct and lifecycle |
| `src/plugin/host_api.rs` | Host functions exposed to plugins |
| `src/plugin/command.rs` | PluginCommand enum |
| `src/plugin/manifest.rs` | Manifest parsing |
| `src/plugin/permissions.rs` | Permission model |
| `src/plugin/wasm_runtime.rs` | WASM loading and execution |
| `src/plugin/context.rs` | PluginContext for invocations |
| `src/plugin/snapshots.rs` | Immutable data snapshots |
| `src/plugin/events.rs` | PluginEvent enum and hooks |

## Sources

**Rust Plugin Systems:**
- [Plugins in Rust: Getting Started - NullDeref](https://nullderef.com/blog/plugin-start/)
- [Plugins in Rust: abi_stable - NullDeref](https://nullderef.com/blog/plugin-abi-stable/)
- [How to build a plugin system in Rust - Arroyo](https://www.arroyo.dev/blog/rust-plugin-systems/)
- [Plugins in Rust - Michael F Bryan](https://adventures.michaelfbryan.com/posts/plugins-in-rust/)
- [abi_stable crate documentation](https://docs.rs/abi_stable/)

**WASM Runtimes:**
- [Wasmtime - Official Site](https://wasmtime.dev/)
- [Wasmtime GitHub](https://github.com/bytecodealliance/wasmtime)
- [WASM Plugins with Rust Components](https://tartanllama.xyz/posts/wasm-plugins/)

**TUI Plugin Architectures:**
- [Zellij Plugin Development Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/)
- [Zellij Plugin API - DeepWiki](https://deepwiki.com/zellij-org/zellij/3.3-creating-plugins)
- [Helix Plugin System Discussion](https://github.com/helix-editor/helix/discussions/3806)

**WebAssembly 2025:**
- [WebAssembly in Rust 2025 Edition](https://medium.com/@mtolmacs/a-gentle-introduction-to-webassembly-in-rust-2025-edition-c1b676515c2d)
