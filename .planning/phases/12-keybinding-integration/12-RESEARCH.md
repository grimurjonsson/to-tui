# Phase 12 Research: Keybinding Integration

**Phase Goal:** Plugins can define custom actions triggered by keybindings

**Requirements:** KEYS-01 through KEYS-05

## Executive Summary

This research documents everything needed to plan Phase 12, which enables plugins to define custom actions and keybindings. The existing keybinding system is well-designed and provides clear extension points. The primary challenge is extending the manifest format, creating a plugin action registry, and integrating plugin keybindings into the existing key routing flow.

---

## 1. Existing Keybinding System Analysis

### 1.1 Core Data Structures

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/keybindings/mod.rs`

The keybinding system uses these core types:

```rust
// KeyBinding: A single key press with modifiers
pub struct KeyBinding {
    pub code: KeyCode,          // From crossterm
    pub modifiers: KeyModifiers, // CONTROL, ALT, SHIFT, SUPER
}

// KeySequence: 1-2 key presses (e.g., "dd" for delete)
pub struct KeySequence(pub Vec<KeyBinding>);

// Action: All bindable host actions (40+ variants)
pub enum Action {
    MoveUp, MoveDown, ToggleState, Delete, NewItem, ...
}

// KeybindingCache: Pre-compiled lookups by mode
pub struct KeybindingCache {
    navigate_single: HashMap<KeyBinding, Action>,
    navigate_sequences: HashMap<KeyBinding, HashMap<KeyBinding, Action>>,
    navigate_sequence_starters: HashSet<KeyBinding>,
    edit_single: HashMap<KeyBinding, Action>,
    visual_single: HashMap<KeyBinding, Action>,
}
```

### 1.2 Key Parsing Format

The key parsing is robust and supports:

- Simple keys: `"j"`, `"k"`, `"?"`, `"<"`, `">"`
- Bracket notation: `"<Space>"`, `"<Tab>"`, `"<Enter>"`, `"<Esc>"`
- Modifiers: `"<C-d>"` (Ctrl), `"<A-b>"` (Alt), `"<S-Tab>"` (Shift)
- Combined: `"<S-A-Up>"` (Shift+Alt+Up)
- Sequences: `"dd"`, `"g g"`, `"<C-d><C-d>"`

**Constraint:** Sequences are limited to 2 keys maximum.

### 1.3 Event Flow

```
KeyEvent received
    |
    v
handle_key_event() in src/app/event.rs
    |
    +-- Mode::Navigate -> handle_navigate_mode()
    |       |
    |       v
    |   keybindings.lookup_navigate(event, pending)
    |       |
    |       +-- KeyLookupResult::Action(action) -> execute_navigate_action()
    |       +-- KeyLookupResult::Pending -> wait for second key
    |       +-- KeyLookupResult::None -> key not bound
    |
    +-- Mode::Edit -> handle_edit_mode()
    |       |
    |       v
    |   keybindings.get_edit_action(event)
    |
    +-- Mode::Visual -> handle_visual_mode()
    |       |
    |       v
    |   keybindings.get_visual_action(event)
    |
    +-- (other modes handle keys directly without keybinding lookup)
```

### 1.4 Configuration Format

**Location:** `~/.config/to-tui/config.toml`

```toml
[keybindings.navigate]
"j" = "move_down"
"k" = "move_up"
"dd" = "delete"
"<C-p>" = "open_project_modal"

[keybindings.edit]
"<Esc>" = "edit_cancel"
"<Enter>" = "edit_confirm"

[keybindings.visual]
"v" = "exit_visual"
```

The config merges with defaults via `merge_with_defaults()`.

---

## 2. Plugin System Analysis

### 2.1 Plugin Manifest

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/plugin/manifest.rs`

Current manifest structure:

```rust
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub min_interface_version: Option<String>,
    // No actions or keybindings yet
}
```

### 2.2 Plugin Loading

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/plugin/loader.rs`

The `PluginLoader` loads dynamic libraries (.dylib/.so/.dll) and stores `LoadedPlugin` instances. Each loaded plugin has:

```rust
pub struct LoadedPlugin {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub plugin: Plugin_TO<'static, RBox<()>>,
    _library: KeepAlive,
}
```

### 2.3 Plugin Interface (FFI)

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/crates/totui-plugin-interface/src/plugin.rs`

The Plugin trait currently has:

```rust
#[sabi_trait]
pub trait Plugin: Send + Sync + Debug {
    fn name(&self) -> RString;
    fn version(&self) -> RString;
    fn min_interface_version(&self) -> RString;
    fn generate(&self, input: RString) -> RResult<RVec<FfiTodoItem>, RString>;
    fn config_schema(&self) -> FfiConfigSchema;
    fn execute_with_host(&self, input: RString, host: HostApi_TO<...>) -> RResult<RVec<FfiCommand>, RString>;
    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>);
}
```

### 2.4 Plugin Manager

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/plugin/manager.rs`

`PluginManager` discovers plugins from `~/.local/share/to-tui/plugins/` and tracks:

```rust
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub enabled: bool,
    pub available: bool,
    pub availability_reason: Option<String>,
    pub error: Option<String>,
}
```

---

## 3. UI Integration Points

### 3.1 Help Panel

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/ui/components/mod.rs`

The help panel (`render_help_overlay`) is a scrollable list with sections:
- Navigation
- Item State
- Item Management
- Indentation
- Move Items
- Priority
- Visual Mode
- Day Navigation
- Other
- Edit Mode

**Context Decision:** Plugin actions will appear in a new "Plugin Actions" section at the bottom, grouped by plugin name.

### 3.2 Status Bar

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/ui/components/status_bar.rs`

The status bar has a message display mechanism:

```rust
// In AppState:
pub status_message: Option<(String, Instant)>,

// Messages auto-clear after 3 seconds
pub fn set_status_message(&mut self, message: String) {
    self.status_message = Some((message, Instant::now()));
}
```

**Context Decision:** Plugin actions will use status_message for feedback (e.g., "Fetching JIRA-123...").

### 3.3 Error Popup

**Location:** `/Users/gimmi/Documents/Sources/rust/to-tui/src/ui/components/mod.rs` (`render_plugin_error_popup`)

The existing plugin error popup can be reused for action errors.

### 3.4 Spinner

The spinner mechanism exists in AppState:

```rust
pub spinner_frame: usize,

pub fn tick_spinner(&mut self) {
    self.spinner_frame = (self.spinner_frame + 1) % 8;
}

pub fn get_spinner_char(&self) -> char {
    const SPINNER_FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
    SPINNER_FRAMES[self.spinner_frame]
}
```

---

## 4. Design Decisions from Context

From `12-CONTEXT.md`:

### 4.1 Action Discovery
- Plugin actions appear in help panel at bottom in "Plugin Actions" section
- Grouped by plugin with per-plugin sub-groupings
- Every action MUST have a description (validation fails without)
- Disabled plugins are hidden entirely from help

### 4.2 Conflict Resolution
- Host keybinding conflicts: host wins + startup warning
- Plugin-to-plugin conflicts: first loaded wins + warning
- Users can unbind host keybindings to give them to plugins
- No reserved keys - everything is rebindable

### 4.3 Invocation Feedback
- Status bar shows custom message while action runs
- Errors displayed in existing error popup
- Plugin actions block UI but show spinner
- Success shows brief status bar message (2-3 seconds)

### 4.4 Config Override Format

```toml
[keybindings.plugins.jira]
fetch = "Ctrl+j"

[keybindings.plugins.github]
sync = "<C-g>"
```

- Can disable actions: `fetch = "none"` or `fetch = ""`

### 4.5 Namespace Format
Internal routing uses: `plugin:name:action`

---

## 5. Implementation Architecture

### 5.1 Manifest Extension

Add `[actions]` section to `plugin.toml`:

```toml
name = "jira"
version = "1.0.0"
description = "Jira integration plugin"

[actions.fetch]
description = "Fetch and create todos from a Jira ticket"
default_keybinding = "<C-j>"

[actions.sync]
description = "Sync all Jira-linked todos"
default_keybinding = "<A-j>"
```

**Manifest Struct Changes:**

```rust
pub struct PluginManifest {
    // ... existing fields ...

    /// Actions this plugin provides with their metadata
    #[serde(default)]
    pub actions: HashMap<String, ActionDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDefinition {
    /// Human-readable description (required for help panel)
    pub description: String,

    /// Default keybinding in bracket notation (optional)
    #[serde(default)]
    pub default_keybinding: Option<String>,
}
```

### 5.2 Action Registry

New struct to manage plugin actions at runtime:

```rust
/// Registered plugin action with resolved keybinding
pub struct PluginAction {
    pub plugin_name: String,
    pub action_name: String,
    pub description: String,
    pub keybinding: Option<KeySequence>, // After conflict resolution
    pub namespace: String,  // "plugin:jira:fetch"
}

pub struct PluginActionRegistry {
    /// All registered actions
    actions: Vec<PluginAction>,

    /// Lookup: keybinding -> action index
    keybinding_map: HashMap<KeyBinding, usize>,

    /// Lookup: namespace -> action index
    namespace_map: HashMap<String, usize>,

    /// Warnings generated during registration
    warnings: Vec<String>,
}

impl PluginActionRegistry {
    pub fn new() -> Self;

    /// Register actions from a plugin manifest
    /// Handles conflict detection and resolution
    pub fn register_plugin(&mut self,
        manifest: &PluginManifest,
        overrides: &PluginKeybindingOverrides,
        host_keybindings: &KeybindingCache,
    ) -> Vec<String>; // Returns warnings

    /// Lookup action by keybinding
    pub fn lookup(&self, binding: &KeyBinding) -> Option<&PluginAction>;

    /// Get all actions for help display (grouped by plugin)
    pub fn actions_by_plugin(&self) -> HashMap<String, Vec<&PluginAction>>;
}
```

### 5.3 Config Extension

Add to existing config:

```rust
pub struct Config {
    // ... existing fields ...

    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

pub struct KeybindingsConfig {
    #[serde(default)]
    pub navigate: HashMap<String, String>,

    #[serde(default)]
    pub edit: HashMap<String, String>,

    #[serde(default)]
    pub visual: HashMap<String, String>,

    // NEW: Plugin keybinding overrides
    #[serde(default)]
    pub plugins: HashMap<String, HashMap<String, String>>,
    // plugins.jira.fetch = "<C-j>"
}
```

### 5.4 Key Routing Integration

Modify `handle_navigate_mode()` to check plugin actions after host:

```rust
fn handle_navigate_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    // ... existing pending key handling ...

    match state.keybindings.lookup_navigate(&key, pending) {
        KeyLookupResult::Action(action) => {
            execute_navigate_action(action, state)?;
        }
        KeyLookupResult::Pending => {
            // ... existing pending logic ...
        }
        KeyLookupResult::None => {
            // NEW: Check plugin actions
            let binding = KeyBinding::from_event(&key);
            if let Some(plugin_action) = state.plugin_action_registry.lookup(&binding) {
                execute_plugin_action(plugin_action, state)?;
            }
        }
    }
    // ...
}
```

### 5.5 Plugin Action Execution

```rust
fn execute_plugin_action(action: &PluginAction, state: &mut AppState) -> Result<()> {
    // Show status message while running
    state.set_status_message(format!("Running {}...", action.action_name));

    // Find the loaded plugin
    let plugin = state.plugin_loader
        .loaded_plugins()
        .find(|p| p.name == action.plugin_name)
        .ok_or_else(|| anyhow::anyhow!("Plugin not loaded"))?;

    // Create host API
    let host_api = PluginHostApiImpl::new(...);
    let host_to = HostApi_TO::from_value(host_api, TD_Opaque);

    // Execute plugin action (blocking with spinner)
    // The plugin's execute_with_host receives action name as input
    let result = call_plugin_execute_with_host(
        &plugin.plugin,
        action.action_name.into(),
        host_to
    );

    match result.into_result() {
        Ok(commands) => {
            if !commands.is_empty() {
                state.save_undo();
                let mut executor = CommandExecutor::new(action.plugin_name.clone());
                executor.execute_batch(commands.into_iter().collect(), &mut state.todo_list)?;
                state.unsaved_changes = true;
            }
            state.set_status_message(format!("{} complete", action.action_name));
        }
        Err(e) => {
            // Show error in popup
            state.pending_plugin_errors.push(PluginLoadError {
                plugin_name: action.plugin_name.clone(),
                error_kind: PluginErrorKind::Other(e.to_string()),
                message: e.to_string(),
            });
            state.show_plugin_error_popup = true;
        }
    }

    Ok(())
}
```

### 5.6 Help Panel Extension

Add plugin actions section to `render_help_overlay()`:

```rust
// After existing sections...

// Plugin Actions section (only if any plugins have actions)
let actions_by_plugin = state.plugin_action_registry.actions_by_plugin();
if !actions_by_plugin.is_empty() {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  ── Plugin Actions ──", section_style)));

    for (plugin_name, actions) in actions_by_plugin.iter().sorted_by_key(|(k, _)| *k) {
        lines.push(Line::from(vec![
            Span::styled(format!("  [{}]", plugin_name),
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        ]));

        for action in actions {
            let key_text = action.keybinding
                .as_ref()
                .map(|k| format!("{:16}", k))
                .unwrap_or_else(|| "(no binding)    ".to_string());

            lines.push(Line::from(vec![
                Span::styled(format!("    {}  ", key_text), key_style),
                Span::styled(&action.description, desc_style),
            ]));
        }
    }
}
```

---

## 6. FFI Considerations

### 6.1 No Plugin Trait Changes Needed

Actions are defined in the manifest (static data), not in the Plugin trait. The plugin receives the action name as input to `execute_with_host()`.

This means:
- No FFI changes required
- No interface version bump
- Existing plugins continue to work
- Plugins distinguish actions by checking the input parameter

### 6.2 Plugin Implementation Pattern

Plugins handle actions in `execute_with_host`:

```rust
fn execute_with_host(&self, input: RString, host: HostApi_TO<...>) -> RResult<RVec<FfiCommand>, RString> {
    let action = input.as_str();

    match action {
        "fetch" => self.handle_fetch(host),
        "sync" => self.handle_sync(host),
        _ => RResult::RErr(format!("Unknown action: {}", action).into()),
    }
}
```

---

## 7. Validation Requirements

### 7.1 Manifest Validation

Add to `PluginManifest::validate()`:

1. Each action must have a non-empty description
2. If `default_keybinding` is present, it must parse as valid `KeySequence`
3. Action names must be valid identifiers (alphanumeric + underscore)

### 7.2 Startup Validation

During plugin loading:

1. Check for keybinding conflicts (host wins, first plugin wins)
2. Generate warnings for conflicts
3. Store warnings for display via `totui plugin status`

---

## 8. Testing Strategy

### 8.1 Unit Tests

1. **Manifest parsing**: Actions section parses correctly
2. **Key sequence validation**: Invalid keybindings rejected
3. **Conflict detection**: Host vs plugin, plugin vs plugin
4. **Registry operations**: Register, lookup, list by plugin

### 8.2 Integration Tests

1. **End-to-end**: Key press triggers plugin action
2. **Override**: User config overrides default binding
3. **Disable**: Setting binding to "none" disables action
4. **Help panel**: Actions appear correctly

---

## 9. Implementation Order

### Plan 12-01: Action Registration and Manifest Keybindings

1. Extend `PluginManifest` with actions field
2. Create `ActionDefinition` struct
3. Add validation for actions in manifest
4. Create `PluginActionRegistry` struct
5. Implement registration with conflict detection
6. Extend help panel to show plugin actions
7. Unit tests for manifest and registry

### Plan 12-02: Key Routing and User Override Support

1. Extend `KeybindingsConfig` with plugins section
2. Parse user overrides for plugin keybindings
3. Integrate override resolution into registry
4. Modify `handle_navigate_mode()` for plugin action lookup
5. Implement `execute_plugin_action()` with status/spinner
6. Add startup warnings for conflicts
7. Integration tests for full flow

---

## 10. Risk Assessment

### 10.1 Low Risk
- Manifest extension (additive, backward compatible)
- Config extension (additive)
- Help panel addition (isolated change)

### 10.2 Medium Risk
- Key routing changes (core event handling, needs careful testing)
- Conflict resolution (edge cases with key sequences)

### 10.3 Mitigations
- Extensive unit tests for key parsing and conflict detection
- Integration tests for common scenarios
- Warnings (not errors) for conflicts to avoid breaking startup

---

## 11. Open Questions

All questions answered in 12-CONTEXT.md. Claude has discretion for:
- Exact spinner implementation (reuse existing)
- Warning message formatting
- Default key assignment strategy for new plugins
- Validation error message wording

---

## 12. File Inventory

Files to modify:
- `src/plugin/manifest.rs` - Add actions field
- `src/plugin/mod.rs` - Export new types
- `src/keybindings/mod.rs` - Add `PluginAction` validation helpers
- `src/config.rs` - Add plugins section to keybindings
- `src/app/state.rs` - Add `plugin_action_registry` field
- `src/app/event.rs` - Modify `handle_navigate_mode()`
- `src/ui/components/mod.rs` - Extend help overlay

New files:
- `src/plugin/actions.rs` - `PluginActionRegistry` and related types

---

*Research completed: 2026-01-26*
*Ready for planning phase*
