# Stack Research: Dynamic Plugin System

**Project:** to-tui
**Researched:** 2026-01-24
**Focus:** Dynamic plugin loading for Rust TUI application
**Confidence:** HIGH (verified with official docs)

## Executive Summary

For a dynamic plugin system in to-tui, the recommended approach is **`abi_stable`** for FFI-safe plugin interfaces combined with **`libloading`** (used internally by abi_stable) for cross-platform dynamic library loading. The existing `reqwest` in the codebase can handle GitHub release downloads for plugin auto-installation.

## Recommended Stack

### Dynamic Loading & ABI Stability

```toml
[dependencies]
abi_stable = "0.11"
```

- **Purpose:** Provides stable ABI for Rust-to-Rust FFI, enabling plugins compiled with different rustc versions to work together
- **Why this one:**
  - Built on `libloading` internally, but adds crucial ABI stability layer
  - Provides `StableAbi` derive macro for FFI-safe types
  - Includes FFI-safe alternatives to std types: `RVec<T>`, `RString`, `ROption<T>`
  - Load-time type checking prevents version mismatches from causing segfaults
  - Active maintenance (0.11.3 current, targets Rust 1.61+)
  - Well-documented with examples
- **Cross-platform:** Yes (Windows, macOS, Linux) - uses platform-appropriate loading primitives

### Plugin Interface Crate Pattern

The project needs a **separate interface crate** that both to-tui and plugins depend on:

```
to-tui-plugin-interface/
  Cargo.toml
  src/lib.rs  <- Defines FFI-safe TodoGenerator trait and types
```

This follows the recommended three-crate architecture:
1. **Interface crate** (to-tui-plugin-interface) - defines types/traits
2. **Implementation crate** (each plugin) - implements the interface
3. **User crate** (to-tui) - loads plugins via the interface

### FFI-Safe Type Wrappers

`abi_stable` provides these FFI-safe replacements needed for `TodoItem`:

| Standard Type | FFI-Safe Equivalent | Notes |
|---------------|---------------------|-------|
| `String` | `RString` | O(1) conversion via `ManuallyDrop` |
| `Vec<T>` | `RVec<T>` | O(1) conversion |
| `Option<T>` | `ROption<T>` | Works with any StableAbi T |
| `Box<dyn Trait>` | `#[sabi_trait]` macro | FFI-safe trait objects |
| `Result<T,E>` | `RResult<T,E>` | For error handling across FFI |

### Plugin Registry & Manifest Format

**Recommended: TOML manifest** (already used via `toml = "0.9"` in codebase)

```toml
# ~/.config/to-tui/plugins/example-plugin/manifest.toml
[plugin]
name = "example-plugin"
version = "1.0.0"
api_version = "1"  # to-tui-plugin-interface version
description = "An example todo generator plugin"
authors = ["Author Name"]
license = "MIT"
repository = "https://github.com/user/example-plugin"

[source]
github = "user/example-plugin"
asset_pattern = "to-tui-plugin-{target}-{version}"

[build]
# For local development
lib_name = "libtotui_example_plugin"
```

**Why TOML:**
- Already in the codebase dependency tree
- Familiar Cargo.toml-like syntax for Rust developers
- Human-readable and editable
- Serde support via existing `serde = { features = ["derive"] }`

### GitHub Release Downloads

**Use existing `reqwest`** (already in Cargo.toml with `stream` feature):

```toml
# Already present:
reqwest = { version = "0.12", features = ["rustls-tls", "json", "blocking", "stream"] }
futures-util = "0.3"
```

For more ergonomic GitHub API access, optionally add:

```toml
[dependencies]
octocrab = { version = "0.47", default-features = false, features = ["rustls", "stream"] }
```

- **Purpose:** GitHub API client for listing releases, getting asset URLs
- **Why:** Cleaner than raw reqwest for GitHub-specific operations (release listing, asset streaming)
- **Optional:** Can use raw reqwest + GitHub API directly if minimizing dependencies

### Async Considerations

`abi_stable` does not directly support async. For async plugin operations:

```toml
[dependencies]
async_ffi = "0.5"  # If async plugin methods are needed
```

**Recommendation:** Keep plugin interface synchronous where possible. The `generate()` method in the existing `TodoGenerator` trait is sync, which is simpler for FFI. Plugins that need async (e.g., HTTP calls) should handle it internally and block.

## Complete Dependencies Addition

```toml
# Add to existing Cargo.toml [dependencies]

# Core plugin system
abi_stable = "0.11"

# Optional: Better GitHub API ergonomics
octocrab = { version = "0.47", default-features = false, features = ["rustls", "stream"] }

# Existing deps already satisfy:
# - toml (manifest parsing)
# - reqwest + futures-util (downloading)
# - serde (serialization)
```

## Integration Notes

### Compatibility with Existing Stack

| Existing | Plugin System | Integration |
|----------|---------------|-------------|
| `ratatui` | No interaction | Plugins run before UI display |
| `axum` | No interaction | Plugins are local, not HTTP |
| `tokio` | Plugin loading in async context | Load in `spawn_blocking` |
| `rusqlite` | Plugin data may write to DB | Plugins return `Vec<TodoItem>`, host writes |
| `serde` | `abi_stable` has serde support | Enable `abi_stable/serde` feature if needed |

### Existing TodoGenerator Trait

Current trait in `src/plugin/mod.rs`:
```rust
pub trait TodoGenerator: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn check_available(&self) -> Result<(), String>;
    fn generate(&self, input: &str) -> Result<Vec<TodoItem>>;
}
```

This will need an FFI-safe equivalent using `#[sabi_trait]`:
```rust
#[sabi_trait]
pub trait TodoGeneratorFfi: Send + Sync {
    fn name(&self) -> RStr<'_>;
    fn description(&self) -> RStr<'_>;
    fn check_available(&self) -> RResult<(), RString>;
    fn generate(&self, input: RStr<'_>) -> RResult<RVec<FfiTodoItem>, RString>;
}
```

### Type Conversions Needed

The existing types need FFI-safe mirrors:

```rust
// FFI-safe TodoState
#[repr(u8)]
#[derive(StableAbi)]
pub enum FfiTodoState {
    Empty = 0,
    Checked = 1,
    Question = 2,
    Exclamation = 3,
    InProgress = 4,
    Cancelled = 5,
}

// FFI-safe Priority
#[repr(u8)]
#[derive(StableAbi)]
pub enum FfiPriority {
    P0 = 0,
    P1 = 1,
    P2 = 2,
}

// FFI-safe TodoItem
#[repr(C)]
#[derive(StableAbi)]
pub struct FfiTodoItem {
    pub id: [u8; 16],  // UUID as bytes
    pub content: RString,
    pub state: FfiTodoState,
    pub indent_level: usize,
    pub parent_id: ROption<[u8; 16]>,
    pub due_date: ROption<i64>,  // Unix timestamp
    pub description: ROption<RString>,
    pub priority: ROption<FfiPriority>,
    // Omit internal fields (created_at, etc.) - host manages those
}
```

## What NOT to Use

### dlopen2 - Rejected

```toml
# Don't use:
# dlopen2 = "0.8"
```

**Why rejected:**
- `abi_stable` uses `libloading` internally
- Adding `dlopen2` creates redundant dependency
- `dlopen2`'s "nicer interface" benefits are superseded by `abi_stable`'s higher-level abstractions

### stabby - Considered but Not Recommended

```toml
# Alternative, not primary:
# stabby = "6.0"
```

**Why not primary:**
- Newer, less ecosystem adoption than `abi_stable`
- Uses canary symbols for version checking (creative but less mature)
- `abi_stable` has more battle-tested FFI-safe std replacements
- Could reconsider if `abi_stable` maintenance stalls

### cglue - Too Limited

```toml
# Don't use:
# cglue = "0.3"
```

**Why rejected:**
- Only handles FFI-safe trait objects
- `abi_stable`'s `#[sabi_trait]` covers this use case
- Doesn't provide the broader type system needed

### WASM (Wasmtime/Wasmer) - Overkill

```toml
# Don't use for this project:
# wasmtime = "..."
```

**Why rejected:**
- Adds significant complexity and runtime overhead
- to-tui plugins are trusted first-party/community code
- WASM sandboxing benefits not needed for this use case
- Would require plugins to be compiled to WASM, limiting Rust ecosystem access

### Raw libloading Alone - Too Low Level

```toml
# Don't use directly:
# libloading = "0.9"
```

**Why not alone:**
- Requires manual `#[repr(C)]` everywhere
- No load-time type checking
- No FFI-safe std types
- Use `abi_stable` which wraps it properly

## Cross-Platform Notes

### Library File Extensions

`libloading` (via `abi_stable`) handles platform differences:

| Platform | Extension | Notes |
|----------|-----------|-------|
| Linux | `.so` | `libplugin.so` |
| macOS | `.dylib` | `libplugin.dylib` |
| Windows | `.dll` | `plugin.dll` (no `lib` prefix) |

Use `libloading::library_filename()` to get the correct name.

### Plugin Directory Structure

```
~/.local/share/to-tui/plugins/
  installed.json                    # Registry of installed plugins
  example-plugin/
    manifest.toml                   # Plugin metadata
    libexample_plugin.so            # Linux
    libexample_plugin.dylib         # macOS
    example_plugin.dll              # Windows
```

### Build Matrix for Plugins

Plugin authors will need to build for:
- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

GitHub Actions can automate this with the same approach to-tui already uses for releases.

## Version Compatibility Strategy

### API Versioning

```rust
// In to-tui-plugin-interface
pub const API_VERSION: u32 = 1;
```

- Bump when breaking changes to FFI types
- Plugins declare required `api_version` in manifest
- Host refuses to load incompatible versions

### abi_stable Versioning

Each `0.y.0` version of `abi_stable` defines its own incompatible ABI. Strategy:
- Pin to specific minor version (e.g., `0.11`)
- Document required `abi_stable` version for plugins
- Rare updates, coordinated with community

## Sources

### HIGH Confidence (Official Documentation)
- [abi_stable docs.rs](https://docs.rs/abi_stable/0.11.3/abi_stable/) - Version 0.11.3, API reference
- [libloading docs.rs](https://docs.rs/libloading/0.9.0/libloading/) - Version 0.9.0
- [octocrab docs.rs](https://docs.rs/octocrab/latest/octocrab/repos/struct.ReleasesHandler.html) - Release asset streaming

### MEDIUM Confidence (Verified Blog Posts)
- [NullDeref: Plugins in Rust with abi_stable](https://nullderef.com/blog/plugin-abi-stable/) - October 2025, detailed tutorial
- [NullDeref: Dynamic Loading](https://nullderef.com/blog/plugin-dynload/) - Foundation concepts
- [Arroyo: Rust Plugin Systems](https://www.arroyo.dev/blog/rust-plugin-systems/) - Comparison of approaches

### GitHub Repositories
- [abi_stable_crates](https://github.com/rodrimati1992/abi_stable_crates) - Examples in repo
- [rust_libloading](https://github.com/nagisa/rust_libloading) - Platform support details
- [octocrab](https://github.com/XAMPPRocky/octocrab) - GitHub API client

## Summary

**Add these dependencies:**
```toml
abi_stable = "0.11"
# Optional:
octocrab = { version = "0.47", default-features = false, features = ["rustls", "stream"] }
```

**Create interface crate:** `to-tui-plugin-interface` with FFI-safe types and traits.

**Use TOML manifests** for plugin metadata (leverages existing `toml` crate).

**Use existing `reqwest`** for downloading, or add `octocrab` for cleaner GitHub API.

**Cross-platform verified:** All recommended crates support Windows, macOS, and Linux.
