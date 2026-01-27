# Requirements: to-tui v2.0 Plugin Framework

**Defined:** 2026-01-24
**Core Value:** Fast, keyboard-driven todo management that lives in the terminal and integrates with the tools I already use.

## v2.0 Requirements

Requirements for the Plugin Framework milestone. Each maps to roadmap phases.

### Plugin Infrastructure

- [x] **PLUG-01**: Plugin trait with stable ABI using abi_stable crate
- [x] **PLUG-02**: Plugin manifest format (TOML) with name, version, description, permissions
- [x] **PLUG-03**: Plugin discovery from ~/.local/share/to-tui/plugins/
- [x] **PLUG-04**: Plugin registration with PluginManager
- [x] **PLUG-05**: Plugin enable/disable via config without uninstalling
- [x] **PLUG-06**: Plugin availability check with dependency reporting

### Dynamic Loading

- [x] **LOAD-01**: Dynamic loading of .so/.dylib/.dll plugins at runtime
- [x] **LOAD-02**: FFI-safe type layer (FfiTodoItem, FfiTodoState, etc.)
- [x] **LOAD-03**: Proxy pattern to prevent use-after-free on library lifetime
- [x] **LOAD-04**: Panic catching at FFI boundary
- [x] **LOAD-05**: Version compatibility checking before method calls

### Todo Operations API

- [x] **TODO-01**: Plugin can create new todo items
- [x] **TODO-02**: Plugin can read/query current todo list
- [x] **TODO-03**: Plugin can update existing todo items
- [x] **TODO-04**: Plugin can soft-delete todo items
- [x] **TODO-05**: All mutations integrate with undo/redo system

### Data Access

- [x] **DATA-01**: Plugin receives current project context
- [x] **DATA-02**: Plugin can attach custom metadata to todos (JSON blob)
- [x] **DATA-03**: Plugin can attach custom metadata to projects (JSON blob)
- [x] **DATA-04**: Metadata persists in database with todo/project records

### Keybinding Integration

- [x] **KEYS-01**: Plugin can define custom actions with names
- [x] **KEYS-02**: Plugin can specify default keybindings for its actions
- [x] **KEYS-03**: User can override plugin keybindings in config.toml
- [x] **KEYS-04**: Key events route to plugins after host handling
- [x] **KEYS-05**: Plugin keybindings use namespace (plugin:name:action)

### Configuration

- [x] **CONF-01**: Per-plugin config directory (~/.config/to-tui/plugins/<name>/)
- [x] **CONF-02**: Plugin can read its own config.toml on init
- [x] **CONF-03**: Plugin can define config schema for validation

### Event Hooks

- [x] **HOOK-01**: Plugin can register for on-add events
- [x] **HOOK-02**: Plugin can register for on-modify events
- [x] **HOOK-03**: Plugin can register for on-complete events
- [x] **HOOK-04**: Hooks receive todo context and can return modifications
- [x] **HOOK-05**: Hook execution is async and non-blocking

### Distribution

- [x] **DIST-01**: Local plugin installation from directory
- [x] **DIST-02**: GitHub repository plugin source support
- [x] **DIST-03**: Plugin download command (totui plugin install <source>)
- [x] **DIST-04**: Plugin list command showing installed plugins
- [x] **DIST-05**: grimurjonsson/to-tui-plugins as default registry

### Migration

- [ ] **MIGR-01**: Create Jira plugin as first plugin in grimurjonsson/to-tui-plugins
- [ ] **MIGR-02**: Remove built-in Jira generator code from to-tui binary
- [ ] **MIGR-03**: Existing plugin invocation (P key) works with new system

## Future Requirements (v2.1+)

Acknowledged but deferred. Not in current roadmap.

### Database Query Access

- **DB-01**: Plugin can execute read-only SQLite queries
- **DB-02**: Plugin can search archived todos
- **DB-03**: Query API with parameterized prepared statements

### Advanced Distribution

- **DIST-06**: Plugin version management with semver
- **DIST-07**: Auto-update check for installed plugins
- **DIST-08**: Plugin dependency resolution

### UI Features

- **UI-01**: Plugin settings modal in TUI
- **UI-02**: Plugin can hint accent colors for its elements

### Claude Integration

- **CLAU-01**: Plugin can bundle Claude Code skills
- **CLAU-02**: Plugin installer adds skills to ~/.claude/skills/

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| UI theming by plugins | Leads to inconsistent UX, visual chaos |
| Direct database writes | Could corrupt data, bypass soft delete |
| Arbitrary network access | Security/privacy concerns |
| Plugin-defined TUI widgets | Massive complexity, hard to maintain |
| Hot reload of plugins | Rust ABI instability makes this very hard |
| WASM plugins | User chose native dynamic loading for v2.0 |
| Any-language plugins via IPC | Deferred to v2.1+ per user decision |
| Scheduled/background execution | Significant infrastructure complexity |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PLUG-01 | Phase 6 | Complete |
| PLUG-02 | Phase 7 | Complete |
| PLUG-03 | Phase 7 | Complete |
| PLUG-04 | Phase 7 | Complete |
| PLUG-05 | Phase 7 | Complete |
| PLUG-06 | Phase 7 | Complete |
| LOAD-01 | Phase 8 | Complete |
| LOAD-02 | Phase 6 | Complete |
| LOAD-03 | Phase 8 | Complete |
| LOAD-04 | Phase 8 | Complete |
| LOAD-05 | Phase 6 | Complete |
| TODO-01 | Phase 9 | Complete |
| TODO-02 | Phase 9 | Complete |
| TODO-03 | Phase 9 | Complete |
| TODO-04 | Phase 9 | Complete |
| TODO-05 | Phase 9 | Complete |
| DATA-01 | Phase 9 | Complete |
| DATA-02 | Phase 10 | Complete |
| DATA-03 | Phase 10 | Complete |
| DATA-04 | Phase 10 | Complete |
| KEYS-01 | Phase 12 | Complete |
| KEYS-02 | Phase 12 | Complete |
| KEYS-03 | Phase 12 | Complete |
| KEYS-04 | Phase 12 | Complete |
| KEYS-05 | Phase 12 | Complete |
| CONF-01 | Phase 11 | Complete |
| CONF-02 | Phase 11 | Complete |
| CONF-03 | Phase 11 | Complete |
| HOOK-01 | Phase 13 | Complete |
| HOOK-02 | Phase 13 | Complete |
| HOOK-03 | Phase 13 | Complete |
| HOOK-04 | Phase 13 | Complete |
| HOOK-05 | Phase 13 | Complete |
| DIST-01 | Phase 14 | Complete |
| DIST-02 | Phase 14 | Complete |
| DIST-03 | Phase 14 | Complete |
| DIST-04 | Phase 14 | Complete |
| DIST-05 | Phase 14 | Complete |
| MIGR-01 | Phase 15 | Pending |
| MIGR-02 | Phase 15 | Pending |
| MIGR-03 | Phase 15 | Pending |

**Coverage:**
- v2.0 requirements: 41 total
- Mapped to phases: 41
- Unmapped: 0

---
*Requirements defined: 2026-01-24*
*Last updated: 2026-01-26 — Phase 14 (Distribution) requirements marked complete*
