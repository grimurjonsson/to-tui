# Roadmap: to-tui

## Milestones

- **v1.0 TUI Enhancements** — Phases 1-5 (shipped 2026-01-21) — [archive](milestones/v1.0-ROADMAP.md)
- **v2.0 Plugin Framework** — Phases 6-15 (in progress)

## Current: v2.0 Plugin Framework

**Milestone Goal:** Enable external plugins to extend to-tui with custom todo generators, keybindings, and event hooks using native dynamic loading with stable ABI.

## Phases

**Phase Numbering:**
- Integer phases (6, 7, 8...): Planned milestone work
- Decimal phases (7.1, 7.2): Urgent insertions (marked with INSERTED)

- [x] **Phase 6: FFI-Safe Type Layer** - Foundation types using abi_stable for stable ABI
- [x] **Phase 7: Plugin Manager Core** - Discovery, manifest parsing, lifecycle management
- [x] **Phase 8: Dynamic Loading** - abi_stable integration with proxy pattern
- [x] **Phase 9: Host API Layer** - Todo CRUD operations exposed to plugins
- [x] **Phase 10: Metadata & Database** - Custom JSON metadata for todos and projects
- [x] **Phase 11: Plugin Configuration** - Per-plugin config directories and schema
- [x] **Phase 12: Keybinding Integration** - Plugin-defined actions and key routing
- [x] **Phase 13: Event Hooks** - Lifecycle events (on-add, on-modify, on-complete)
- [x] **Phase 14: Distribution** - Local and GitHub-based plugin installation
- [ ] **Phase 15: Migration** - Jira plugin extraction to registry

## Phase Details

### Phase 6: FFI-Safe Type Layer
**Goal**: Establish stable ABI foundation with FFI-safe type definitions
**Depends on**: Nothing (first v2.0 phase)
**Requirements**: PLUG-01, LOAD-02, LOAD-05
**Success Criteria** (what must be TRUE):
  1. FfiTodoItem, FfiTodoState, FfiPriority types exist with #[derive(StableAbi)]
  2. Plugin trait defined with #[sabi_trait] macro
  3. Conversion between native types and FFI types works bidirectionally
  4. Version compatibility protocol prevents loading incompatible plugins
**Plans**: 2 plans

Plans:
- [x] 06-01-PLAN.md — Interface crate and FFI-safe types
- [x] 06-02-PLAN.md — Plugin trait definition with version protocol

### Phase 7: Plugin Manager Core
**Goal**: Plugins can be discovered, registered, and managed without dynamic loading
**Depends on**: Phase 6
**Requirements**: PLUG-02, PLUG-03, PLUG-04, PLUG-05, PLUG-06
**Success Criteria** (what must be TRUE):
  1. TOML manifest format defines plugin name, version, description, permissions
  2. Plugins in ~/.local/share/to-tui/plugins/ are discovered at startup
  3. PluginManager tracks registered plugins with enable/disable state
  4. Plugin availability check reports missing dependencies
  5. Disabled plugins are not loaded but remain installed
**Plans**: 3 plans

Plans:
- [x] 07-01-PLAN.md — Manifest format and parsing with validation
- [x] 07-02-PLAN.md — PluginManager discovery and registration
- [x] 07-03-PLAN.md — Config extension and CLI commands

### Phase 8: Dynamic Loading
**Goal**: Native plugins (.so/.dylib/.dll) load at runtime with safety guarantees
**Depends on**: Phase 7
**Requirements**: LOAD-01, LOAD-03, LOAD-04
**Success Criteria** (what must be TRUE):
  1. Dynamic libraries load on Linux (.so), macOS (.dylib), and Windows (.dll)
  2. Proxy pattern keeps library alive as long as any plugin object exists
  3. Plugin panics are caught at FFI boundary without crashing host
  4. Plugins never unload during app lifetime (TLS safety)
**Plans**: 2 plans

Plans:
- [x] 08-01-PLAN.md — PluginLoader with abi_stable loading and panic-safe calling
- [x] 08-02-PLAN.md — TUI integration with error popup and startup experience

### Phase 9: Host API Layer
**Goal**: Plugins can perform CRUD operations on todos with undo/redo support
**Depends on**: Phase 8
**Requirements**: TODO-01, TODO-02, TODO-03, TODO-04, TODO-05, DATA-01
**Success Criteria** (what must be TRUE):
  1. Plugin can create new todo items via PluginHostApi
  2. Plugin can query current todo list (immutable snapshot)
  3. Plugin can update existing todo content, state, and properties
  4. Plugin can soft-delete todo items
  5. All plugin mutations integrate with existing undo/redo system
  6. Plugin receives current project context on invocation
**Plans**: 4 plans

Plans:
- [x] 09-01-PLAN.md — FFI types for commands, HostApi trait, query types
- [x] 09-02-PLAN.md — PluginHostApiImpl with query methods
- [x] 09-03-PLAN.md — CommandExecutor with undo integration
- [x] 09-04-PLAN.md — Gap closure: Wire calling convention with undo integration

### Phase 10: Metadata & Database
**Goal**: Plugins can persist custom data attached to todos and projects
**Depends on**: Phase 9
**Requirements**: DATA-02, DATA-03, DATA-04
**Success Criteria** (what must be TRUE):
  1. Plugin can attach JSON metadata blob to any todo item
  2. Plugin can attach JSON metadata blob to any project
  3. Metadata persists in database alongside todo/project records
  4. Metadata survives todo edits and app restarts
**Plans**: 3 plans

Plans:
- [x] 10-01-PLAN.md — Database schema and storage layer CRUD operations
- [x] 10-02-PLAN.md — FFI types and HostApi trait extension
- [x] 10-03-PLAN.md — Host implementation wiring CommandExecutor and PluginHostApiImpl

### Phase 11: Plugin Configuration
**Goal**: Each plugin has isolated configuration with schema validation
**Depends on**: Phase 7
**Requirements**: CONF-01, CONF-02, CONF-03
**Success Criteria** (what must be TRUE):
  1. Per-plugin config directory exists at ~/.config/to-tui/plugins/<name>/
  2. Plugin can read its config.toml during initialization
  3. Plugin can define config schema for validation
  4. Invalid config fails plugin initialization with clear error
**Plans**: 2 plans

Plans:
- [x] 11-01-PLAN.md — FFI config types, Plugin trait extension, host-side config loader
- [x] 11-02-PLAN.md — PluginLoader integration, CLI commands (validate, config --init), TUI error popup

### Phase 12: Keybinding Integration
**Goal**: Plugins can define custom actions triggered by keybindings
**Depends on**: Phase 9
**Requirements**: KEYS-01, KEYS-02, KEYS-03, KEYS-04, KEYS-05
**Success Criteria** (what must be TRUE):
  1. Plugin can define named actions via manifest
  2. Plugin can specify default keybindings for its actions
  3. User can override plugin keybindings in config.toml
  4. Key events route to plugins after host handling
  5. Plugin keybindings use namespace format (plugin:name:action)
**Plans**: 2 plans

Plans:
- [x] 12-01-PLAN.md — Manifest actions field, PluginActionRegistry, validation
- [x] 12-02-PLAN.md — Config overrides, key routing, help panel, action execution

### Phase 13: Event Hooks
**Goal**: Plugins can respond to todo lifecycle events asynchronously
**Depends on**: Phase 9
**Requirements**: HOOK-01, HOOK-02, HOOK-03, HOOK-04, HOOK-05
**Success Criteria** (what must be TRUE):
  1. Plugin can register handler for on-add events
  2. Plugin can register handler for on-modify events
  3. Plugin can register handler for on-complete events
  4. Hooks receive todo context and can return modifications
  5. Hook execution is async and does not block UI
**Plans**: 3 plans

Plans:
- [x] 13-01-PLAN.md — FFI event types and Plugin trait extension
- [x] 13-02-PLAN.md — HookDispatcher with async dispatch and result polling
- [x] 13-03-PLAN.md — TUI integration, event firing, cascade prevention

### Phase 14: Distribution
**Goal**: Plugins can be installed from local directories or GitHub repositories
**Depends on**: Phase 8
**Requirements**: DIST-01, DIST-02, DIST-03, DIST-04, DIST-05
**Success Criteria** (what must be TRUE):
  1. Local plugin installation works from directory path
  2. GitHub repository can be specified as plugin source
  3. `totui plugin install <source>` command downloads and installs plugins
  4. `totui plugin list` command shows installed plugins with status
  5. grimurjonsson/to-tui-plugins serves as default registry
**Plans**: 3 plans

Plans:
- [x] 14-01-PLAN.md — Local plugin installation with PluginInstaller and PluginSource
- [x] 14-02-PLAN.md — GitHub download and remote install command
- [x] 14-03-PLAN.md — Enhanced list command, marketplace manifest, default registry config

### Phase 15: Migration
**Goal**: Existing Jira generator becomes first external plugin in registry
**Depends on**: Phase 14
**Requirements**: MIGR-01, MIGR-02, MIGR-03
**Success Criteria** (what must be TRUE):
  1. Jira plugin exists in grimurjonsson/to-tui-plugins repository
  2. Built-in Jira generator code removed from to-tui binary
  3. P key invocation works with new plugin system seamlessly
**Plans**: 3 plans

Plans:
- [ ] 15-01-PLAN.md — Create jira-claude external plugin and registry setup
- [ ] 15-02-PLAN.md — Remove built-in generator and add version to LoadedPlugin
- [ ] 15-03-PLAN.md — Implement tabbed plugins modal with E2E verification

## Progress

**Execution Order:**
Phases execute in numeric order: 6 -> 6.1 -> 7 -> 7.1 -> ... -> 15

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 6. FFI-Safe Type Layer | v2.0 | 2/2 | Complete | 2026-01-24 |
| 7. Plugin Manager Core | v2.0 | 3/3 | Complete | 2026-01-24 |
| 8. Dynamic Loading | v2.0 | 2/2 | Complete | 2026-01-25 |
| 9. Host API Layer | v2.0 | 4/4 | Complete | 2026-01-26 |
| 10. Metadata & Database | v2.0 | 3/3 | Complete | 2026-01-26 |
| 11. Plugin Configuration | v2.0 | 2/2 | Complete | 2026-01-26 |
| 12. Keybinding Integration | v2.0 | 2/2 | Complete | 2026-01-26 |
| 13. Event Hooks | v2.0 | 3/3 | Complete | 2026-01-26 |
| 14. Distribution | v2.0 | 3/3 | Complete | 2026-01-26 |
| 15. Migration | v2.0 | 0/3 | Not started | - |

## Archived Milestones

<details>
<summary>v1.0 TUI Enhancements (Phases 1-5) - SHIPPED 2026-01-21</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full details.

| Phase | Plans | Status | Completed |
|-------|-------|--------|-----------|
| 1. Foundation | 3/3 | Complete | 2026-01-18 |
| 2. Clipboard | 3/3 | Complete | 2026-01-19 |
| 3. Scrolling | 3/3 | Complete | 2026-01-19 |
| 4. Priority | 3/3 | Complete | 2026-01-20 |
| 5. Self-Upgrade | 3/3 | Complete | 2026-01-21 |

</details>
