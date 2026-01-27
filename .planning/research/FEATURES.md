# Features Research: Plugin System

**Domain:** TUI application plugin system for todo management
**Researched:** 2026-01-24
**Confidence:** MEDIUM (based on established patterns from Neovim, Zellij, Taskwarrior, WezTerm)

## Table Stakes

Features users expect from any plugin system. Missing these means the system feels incomplete or unusable.

### Plugin Lifecycle

- **Plugin Registration**: Ability to register plugins with the host application
  - Complexity: LOW
  - Depends on: None
  - Notes: Current `PluginRegistry` already does this for generators

- **Plugin Discovery**: Auto-discover plugins from a known location
  - Complexity: LOW
  - Depends on: Plugin Registration
  - Notes: Follow pattern of `~/.config/to-tui/plugins/` or similar

- **Plugin Enable/Disable**: Toggle plugins on/off without uninstalling
  - Complexity: LOW
  - Depends on: Plugin Registration
  - Notes: Config-based enable flag

- **Plugin Availability Check**: Report why a plugin cannot run (missing dependencies)
  - Complexity: LOW
  - Depends on: None
  - Notes: Current `check_available()` trait method handles this

### Todo Manipulation

- **Create Todos**: Plugins can create new todo items
  - Complexity: LOW
  - Depends on: TodoList access
  - Notes: Current generators already do this via return values

- **Read Todos**: Plugins can query the current todo list
  - Complexity: LOW
  - Depends on: TodoList access
  - Notes: Essential for any plugin that processes existing todos

- **Update Todos**: Plugins can modify existing todo items
  - Complexity: MEDIUM
  - Depends on: Todo identification (UUID)
  - Notes: Need safe mutation API that respects undo stack

- **Delete Todos**: Plugins can remove todo items (soft delete)
  - Complexity: MEDIUM
  - Depends on: Todo identification (UUID)
  - Notes: Must integrate with undo system; prefer soft delete

### Plugin Input/Output

- **Receive Input from User**: Plugin can prompt for input
  - Complexity: LOW
  - Depends on: TUI mode system
  - Notes: Current `PluginSubState::InputPrompt` handles this

- **Display Status Messages**: Plugin can show feedback to user
  - Complexity: LOW
  - Depends on: Status message system
  - Notes: Current `set_status_message()` available

- **Show Plugin Results Preview**: User can review before accepting changes
  - Complexity: MEDIUM
  - Depends on: TUI modal system
  - Notes: Current `PluginSubState::Preview` handles this

### Error Handling

- **Graceful Error Reporting**: Plugin errors don't crash the app
  - Complexity: MEDIUM
  - Depends on: Result handling
  - Notes: Current system handles this via `PluginSubState::Error`

- **Plugin Timeout Handling**: Long-running plugins don't freeze UI
  - Complexity: MEDIUM
  - Depends on: Async execution
  - Notes: Need timeout mechanism for subprocess plugins


## Differentiators

Features that would make this plugin system stand out. Not expected, but add significant value.

### Data Access

- **Database Read-Only Access**: Plugins can query historical todos, archived data
  - Complexity: MEDIUM
  - Value: Enables analytics plugins, reporting, cross-day operations
  - Depends on: Database module
  - Notes: Read-only is safer than read-write; use prepared queries

- **Project-Aware Operations**: Plugins can work with project boundaries
  - Complexity: MEDIUM
  - Value: Enables project-specific plugins (e.g., Jira per project)
  - Depends on: Project system
  - Notes: Plugin should know current project context

- **Custom Metadata on Todos**: Plugins can attach key-value data to todos
  - Complexity: HIGH
  - Value: Enables rich plugins (time tracking, tags, links)
  - Depends on: Database schema extension
  - Notes: Consider JSON blob column for flexibility

- **Custom Metadata on Projects**: Plugins can attach data to projects
  - Complexity: HIGH
  - Value: Plugin-specific project settings (API keys, URLs)
  - Depends on: Project system, Database schema
  - Notes: Could enable per-project Jira configuration

### Keybinding Integration

- **Register Custom Keybindings**: Plugin can add new key mappings
  - Complexity: MEDIUM
  - Value: Native feel for plugin actions
  - Depends on: Keybinding system (Action enum, KeybindingCache)
  - Notes: Must avoid conflicts with built-in bindings; need namespace

- **Register Custom Commands**: Plugin can add slash-commands or similar
  - Complexity: MEDIUM
  - Value: Discoverable plugin actions
  - Depends on: Command system (would need to build)
  - Notes: Alternative to keybindings; more discoverable

### Advanced Lifecycle

- **Hot Reload**: Plugins can be updated without restarting app
  - Complexity: HIGH
  - Value: Better DX for plugin development
  - Depends on: Dynamic loading infrastructure
  - Notes: Rust makes this challenging without WASM

- **Plugin Dependencies**: Plugin can declare dependencies on other plugins
  - Complexity: HIGH
  - Value: Enables plugin ecosystem
  - Depends on: Plugin metadata system
  - Notes: Only valuable if ecosystem grows

- **Async Operations with Progress**: Long operations show progress
  - Complexity: MEDIUM
  - Value: Better UX for slow operations
  - Depends on: Async runtime
  - Notes: Current system uses channels for plugin results

### Plugin Configuration

- **Per-Plugin Config**: Each plugin can store its own settings
  - Complexity: MEDIUM
  - Value: Essential for plugins like Jira that need API URLs
  - Depends on: Config system
  - Notes: Could use `~/.config/to-tui/plugins/<name>/config.toml`

- **Plugin Settings UI**: Configure plugins through TUI interface
  - Complexity: HIGH
  - Value: User-friendly plugin management
  - Depends on: Modal system, Config system
  - Notes: Complex but very user-friendly

### Hooks/Events

- **On-Add Hook**: Plugin notified when todo is added
  - Complexity: MEDIUM
  - Value: Enables auto-tagging, validation, enrichment
  - Depends on: Event system
  - Notes: Taskwarrior pattern; powerful for workflows

- **On-Modify Hook**: Plugin notified when todo is changed
  - Complexity: MEDIUM
  - Value: Enables time tracking, audit logging
  - Depends on: Event system
  - Notes: Could enable "log hours when marked complete"

- **On-Rollover Hook**: Plugin notified during daily rollover
  - Complexity: MEDIUM
  - Value: Custom rollover logic (archive to external system)
  - Depends on: Event system
  - Notes: Good for integrations

- **Scheduled Execution**: Plugin runs on schedule (not just user action)
  - Complexity: HIGH
  - Value: Background sync, reminders
  - Depends on: Background task infrastructure
  - Notes: Significant complexity increase


## Anti-Features

Features to deliberately NOT build. Common mistakes to avoid.

- **Full UI Theming by Plugins**: Plugins should NOT control colors/styles
  - Why not: Leads to inconsistent UX, visual chaos
  - What to do instead: Plugins use host theme; maybe allow accent color hints

- **Arbitrary Code Execution Without Sandbox**: Running untrusted code unsafely
  - Why not: Security risk; could damage user data
  - What to do instead: If supporting external plugins, use WASM sandbox

- **Direct Database Write Access**: Plugins directly modifying SQLite
  - Why not: Could corrupt data, bypass soft delete, break consistency
  - What to do instead: Provide mutation API that enforces invariants

- **Blocking UI During Plugin Execution**: Plugin operations freeze the app
  - Why not: Terrible UX; appears crashed
  - What to do instead: Async execution with spinner/progress indicator

- **Plugin-Defined TUI Widgets**: Plugins rendering arbitrary UI
  - Why not: Massive complexity; hard to maintain consistency
  - What to do instead: Plugins work through defined interaction patterns

- **Global State Mutation**: Plugins modifying app state directly
  - Why not: Race conditions, hard to debug, breaks undo
  - What to do instead: Plugins return changes; host applies them

- **Unlimited Network Access**: Plugins making arbitrary HTTP requests
  - Why not: Security/privacy concerns; unexpected network usage
  - What to do instead: If network needed, require explicit permission model

- **Bundling Claude Skills into Plugins**: LLM-powered plugins as v2.0 scope
  - Why not: Adds significant complexity; authentication, rate limits, costs
  - What to do instead: Keep LLM integration separate from plugin system initially


## User Workflows

Common plugin developer and user workflows to design for.

### 1. Create a Generator Plugin (Like Jira)

Steps a developer takes:
1. Create new crate/module implementing `TodoGenerator` trait
2. Implement `name()`, `description()`, `check_available()`, `generate()`
3. Register with `PluginRegistry` (either built-in or discovered)
4. User invokes via `P` key, selects plugin, provides input
5. Plugin returns `Vec<TodoItem>`, user previews, accepts/rejects

### 2. Create a Query Plugin

Steps a developer takes:
1. Create plugin that reads todos matching criteria
2. Query database for todos with specific state/date/project
3. Format output (copy to clipboard, display summary)
4. User invokes, sees results

### 3. Add Custom Keybinding for Plugin

Steps a user takes:
1. Install/enable plugin
2. Edit `~/.config/to-tui/config.toml` keybindings section
3. Add custom binding: `"z" = "plugin:my_plugin"`
4. Restart app (or hot-reload if supported)
5. Press `z` to invoke plugin directly

### 4. Configure Plugin Settings

Steps a user takes:
1. Plugin provides configuration schema
2. User edits `~/.config/to-tui/plugins/<name>/config.toml`
3. Or: User accesses plugin settings through TUI modal
4. Plugin reads its config on invocation

### 5. Hook into Todo Lifecycle

Steps a developer takes:
1. Register hook function: `on_add`, `on_modify`, `on_complete`
2. Hook receives todo item context
3. Hook can return modifications or side effects
4. Changes applied by host after validation


## Feature Dependencies Map

```
Plugin Registration
    |
    v
Plugin Discovery <-- Plugin Enable/Disable
    |
    v
Create Todos (basic) --> Read Todos --> Update Todos --> Delete Todos
    |                         |
    |                         v
    |                    Database Read-Only Access
    |                         |
    v                         v
Custom Keybindings       Custom Metadata
    |                         |
    v                         v
Command System           Per-Plugin Config
    |
    v
Plugin Settings UI
```

## MVP Recommendation

For initial plugin system v2.0, prioritize:

1. **Table stakes first**: All items in Table Stakes section
2. **Key differentiators**:
   - Database Read-Only Access (enables powerful queries)
   - Custom Keybindings (native integration feel)
   - Per-Plugin Config (essential for external integrations)
   - Custom Metadata on Todos (extensibility foundation)

Defer to post-v2.0:
- Plugin Settings UI (complex, can use file-based config initially)
- Hot Reload (Rust makes this hard)
- Hooks/Events (powerful but complex; build after core is solid)
- Scheduled Execution (significant infrastructure)
- Plugin Dependencies (only needed with large ecosystem)

## Sources

### Plugin Architecture Patterns
- [Plugin Architecture Overview](https://www.dotcms.com/blog/plugin-achitecture) - General plugin design principles
- [Plugins in Rust: The Technologies](https://nullderef.com/blog/plugin-tech/) - Rust-specific plugin approaches
- [How to build a plugin system in Rust](https://www.arroyo.dev/blog/rust-plugin-systems/) - WASM-based plugin patterns

### Terminal Application Plugin Systems
- [Zellij Plugin API](https://zellij.dev/documentation/plugin-api) - WASM-based terminal multiplexer plugins
- [WezTerm Event Hooks](https://wezterm.org/config/lua/wezterm/on.html) - Lua-based terminal configuration
- [Neovim Lua Plugin Guide](https://neovim.io/doc/user/lua-guide.html) - Comprehensive plugin API example

### Todo/Task Application Patterns
- [Taskwarrior Hooks v2](https://taskwarrior.org/docs/hooks2/) - CLI-based hook system for task management
- [Taskwarrior::Kusarigama](https://github.com/yanick/Taskwarrior-Kusarigama) - Plugin framework for Taskwarrior

### Security Considerations
- [Wasmtime Security](https://docs.wasmtime.dev/security.html) - WASM sandbox security model
- [wasm-sandbox crate](https://docs.rs/wasm-sandbox/latest/wasm_sandbox/) - Rust WASM sandboxing

### Keybinding Patterns
- [crossterm-keybind](https://github.com/yanganto/crossterm-keybind) - Configurable keybindings for TUI apps
- [Ratatui Keymap Discussion](https://github.com/ratatui/ratatui/discussions/627) - Community patterns for TUI keybindings
