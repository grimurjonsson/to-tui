# Phase 13: Event Hooks - Research

**Researched:** 2026-01-26
**Domain:** FFI-safe async event hooks for plugin lifecycle events
**Confidence:** HIGH

## Summary

This phase enables plugins to respond to todo lifecycle events (add, modify, complete, delete) asynchronously. The research reveals that the existing codebase has well-established patterns for background tasks using `std::thread::spawn` with `mpsc` channels (see version_check.rs, upgrade.rs), plugin execution with panic-safe FFI wrappers, and command-based mutations that preserve undo/redo.

The key architectural challenge is making hooks non-blocking while allowing them to return modifications. The solution is a dedicated background thread per event that calls the plugin hook, with modifications returned via channels and applied on the next UI loop iteration with a silent refresh.

**Primary recommendation:** Use `std::thread::spawn` with `mpsc` channels (not tokio) for hook execution, since the TUI event loop is synchronous and already polls channels this way. Add new hook methods to the Plugin trait with `#[sabi(last_prefix_field)]` for ABI extensibility.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| abi_stable | 0.11 | Stable ABI for FFI hook methods | Already in use, sabi_trait for Plugin trait |
| std::thread | stdlib | Spawn background hook threads | Already used in version_check.rs, upgrade.rs |
| std::sync::mpsc | stdlib | Channel communication | Already used throughout TUI event loop |
| uuid | 1.11 | Todo identification | Already in use for todo IDs |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| RVec | abi_stable | FFI-safe command vectors | Hook return values |
| ROption | abi_stable | FFI-safe optional values | Optional hook registration |
| catch_unwind | std::panic | Panic-safe hook calls | All hook invocations |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| std::thread + mpsc | tokio async | TUI loop is sync; std::thread simpler, already used |
| Per-event thread spawn | Thread pool | Simpler, events are infrequent, no warm pool needed |
| Channel-based return | Direct mutation | Channel preserves UI responsiveness, allows timeout |
| Sequential plugin execution | Parallel execution | Sequential is predictable, prevents race conditions |

**Installation:** No new dependencies needed - uses existing infrastructure.

## Architecture Patterns

### Recommended Module Structure

```
crates/totui-plugin-interface/src/
    plugin.rs           # Extended Plugin trait with hook methods
    host_api.rs         # FfiEvent enum, FfiHookResponse type
src/plugin/
    hooks.rs            # NEW: HookDispatcher, HookResult handling
    loader.rs           # Extended for hook subscription tracking
src/app/
    state.rs            # hook_result_rx channel, apply_hook_results()
src/ui/
    mod.rs              # Poll hook_result_rx in event loop
```

### Pattern 1: Event Enum for Hook Invocation

**What:** FFI-safe enum representing lifecycle events with todo context
**When to use:** All hook dispatches - provides typed event data to plugins

**Design:**
```rust
// In totui-plugin-interface crate

/// FFI-safe event type for hooks
#[repr(C)]
#[derive(StableAbi, Clone)]
pub enum FfiEvent {
    /// Todo was added (manual, rollover, or plugin-generated)
    OnAdd {
        todo: FfiTodoItem,
        source: FfiEventSource,
    },
    /// Todo was modified (any property change)
    OnModify {
        todo: FfiTodoItem,  // Current (after) state
        field_changed: FfiFieldChange,
    },
    /// Todo was completed (state changed to Checked)
    OnComplete {
        todo: FfiTodoItem,
    },
    /// Todo was soft-deleted
    OnDelete {
        todo: FfiTodoItem,
    },
    /// Application startup (opt-in)
    OnLoad {
        project_name: RString,
        date: RString,  // YYYY-MM-DD
    },
}

/// Source of the add event
#[repr(u8)]
#[derive(StableAbi, Clone, Copy)]
pub enum FfiEventSource {
    Manual = 0,      // User created
    Rollover = 1,    // Daily rollover
    Plugin = 2,      // Plugin-generated
    Api = 3,         // REST API
}

/// Which field changed (for OnModify)
#[repr(u8)]
#[derive(StableAbi, Clone, Copy)]
pub enum FfiFieldChange {
    Content = 0,
    State = 1,
    DueDate = 2,
    Priority = 3,
    Description = 4,
    Indent = 5,
    Parent = 6,
    Multiple = 7,    // Batch update
}
```

### Pattern 2: Hook Response with Optional Modifications

**What:** Return type allowing hooks to modify the triggering todo
**When to use:** All hook returns - enables reactive modifications

**Design:**
```rust
/// FFI-safe hook response
#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct FfiHookResponse {
    /// Commands to apply (may be empty)
    pub commands: RVec<FfiCommand>,
}

impl Default for FfiHookResponse {
    fn default() -> Self {
        Self { commands: RVec::new() }
    }
}

/// Plugin trait extension for hooks
#[sabi_trait]
pub trait Plugin: Send + Sync + Debug {
    // ... existing methods ...

    /// Return event types this plugin wants to receive.
    /// Empty vec means plugin subscribes to no events.
    fn subscribed_events(&self) -> RVec<FfiEventType>;

    /// Handle an event hook.
    /// Called asynchronously - should not block for long.
    /// Returns commands to apply (or empty vec for no-op).
    #[sabi(last_prefix_field)]
    fn on_event(&self, event: FfiEvent) -> RResult<FfiHookResponse, RString>;
}

/// Event type enum for subscription
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, PartialEq, Eq)]
pub enum FfiEventType {
    OnAdd = 0,
    OnModify = 1,
    OnComplete = 2,
    OnDelete = 3,
    OnLoad = 4,
}
```

### Pattern 3: Background Thread Hook Dispatch

**What:** Fire-and-forget async execution with channel-based result return
**When to use:** All hook dispatches - keeps UI responsive

**Design:**
```rust
// src/plugin/hooks.rs

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Result of a hook execution
pub struct HookResult {
    pub plugin_name: String,
    pub event_type: FfiEventType,
    pub commands: Vec<FfiCommand>,
    pub error: Option<String>,
}

/// Dispatches events to subscribed plugins asynchronously
pub struct HookDispatcher {
    /// Channel to receive completed hook results
    result_rx: mpsc::Receiver<HookResult>,
    /// Sender cloned for each hook thread
    result_tx: mpsc::Sender<HookResult>,
    /// Consecutive failure count per plugin (for auto-disable)
    failure_counts: HashMap<String, u32>,
    /// Session-disabled plugins (from failures)
    disabled_plugins: HashSet<String>,
    /// Default timeout for hooks (5 seconds)
    default_timeout: Duration,
    /// Auto-disable threshold (3 consecutive failures)
    auto_disable_threshold: u32,
}

impl HookDispatcher {
    pub fn new() -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            result_rx,
            result_tx,
            failure_counts: HashMap::new(),
            disabled_plugins: HashSet::new(),
            default_timeout: Duration::from_secs(5),
            auto_disable_threshold: 3,
        }
    }

    /// Dispatch an event to all subscribed plugins.
    /// Non-blocking - hooks run in background threads.
    ///
    /// # Arguments
    /// * `event` - The event to dispatch
    /// * `subscribed_plugins` - List of (plugin_name, timeout) for plugins that want this event
    /// * `plugin_loader` - Reference to get plugin trait objects
    pub fn dispatch(
        &self,
        event: FfiEvent,
        subscribed_plugins: Vec<(String, Duration)>,
        plugin_loader: &PluginLoader,
    ) {
        for (plugin_name, timeout) in subscribed_plugins {
            // Skip session-disabled plugins
            if self.disabled_plugins.contains(&plugin_name) {
                continue;
            }

            // Get plugin reference (clone event for each thread)
            let event_clone = event.clone();
            let tx = self.result_tx.clone();
            let name = plugin_name.clone();
            let event_type = event_to_type(&event);

            // Get plugin - must be done on main thread
            let Some(loaded) = plugin_loader.get(&plugin_name) else {
                continue;
            };

            // Clone plugin reference for thread
            let plugin = loaded.plugin.clone();

            thread::spawn(move || {
                let start = Instant::now();

                // Call hook with timeout
                let result = call_hook_with_timeout(&plugin, event_clone, timeout);

                let hook_result = match result {
                    Ok(response) => HookResult {
                        plugin_name: name,
                        event_type,
                        commands: response.commands.into_iter().collect(),
                        error: None,
                    },
                    Err(e) => HookResult {
                        plugin_name: name,
                        event_type,
                        commands: vec![],
                        error: Some(e),
                    },
                };

                let _ = tx.send(hook_result);
            });
        }
    }

    /// Poll for completed hook results (non-blocking).
    /// Call this from the UI event loop.
    pub fn poll_results(&mut self) -> Vec<HookResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            // Track failures for auto-disable
            if result.error.is_some() {
                let count = self.failure_counts
                    .entry(result.plugin_name.clone())
                    .or_insert(0);
                *count += 1;

                if *count >= self.auto_disable_threshold {
                    self.disabled_plugins.insert(result.plugin_name.clone());
                    tracing::warn!(
                        plugin = %result.plugin_name,
                        "Plugin hook auto-disabled after {} consecutive failures",
                        self.auto_disable_threshold
                    );
                }
            } else {
                // Reset failure count on success
                self.failure_counts.remove(&result.plugin_name);
            }

            results.push(result);
        }
        results
    }
}
```

### Pattern 4: Timeout and Panic Safety

**What:** Wrap hook calls with catch_unwind and timeout
**When to use:** Every hook invocation

**Design:**
```rust
fn call_hook_with_timeout(
    plugin: &Plugin_TO<'_, RBox<()>>,
    event: FfiEvent,
    timeout: Duration,
) -> Result<FfiHookResponse, String> {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();
    let event_clone = event.clone();

    // Spawn a thread for the actual call (allows timeout)
    let plugin_clone = plugin.clone();
    thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            plugin_clone.on_event(event_clone)
        }));

        let _ = tx.send(result);
    });

    // Wait with timeout
    match rx.recv_timeout(timeout) {
        Ok(Ok(result)) => {
            result.into_result().map_err(|e| e.to_string())
        }
        Ok(Err(panic_info)) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                format!("Hook panicked: {}", s)
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                format!("Hook panicked: {}", s)
            } else {
                "Hook panicked with unknown error".to_string()
            };
            Err(msg)
        }
        Err(_) => Err("Hook timed out".to_string()),
    }
}
```

### Pattern 5: Cascade Prevention

**What:** Track "in-hook" state to prevent hook-triggered modifications from firing new events
**When to use:** All event firing points

**Design:**
```rust
// In AppState or a shared context
pub struct EventContext {
    /// True when applying hook-returned commands
    in_hook_apply: bool,
}

// When firing events:
fn fire_event_if_not_cascade(
    dispatcher: &HookDispatcher,
    event: FfiEvent,
    context: &EventContext,
    // ...
) {
    // Don't fire events for changes made by hooks
    if context.in_hook_apply {
        return;
    }

    dispatcher.dispatch(event, /* ... */);
}

// When applying hook results:
fn apply_hook_commands(
    commands: Vec<FfiCommand>,
    todo_list: &mut TodoList,
    context: &mut EventContext,
) -> Result<()> {
    context.in_hook_apply = true;

    // Execute commands (no undo snapshot - hooks are secondary effects)
    let mut executor = CommandExecutor::new(/* ... */);
    executor.execute_batch(commands, todo_list)?;

    context.in_hook_apply = false;
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Blocking hook calls in UI thread:** Never call hooks synchronously. Always dispatch to background.
- **Cascade chains:** Hook modifications must NOT fire new events. Track in-hook state.
- **Unbounded hook execution time:** Always use timeouts. Default 5 seconds.
- **Shared mutable state in hooks:** Hooks receive immutable todo snapshot. Return commands only.
- **Direct todo mutation in hooks:** Hooks return FfiCommand; host applies them.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async execution | tokio in TUI | std::thread + mpsc | TUI is sync, existing pattern works |
| Timeout handling | Manual timer threads | recv_timeout | Standard library solution |
| Panic safety | Uncaught panics | catch_unwind wrapper | Existing call_plugin_* pattern |
| Event subscription | Runtime queries | subscribed_events() method | Single call at load time |
| Hook result aggregation | Complex state machine | Simple channel polling | Existing pattern in run_app |

**Key insight:** The existing codebase uses `std::thread::spawn` + `mpsc::channel` for all background work (version check, downloads). Event hooks should follow the same pattern, not introduce tokio to the TUI.

## Common Pitfalls

### Pitfall 1: Infinite Event Cascade

**What goes wrong:** Hook modifies todo, fires new event, triggers same hook, infinite loop
**Why it happens:** Not tracking whether current change is hook-originated
**How to avoid:** `in_hook_apply` flag prevents event firing during hook command application
**Warning signs:** Stack overflow, UI freeze, runaway CPU usage

### Pitfall 2: UI Responsiveness Loss

**What goes wrong:** Long-running hooks block UI, spinner stuck
**Why it happens:** Synchronous hook calls on main thread
**How to avoid:** All hooks run in spawned threads, main thread only polls results
**Warning signs:** UI freeze during hook execution, slow key response

### Pitfall 3: Race Conditions in Hook Results

**What goes wrong:** Multiple hooks modify same todo, unexpected final state
**Why it happens:** Parallel hook execution with overlapping modifications
**How to avoid:** Sequential execution in plugin load order; commands applied atomically
**Warning signs:** Flaky test results, inconsistent todo states

### Pitfall 4: Memory Leaks from Orphaned Threads

**What goes wrong:** Hook thread blocks forever, never terminates
**Why it happens:** Plugin hook hangs, no timeout enforcement
**How to avoid:** All hook calls use recv_timeout; thread terminates regardless
**Warning signs:** Growing thread count, memory growth over time

### Pitfall 5: FFI Boundary Panic Propagation

**What goes wrong:** Plugin panic crashes host application
**Why it happens:** Uncaught panic crosses FFI boundary
**How to avoid:** All hook calls wrapped in catch_unwind; existing pattern in loader.rs
**Warning signs:** Segfault or abort during plugin hook

## Code Examples

### Event Firing Points (where to emit events)

```rust
// src/app/state.rs - After toggle_current_item_state()
pub fn toggle_current_item_state(&mut self) -> bool {
    // ... existing code ...

    // Fire event if not in cascade
    if !self.event_context.in_hook_apply {
        let item = &self.todo_list.items[self.cursor_position];
        let event = if item.state.is_complete() {
            FfiEvent::OnComplete { todo: item.into() }
        } else {
            FfiEvent::OnModify {
                todo: item.into(),
                field_changed: FfiFieldChange::State,
            }
        };
        self.fire_event(event);
    }

    true
}

// Helper method
fn fire_event(&self, event: FfiEvent) {
    let subscribed = self.get_subscribed_plugins(&event);
    self.hook_dispatcher.dispatch(event, subscribed, &self.plugin_loader);
}
```

### UI Event Loop Integration

```rust
// src/ui/mod.rs - In run_app()
fn run_app(...) -> Result<()> {
    loop {
        state.clear_expired_status_message();
        state.check_plugin_result();
        state.check_version_update();
        state.tick_spinner();
        state.check_download_progress();

        // NEW: Poll hook results and apply commands
        state.apply_pending_hook_results()?;

        terminal.draw(|f| {
            components::render(f, state);
        })?;

        // ... rest of loop ...
    }
}
```

### Applying Hook Results

```rust
// src/app/state.rs
impl AppState {
    pub fn apply_pending_hook_results(&mut self) -> Result<()> {
        let results = self.hook_dispatcher.poll_results();

        for result in results {
            if let Some(error) = result.error {
                // Show error popup (reuse existing infrastructure)
                self.pending_plugin_errors.push(PluginLoadError {
                    plugin_name: result.plugin_name,
                    error_kind: PluginErrorKind::Panicked { message: error.clone() },
                    message: error,
                });
                self.show_plugin_error_popup = true;
                continue;
            }

            if result.commands.is_empty() {
                continue;
            }

            // Apply commands without undo (hooks are secondary effects)
            // and without firing new events (cascade prevention)
            self.event_context.in_hook_apply = true;

            let mut executor = CommandExecutor::new(result.plugin_name);
            if let Err(e) = executor.execute_batch(result.commands, &mut self.todo_list) {
                tracing::warn!("Hook command failed: {}", e);
            } else {
                // Silent refresh - mark as changed but no notification
                self.unsaved_changes = true;
            }

            self.event_context.in_hook_apply = false;
        }

        Ok(())
    }
}
```

### Manifest Extension for Timeout

```rust
// src/plugin/manifest.rs
pub struct PluginManifest {
    // ... existing fields ...

    /// Timeout for hook execution in seconds (default: 5)
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_secs: u64,
}

fn default_hook_timeout() -> u64 {
    5
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Sync callbacks | Async background hooks | Phase 13 | Non-blocking UI |
| Direct mutation | Command pattern | Phase 9 | Preserves undo, enables cascade prevention |
| No lifecycle events | Full event system | Phase 13 | Enables reactive plugins |

**Deprecated/outdated:**
- Synchronous plugin hooks: Would block UI
- Direct todo mutation in hooks: Breaks undo/redo and cascade prevention

## Open Questions

### 1. Hook Timeout Default Value

**What we know:** CONTEXT.md says timeout is configurable per plugin in manifest
**What's unclear:** Best default value
**Recommendation:** 5 seconds - long enough for network operations, short enough to avoid perceived hang
**Confidence:** MEDIUM - may need tuning based on real plugin usage

### 2. Auto-Disable Threshold

**What we know:** CONTEXT.md says auto-disable after N consecutive failures
**What's unclear:** Exact N value
**Recommendation:** 3 consecutive failures - catches consistently broken hooks without being too aggressive
**Confidence:** MEDIUM - reasonable starting point, can be adjusted

### 3. On-Load Event Implementation

**What we know:** CONTEXT.md says on-load is optional, plugins can subscribe if needed
**What's unclear:** Exact timing (before or after todo list load?)
**Recommendation:** Fire after todo list is loaded but before first render, so hooks can modify initial state
**Confidence:** HIGH - logical sequence

### 4. Hook Registration Mechanism

**What we know:** CONTEXT.md gives discretion on manifest vs trait method
**Recommendation:** Use `subscribed_events()` trait method rather than manifest. Allows dynamic subscription based on config, and keeps event subscription close to event handling code.
**Confidence:** HIGH - trait method is more flexible and type-safe

## Sources

### Primary (HIGH confidence)

- Codebase analysis: `src/utils/version_check.rs` - Existing thread + channel pattern
- Codebase analysis: `src/utils/upgrade.rs` - Existing download with progress pattern
- Codebase analysis: `src/ui/mod.rs` - Existing event loop with channel polling
- Codebase analysis: `src/plugin/loader.rs` - Existing panic-safe call_safely pattern
- Codebase analysis: `crates/totui-plugin-interface/src/plugin.rs` - Existing Plugin trait

### Secondary (MEDIUM confidence)

- [abi_stable documentation](https://docs.rs/abi_stable/latest/abi_stable/) - sabi_trait patterns
- Context7 tokio docs - Timeout and channel patterns (referenced for comparison)

### Tertiary (LOW confidence)

- General event-driven architecture patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Uses only existing patterns and dependencies
- Architecture: HIGH - Follows established codebase patterns (thread+channel)
- Pitfalls: HIGH - Based on direct codebase analysis and common async pitfalls

**Research date:** 2026-01-26
**Valid until:** 2026-02-26 (stable domain, internal patterns)
