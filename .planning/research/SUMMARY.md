# Project Research Summary

**Project:** to-tui v2.0 Plugin Framework
**Domain:** Rust dynamic plugin system for TUI todo application
**Researched:** 2026-01-24
**Confidence:** HIGH

## Executive Summary

The to-tui plugin system research reveals a critical architectural tension: the Architecture research recommends WebAssembly for safety and portability, while the user requirement is "Dynamic from the start" with native dynamic loading. The resolution is to use **abi_stable for native dynamic loading** (.so/.dylib/.dll) rather than WASM, accepting the security tradeoff for a personal todo app in exchange for simpler integration with existing Rust code.

The recommended approach centers on abi_stable (0.11+) for ABI stability with three-crate architecture: a shared interface crate defining FFI-safe types, host application loading plugins, and plugins implementing the FFI-safe TodoGenerator trait. This requires converting existing TodoItem and related types to FFI-safe equivalents using RString, RVec, ROption from abi_stable. The plugin registry will use TOML manifests and GitHub release downloads via existing reqwest infrastructure.

Key risks are Rust's unstable ABI (mitigated by abi_stable), panic across FFI boundaries (abi_stable handles automatically), and thread-local storage issues on unload (avoid unloading entirely). The existing codebase already has plugin infrastructure (TodoGenerator trait, PluginRegistry, TUI integration) that must be carefully adapted to support dynamic loading without breaking the compile-time plugin path.

## Key Findings

### Recommended Stack

The core dynamic plugin technology stack focuses on abi_stable as the primary ABI stability layer with supporting infrastructure.

**Core technologies:**
- **abi_stable (0.11+)**: FFI-safe plugin interfaces with load-time type checking — provides StableAbi macro, FFI-safe std types (RVec, RString, ROption), and automatic panic handling across FFI boundary
- **libloading (via abi_stable)**: Cross-platform dynamic library loading — handles .so/.dylib/.dll differences transparently, used internally by abi_stable
- **TOML manifests**: Plugin metadata format — reuses existing toml crate dependency, familiar Cargo.toml-like syntax for Rust developers
- **reqwest (existing)**: GitHub release downloads — already in dependencies with stream feature for downloading plugin binaries
- **octocrab (optional)**: GitHub API client — cleaner API for release listing than raw reqwest, but adds dependency

**Critical decision: NOT using WASM**
Architecture research recommended WASM (wasmtime) for sandboxing and portability. However, user requirement "Dynamic from the start" implies native dynamic loading. For a personal todo app, the security tradeoff is acceptable: plugins are trusted first-party code, native loading is simpler to integrate with existing TodoItem/TodoList types, and performance is better for local operations.

**Three-crate architecture required:**
1. `to-tui-plugin-interface` — FFI-safe trait definitions and types
2. `to-tui` (host) — loads plugins via interface crate
3. Plugin crates — implement interface, build as cdylib

### Expected Features

Research identified clear table stakes vs differentiators for plugin system maturity.

**Must have (table stakes):**
- Plugin registration, discovery, enable/disable — users expect to manage plugins
- Create/read todos via plugins — core generator use case
- Plugin input/output (prompts, status messages, preview) — existing PluginSubState handles this
- Graceful error handling and timeout — plugins cannot crash host
- Plugin availability checks — report missing dependencies before execution

**Should have (competitive differentiators):**
- Database read-only access — enables analytics, historical queries, cross-day operations
- Custom metadata on todos — plugin-specific key-value data (requires schema extension)
- Custom keybindings per plugin — native feel, registered via manifest
- Per-plugin config files — essential for external integrations like Jira API keys
- Project-aware operations — plugins understand project boundaries

**Defer (v2+):**
- Plugin settings UI through TUI — complex, file-based config is sufficient initially
- Hot reload — Rust makes this hard, requires careful TypeId handling
- Event hooks (on-add, on-modify) — powerful but complex, build after core is solid
- Scheduled execution — significant background task infrastructure
- Plugin dependencies — only needed with large ecosystem

**Anti-features (deliberately avoid):**
- Full UI theming by plugins — leads to visual chaos
- Direct database write access — could corrupt data, bypass soft delete
- Blocking UI during plugin execution — terrible UX
- Arbitrary network access without permission model — security risk

### Architecture Approach

The architecture integrates plugins through controlled API surface rather than direct state access. Plugins never get mutable references to AppState; instead they call PluginHostApi methods that return PluginCommand enums, which the host processes to apply changes.

**Major components:**
1. **PluginManager** (`src/plugin/manager.rs`) — loads plugins from ~/.config/to-tui/plugins/, manages lifecycle, routes events, maintains command queue
2. **PluginHostApi** (`src/plugin/host_api.rs`) — safe API exposed to plugins: query todos (immutable snapshots), create/update/delete commands, metadata access
3. **PluginCommand** (`src/plugin/command.rs`) — enum of mutations plugins request: CreateTodo, UpdateTodo, SetMetadata, ShowNotification, RegisterKeybinding
4. **Database extension** (`plugin_metadata` table) — stores plugin-specific persistent data keyed by (plugin_id, todo_id, key)
5. **Keybinding integration** — extends Action enum with PluginAction variant, KeybindingCache handles dynamic registrations
6. **FFI-safe type layer** — FfiTodoItem, FfiTodoState, FfiPriority using StableAbi derive, convertible to/from existing types

**Critical pattern:**
```
Plugin calls HostApi::create_todo(...)
  -> PluginHostApi queues PluginCommand::CreateTodo
  -> Event loop processes command
  -> AppState applies via existing save_undo() + mutation
  -> UI re-renders
```

This preserves existing undo/redo, file saving, and UI consistency.

### Critical Pitfalls

Top 7 pitfalls from research, prioritized by impact and phase relevance.

1. **Assuming Rust has stable ABI** — Plugin compiled with different rustc version has different memory layouts, silent corruption. Prevention: Use #[repr(C)] everywhere, abi_stable's load-time type checking. Must address in Phase 1 trait design.

2. **Library outliving its contents (use-after-free)** — Plugin returns trait object, library drops, host calls method on stale vtable, segfault. Prevention: Proxy pattern, store Library alongside all Symbols with lifetimes. Must address in Phase 1-2 structurally.

3. **Panic across FFI boundary** — Plugin panics, unwind through C ABI, undefined behavior. Prevention: abi_stable handles automatically via AbortBomb, wrap exports in catch_unwind. Phase 1-2, baked into export conventions.

4. **Thread-local storage on unload** — Plugin uses thread_local, unloaded with dlclose, thread exits and runs TLS destructor on missing code, segfault. Prevention: Don't unload plugins, keep loaded for app lifetime. Phase 2 design decision.

5. **Type signature mismatch (silent UB)** — Plugin exports fn(i32)->i32, host loads as fn(i64)->i64, no runtime check, garbage results. Prevention: abi_stable's load-time type checking, version verification protocol. Phase 1 trait design.

6. **Using standard library types directly** — Vec<String>, HashMap have no stable ABI, memory layout changes between rustc versions. Prevention: Use RVec, RString, RHashMap from abi_stable. Phase 1, all shared types must be FFI-safe.

7. **Using dylib instead of cdylib** — Plugin uses crate-type = ["dylib"], can only load with exact compiler version. Prevention: Use cdylib for plugins, document in template/scaffold. Phase 1 requirement, plugin author education.

**Phase-specific risk concentration:**
- Phase 1 (Trait Design): Pitfalls #1, #3, #5, #6, #7 — design FFI-safe trait from start
- Phase 2 (Loading): Pitfalls #2, #4 — implement proxy pattern, decide on unload
- Phase 3 (Cross-platform): Symbol export (Windows), code signing (macOS), path handling
- Phase 4 (Registry): Security verification, checksum validation

## Implications for Roadmap

Based on research, suggested 7-phase structure with clear dependencies:

### Phase 1: FFI-Safe Type Layer
**Rationale:** Foundation for everything else. Cannot load plugins without FFI-safe types. Must define before writing any loading code.
**Delivers:** `to-tui-plugin-interface` crate with FfiTodoItem, FfiTodoState, FfiPriority, TodoGeneratorFfi trait using #[sabi_trait].
**Addresses:** Table stakes plugin registration, pitfalls #1, #5, #6, #7 (ABI stability, type mismatches, std types, cdylib).
**Avoids:** Building on unstable foundation that requires rewrite later.
**Research flag:** LOW — abi_stable docs are comprehensive, examples exist.

### Phase 2: Plugin Command Infrastructure
**Rationale:** Defines how plugins communicate changes to host. No runtime dependencies yet, can be tested in isolation.
**Delivers:** PluginCommand enum, PluginContext, TodoItemSnapshot (immutable data for plugins).
**Addresses:** Update/delete todos (table stakes), command pattern for safe state mutation.
**Avoids:** Direct mutable access to AppState (anti-feature).
**Research flag:** LOW — standard command pattern, no novel concepts.

### Phase 3: Plugin Manager Core
**Rationale:** Implements discovery and manifest parsing before dynamic loading complexity.
**Delivers:** PluginManager skeleton, manifest TOML parsing, plugin directory scanning, command queue processing.
**Addresses:** Plugin discovery, enable/disable, availability checks (all table stakes).
**Avoids:** Loading plugins before infrastructure exists to manage them safely.
**Research flag:** MEDIUM — manifest schema design, error handling patterns.

### Phase 4: Dynamic Loading Integration
**Rationale:** Core abi_stable integration after infrastructure is ready.
**Delivers:** WasmPlugin wrapper (misnomer, actually abi_stable), load/unload logic, proxy pattern to prevent use-after-free.
**Addresses:** Pitfalls #2 (library lifetime), #4 (TLS unload — decide not to support).
**Avoids:** Pitfall #2 by using proxy pattern from start.
**Research flag:** HIGH — abi_stable loading sequence, version verification, panic handling testing.

### Phase 5: Host API Layer
**Rationale:** Defines what plugins can do. Requires working plugin loading to test.
**Delivers:** PluginHostApi with query_todos, create/update commands, metadata access.
**Addresses:** Database read-only access, custom metadata (differentiators).
**Avoids:** Blocking UI (async execution pattern).
**Research flag:** MEDIUM — database query API design, snapshot conversion performance.

### Phase 6: Keybinding Integration
**Rationale:** Integrates with existing keybinding system. Requires all previous phases working.
**Delivers:** Action::PluginAction variant, KeybindingCache dynamic registration, execute_navigate_action dispatch to plugins.
**Addresses:** Custom keybindings (differentiator), native integration feel.
**Avoids:** Keybinding conflicts via manifest validation.
**Research flag:** LOW — existing keybinding system is well-structured.

### Phase 7: Database Extensions
**Rationale:** Enables persistent plugin data. Can be last because plugins work without it initially.
**Delivers:** plugin_metadata table, CRUD operations, per-plugin and per-todo metadata.
**Addresses:** Custom metadata on todos/projects (differentiator).
**Avoids:** Direct database write access (anti-feature) by using metadata table.
**Research flag:** LOW — standard SQLite schema extension.

### Phase Ordering Rationale

- **Types before loading:** Cannot load plugins without FFI-safe types (Phase 1 before Phase 4)
- **Commands before API:** Host API returns commands, need command infrastructure first (Phase 2 before Phase 5)
- **Manager before loading:** Need lifecycle management before loading plugins (Phase 3 before Phase 4)
- **Loading before keybindings:** Need working plugins before integrating with keybindings (Phase 4 before Phase 6)
- **Database last:** Metadata is enhancement, not blocker for basic plugins (Phase 7 can be deferred)

This ordering minimizes rework: each phase is testable before the next begins, pitfalls are addressed when they become relevant, and the system is incrementally functional.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 4:** Complex abi_stable integration, version verification protocol, panic handling edge cases require testing with example plugins
- **Phase 5:** Database query API design for plugins, performance of snapshot conversion for large todo lists

Phases with standard patterns (skip research-phase):
- **Phase 1:** abi_stable's StableAbi macro is well-documented with FFI-safe type examples
- **Phase 2:** Command pattern is standard software design, no novel concepts
- **Phase 3:** TOML parsing and directory scanning are established patterns
- **Phase 6:** Extending existing keybinding system follows clear pattern
- **Phase 7:** SQLite schema extension is straightforward database work

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | abi_stable is well-documented with official examples, libloading is mature, TOML/reqwest already in use |
| Features | MEDIUM | Based on Neovim, Zellij, Taskwarrior patterns but adapted to specific to-tui context |
| Architecture | HIGH | Existing codebase analysis combined with verified Rust plugin system patterns from authoritative sources |
| Pitfalls | HIGH | Verified from official Rust docs, Rustonomicon, libloading/abi_stable documentation, and deep technical blog posts |

**Overall confidence:** HIGH

Research sources are authoritative (official docs, Rustonomicon, maintained crate documentation) and findings are consistent across multiple sources. The main uncertainty is not technical feasibility but design tradeoffs: native loading vs WASM was resolved by user requirement clarification.

### Gaps to Address

Areas where research was inconclusive or needs validation during implementation:

- **Performance of FFI type conversion:** Converting TodoItem <-> FfiTodoItem for every plugin call may have overhead. Measure with large todo lists (>1000 items) during Phase 5. Mitigation: Use lazy conversion or pagination for query results.

- **Plugin dependency on external binaries:** Existing Jira plugin uses acli (Atlassian CLI) subprocess. How to handle plugins that need external tools? Research during Phase 3 when migrating existing plugin. Consider manifest field for required_binaries with availability check.

- **Version compatibility across abi_stable updates:** Each abi_stable 0.y.0 version is incompatible. Need strategy for ecosystem coordination if abi_stable 0.12 releases. Document pinned version requirement, coordinate community updates. Address during Phase 4.

- **Custom metadata schema evolution:** plugin_metadata table uses TEXT for value. How to handle plugin schema changes over time? Consider versioning within value (JSON with schema version), or separate columns for common types. Design during Phase 7.

- **Hot reload for development:** Research explicitly punted on hot reload (deferred to v2+), but plugin developers will want faster iteration. Consider documenting `cargo watch` workflow or providing dev mode that reloads app entirely. Not a blocker, address in documentation phase.

## Sources

### Primary (HIGH confidence)
- [abi_stable 0.11.3 docs.rs](https://docs.rs/abi_stable/0.11.3/abi_stable/) — StableAbi macro, FFI-safe types, version verification
- [libloading 0.9.0 docs.rs](https://docs.rs/libloading/0.9.0/libloading/) — Cross-platform dynamic loading, library_filename utility
- [Rust Linkage Reference](https://doc.rust-lang.org/reference/linkage.html) — cdylib vs dylib, crate types
- [Rustonomicon FFI](https://doc.rust-lang.org/nomicon/ffi.html) — repr(C), panic across FFI boundary
- [Rust issue #52138](https://github.com/rust-lang/rust/issues/52138) — TLS unload segfault on all platforms
- [Rust issue #28794](https://github.com/rust-lang/rust/issues/28794) — TLS unload on macOS specifically

### Secondary (MEDIUM confidence)
- [NullDeref: Plugins in Rust with abi_stable](https://nullderef.com/blog/plugin-abi-stable/) — October 2025, detailed tutorial with proxy pattern examples
- [NullDeref: Dynamic Loading](https://nullderef.com/blog/plugin-dynload/) — Foundation concepts, use-after-free examples
- [Arroyo: Rust Plugin Systems](https://www.arroyo.dev/blog/rust-plugin-systems/) — WASM vs native comparison, architectural patterns
- [Zellij Plugin API](https://zellij.dev/documentation/plugin-api) — WASM-based terminal plugin architecture
- [Taskwarrior Hooks v2](https://taskwarrior.org/docs/hooks2/) — Event hook patterns for task management
- [Neovim Lua Plugin Guide](https://neovim.io/doc/user/lua-guide.html) — Plugin API design patterns

### Tertiary (informational)
- [octocrab docs.rs](https://docs.rs/octocrab/latest/octocrab/repos/struct.ReleasesHandler.html) — GitHub release API
- [Helix Plugin System Discussion](https://github.com/helix-editor/helix/discussions/3806) — Community plugin architecture debate
- [hot-lib-reloader](https://docs.rs/hot-lib-reloader/latest/hot_lib_reloader/) — Hot reload patterns for development
- [Plugin Architecture Overview](https://www.dotcms.com/blog/plugin-achitecture) — General plugin design principles

---
*Research completed: 2026-01-24*
*Ready for roadmap: yes*
