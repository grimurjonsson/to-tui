# Phase 6: FFI-Safe Type Layer - Research

**Researched:** 2026-01-24
**Domain:** Rust stable ABI, FFI-safe types, dynamic plugin loading
**Confidence:** HIGH

## Summary

This research covers the abi_stable crate for creating FFI-safe types that enable dynamic Rust plugin loading with stable ABI guarantees. The crate provides derive macros (`#[derive(StableAbi)]`, `#[sabi_trait]`) and FFI-safe wrappers for standard library types (RString, RVec, ROption, RResult).

The phase requires creating FFI-safe equivalents of the existing TodoItem, TodoState, and Priority types, plus a Plugin trait that can be safely called across dynamic library boundaries. Version compatibility checking is built into the RootModule pattern.

**Primary recommendation:** Create a separate `totui-plugin-interface` crate containing all FFI-safe types and the Plugin trait. Use `#[repr(C)]` with `#[derive(StableAbi)]` for all types. Use RString for strings, ROption for optionals, i64 timestamps for DateTime, and RResult<T, RString> for error handling.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| [abi_stable](https://docs.rs/abi_stable/) | 0.11.3 | Stable ABI derive macros and FFI-safe types | The only mature Rust-to-Rust stable ABI solution; type layout verification at load time |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| core_extensions | (dep of abi_stable) | Internal utilities | Automatically included |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| abi_stable | libloading + manual C ABI | More control but massive unsafe surface, no type verification |
| abi_stable | WASM (wasmtime) | Sandboxing but user explicitly chose native loading |
| RString | *const c_char | Simpler but no length tracking, null-termination issues |

**Installation:**
```toml
# In totui-plugin-interface/Cargo.toml
[dependencies]
abi_stable = "0.11"
```

## Architecture Patterns

### Recommended Project Structure
```
to-tui/
├── Cargo.toml                     # Workspace root
├── crates/
│   └── totui-plugin-interface/    # Interface crate (lib)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs             # Re-exports all types
│           ├── types.rs           # FfiTodoItem, FfiTodoState, FfiPriority
│           ├── plugin.rs          # Plugin trait via #[sabi_trait]
│           └── version.rs         # Version protocol
├── src/                           # Host crate (unchanged structure)
│   ├── main.rs
│   ├── lib.rs
│   └── plugin/
│       └── ffi_convert.rs         # Native <-> FFI conversions
```

### Pattern 1: FFI-Safe Type Definition
**What:** Define types with `#[repr(C)]` and `#[derive(StableAbi)]`
**When to use:** Any type that crosses the FFI boundary
**Example:**
```rust
// Source: https://docs.rs/abi_stable/latest/abi_stable/derive.StableAbi.html
use abi_stable::StableAbi;
use abi_stable::std_types::{ROption, RString};

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiTodoItem {
    pub id: RString,                          // UUID as string
    pub content: RString,
    pub state: FfiTodoState,
    pub priority: ROption<FfiPriority>,
    pub due_date: ROption<RString>,           // YYYY-MM-DD format
    pub description: ROption<RString>,
    pub parent_id: ROption<RString>,          // UUID as string
    pub indent_level: u32,                    // usize not FFI-safe, use u32
    pub created_at: i64,                      // Unix timestamp millis
    pub modified_at: i64,
    pub completed_at: ROption<i64>,
}

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiTodoState {
    Empty = 0,
    Checked = 1,
    Question = 2,
    Exclamation = 3,
    InProgress = 4,
    Cancelled = 5,
}

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiPriority {
    P0 = 0,
    P1 = 1,
    P2 = 2,
}
```

### Pattern 2: sabi_trait for Plugin Trait
**What:** Use `#[sabi_trait]` to generate FFI-safe trait objects
**When to use:** Any trait that plugins implement
**Example:**
```rust
// Source: https://docs.rs/abi_stable/latest/abi_stable/attr.sabi_trait.html
use abi_stable::sabi_trait;
use abi_stable::std_types::{RResult, RString, RVec};

#[sabi_trait]
pub trait Plugin: Send + Sync + Debug {
    /// Plugin name for display
    fn name(&self) -> RString;

    /// Plugin version (semver)
    fn version(&self) -> RString;

    /// Minimum interface version this plugin requires
    fn min_interface_version(&self) -> RString;

    /// Generate todos from input
    #[sabi(last_prefix_field)]
    fn generate(&self, input: RString) -> RResult<RVec<FfiTodoItem>, RString>;
}
```

### Pattern 3: RootModule for Version Protocol
**What:** Use RootModule trait for library entry point with version checking
**When to use:** The plugin's exported module
**Example:**
```rust
// Source: https://docs.rs/abi_stable/latest/abi_stable/library/trait.RootModule.html
use abi_stable::{
    library::RootModule,
    package_version_strings,
    sabi_types::VersionStrings,
    StableAbi,
};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginModule_Ref)))]
pub struct PluginModule {
    /// Factory function to create plugin instance
    #[sabi(last_prefix_field)]
    pub create_plugin: extern "C" fn() -> Plugin_TO<'static, RBox<()>>,
}

impl RootModule for PluginModule_Ref {
    abi_stable::declare_root_module_statics!{PluginModule_Ref}

    const BASE_NAME: &'static str = "totui_plugin";
    const NAME: &'static str = "to-tui plugin";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}
```

### Pattern 4: Bidirectional Type Conversion
**What:** Implement From/Into for native <-> FFI types
**When to use:** Host code when calling plugins and processing results
**Example:**
```rust
// Native -> FFI
impl From<&TodoItem> for FfiTodoItem {
    fn from(item: &TodoItem) -> Self {
        FfiTodoItem {
            id: item.id.to_string().into(),
            content: item.content.clone().into(),
            state: item.state.into(),
            priority: item.priority.map(Into::into).into(),
            due_date: item.due_date.map(|d| d.format("%Y-%m-%d").to_string().into()).into(),
            description: item.description.clone().map(Into::into).into(),
            parent_id: item.parent_id.map(|u| u.to_string().into()).into(),
            indent_level: item.indent_level as u32,
            created_at: item.created_at.timestamp_millis(),
            modified_at: item.modified_at.timestamp_millis(),
            completed_at: item.completed_at.map(|dt| dt.timestamp_millis()).into(),
        }
    }
}

// FFI -> Native (fallible due to parsing)
impl TryFrom<FfiTodoItem> for TodoItem {
    type Error = anyhow::Error;

    fn try_from(ffi: FfiTodoItem) -> Result<Self, Self::Error> {
        Ok(TodoItem {
            id: Uuid::parse_str(&ffi.id)?,
            content: ffi.content.into(),
            state: ffi.state.into(),
            // ... etc
        })
    }
}
```

### Anti-Patterns to Avoid
- **Using usize in FFI types:** Not FFI-safe on all platforms; use u32 or u64 explicitly
- **Passing &str or String directly:** Use RStr or RString instead
- **Exposing DateTime<Utc> directly:** Use i64 Unix timestamps instead
- **Forgetting #[repr(C)]:** Required for stable layout; derive macro will error without it
- **Using Option<T> directly:** Use ROption<T> for FFI safety
- **Panicking across FFI boundary:** Use catch_unwind at boundary or return RResult

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| FFI-safe String | `*const c_char` + len | `RString` | Memory management, UTF-8 validation built-in |
| FFI-safe Option | Custom enum | `ROption<T>` | Proper niche optimization, ergonomic conversion |
| FFI-safe Result | Custom enum | `RResult<T, E>` | Works with `?` via rtry! macro |
| FFI-safe Vec | `*const T` + len + cap | `RVec<T>` | Memory ownership, proper drop |
| Trait objects over FFI | Manual vtable | `#[sabi_trait]` | Automatic, type-safe, version-aware |
| Version checking | String comparison | `RootModule` trait | Automatic at load time, catches ABI drift |
| Panic handling | Manual catch | `extern_fn_panic_handling!` | Correct unwinding, AbortBomb safety |

**Key insight:** abi_stable's types use `ManuallyDrop` internally for O(1) conversions. Rolling your own means reinventing memory management and layout guarantees.

## Common Pitfalls

### Pitfall 1: usize in Struct Fields
**What goes wrong:** Compilation error or ABI mismatch on 32-bit vs 64-bit platforms
**Why it happens:** usize is platform-dependent, not FFI-safe
**How to avoid:** Use u32 or u64 explicitly. For indices, u32 is usually sufficient (4 billion items max)
**Warning signs:** StableAbi derive error mentioning "not FFI-safe"

### Pitfall 2: Panic Unwinding Across FFI
**What goes wrong:** Undefined behavior, potential crash
**Why it happens:** Rust panics can't safely unwind through FFI boundaries
**How to avoid:** Use `extern_fn_panic_handling!` macro or `std::panic::catch_unwind` at every extern "C" fn
**Warning signs:** Crash without error message when plugin throws panic

### Pitfall 3: DateTime/Timestamp Handling
**What goes wrong:** chrono's DateTime<Utc> is not FFI-safe
**Why it happens:** Internal representation is complex, not `#[repr(C)]`
**How to avoid:** Convert to i64 Unix timestamp (milliseconds) at boundary, reconstruct on other side
**Warning signs:** StableAbi derive error on DateTime field

### Pitfall 4: Version String Mismatch
**What goes wrong:** Plugin fails to load with "incompatible version" error
**Why it happens:** Interface crate version changed incompatibly
**How to avoid:** Follow semver strictly for interface crate. Major version = breaking ABI change
**Warning signs:** LibraryError::IncompatibleVersionNumber at runtime

### Pitfall 5: Forgetting #[sabi(last_prefix_field)]
**What goes wrong:** Cannot add new fields to struct in future versions without breaking ABI
**Why it happens:** Prefix types need marker for version-safe extension
**How to avoid:** Always mark the last field that's guaranteed in v1.0 with this attribute
**Warning signs:** None until you try to add a field and break existing plugins

### Pitfall 6: Non-UTF8 Strings from Plugins
**What goes wrong:** Panic or garbage data when converting RString to String
**Why it happens:** Plugin might provide invalid UTF-8 (especially if written in other languages later)
**How to avoid:** RString validates UTF-8 on construction; validate at boundary and return error
**Warning signs:** None until malformed data arrives

## Code Examples

Verified patterns from official sources:

### Complete FfiTodoState Enum with Conversions
```rust
// Source: Context decision + abi_stable docs
use abi_stable::StableAbi;
use crate::todo::state::TodoState;

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiTodoState {
    Empty = 0,
    Checked = 1,
    Question = 2,
    Exclamation = 3,
    InProgress = 4,
    Cancelled = 5,
}

impl From<TodoState> for FfiTodoState {
    fn from(state: TodoState) -> Self {
        match state {
            TodoState::Empty => FfiTodoState::Empty,
            TodoState::Checked => FfiTodoState::Checked,
            TodoState::Question => FfiTodoState::Question,
            TodoState::Exclamation => FfiTodoState::Exclamation,
            TodoState::InProgress => FfiTodoState::InProgress,
            TodoState::Cancelled => FfiTodoState::Cancelled,
        }
    }
}

impl From<FfiTodoState> for TodoState {
    fn from(state: FfiTodoState) -> Self {
        match state {
            FfiTodoState::Empty => TodoState::Empty,
            FfiTodoState::Checked => TodoState::Checked,
            FfiTodoState::Question => TodoState::Question,
            FfiTodoState::Exclamation => TodoState::Exclamation,
            FfiTodoState::InProgress => TodoState::InProgress,
            FfiTodoState::Cancelled => TodoState::Cancelled,
        }
    }
}
```

### Version Checking Function
```rust
// Source: https://docs.rs/abi_stable/latest/abi_stable/library/trait.RootModule.html
use abi_stable::library::{LibraryError, RootModule};
use semver::Version;

/// Check if plugin interface version is compatible with host
pub fn is_version_compatible(
    plugin_min_version: &str,
    host_version: &str,
) -> Result<bool, String> {
    let plugin_min = Version::parse(plugin_min_version)
        .map_err(|e| format!("Invalid plugin version: {}", e))?;
    let host = Version::parse(host_version)
        .map_err(|e| format!("Invalid host version: {}", e))?;

    // Compatible if same major and host >= plugin_min
    Ok(host.major == plugin_min.major && host >= plugin_min)
}

/// Load plugin with version check
pub fn load_plugin(path: &Path) -> Result<PluginModule_Ref, LibraryError> {
    let plugin = PluginModule_Ref::load_from_directory(path)?;

    // RootModule loading already validates abi_stable version
    // Add custom interface version check here
    let instance = (plugin.create_plugin())();
    let min_version = instance.min_interface_version();

    if !is_version_compatible(&min_version, env!("CARGO_PKG_VERSION"))? {
        // Return error or warning
    }

    Ok(plugin)
}
```

### Panic Handling at FFI Boundary
```rust
// Source: https://nullderef.com/blog/plugin-abi-stable/
use abi_stable::std_types::{RResult, RString};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Wrapper for calling plugin methods safely
pub fn call_plugin_generate(
    plugin: &Plugin_TO<'_, RBox<()>>,
    input: RString,
) -> RResult<RVec<FfiTodoItem>, RString> {
    // Catch any panics from the plugin
    let result = catch_unwind(AssertUnwindSafe(|| {
        plugin.generate(input)
    }));

    match result {
        Ok(r) => r,
        Err(panic_info) => {
            // Extract panic message if possible
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                format!("Plugin panicked: {}", s)
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                format!("Plugin panicked: {}", s)
            } else {
                "Plugin panicked with unknown error".to_string()
            };
            RResult::RErr(msg.into())
        }
    }
}
```

### String Length Validation
```rust
// Source: Context decision (max 64KB)
const MAX_STRING_LENGTH: usize = 64 * 1024;

/// Validate string length from plugin
pub fn validate_plugin_string(s: &RString) -> Result<(), String> {
    if s.len() > MAX_STRING_LENGTH {
        Err(format!(
            "String too long: {} bytes (max {})",
            s.len(),
            MAX_STRING_LENGTH
        ))
    } else {
        Ok(())
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| libloading + unsafe | abi_stable with type checking | 2019 (abi_stable 0.1) | Safe Rust-to-Rust FFI possible |
| Manual C ABI types | #[derive(StableAbi)] | abi_stable 0.5 | Automatic layout verification |
| Hand-written vtables | #[sabi_trait] | abi_stable 0.7 | Safe trait objects across FFI |
| WASM for plugins | Native with abi_stable | 2024 (user decision) | Better performance, simpler toolchain |

**Deprecated/outdated:**
- libloading alone: Still works but no type safety, requires unsafe
- abi_stable < 0.11: Use 0.11.3 for latest features and fixes
- async_ffi: Mentioned in sources but not needed for this phase (sync-only plugin calls)

## Open Questions

Things that couldn't be fully resolved:

1. **Collapsed field handling**
   - What we know: TodoItem has a `collapsed: bool` field for UI state
   - What's unclear: Should this be exposed to plugins or is it UI-only?
   - Recommendation: Exclude from FFI types initially; add if plugins need it

2. **deleted_at field**
   - What we know: TodoItem has deleted_at for soft deletes
   - What's unclear: Should plugins see deleted items?
   - Recommendation: Exclude from FFI; host filters out deleted items before passing to plugins

3. **Interface crate publishing**
   - What we know: Plugins need to depend on interface crate
   - What's unclear: Will interface be published to crates.io or git-only?
   - Recommendation: Use git dependency initially; publish when API stabilizes

## Sources

### Primary (HIGH confidence)
- [abi_stable docs](https://docs.rs/abi_stable/) - StableAbi derive, sabi_trait, std_types
- [abi_stable GitHub](https://github.com/rodrimati1992/abi_stable_crates) - Examples, README
- [RootModule trait](https://docs.rs/abi_stable/latest/abi_stable/library/trait.RootModule.html) - Version checking
- [sabi_trait attribute](https://docs.rs/abi_stable/latest/abi_stable/attr.sabi_trait.html) - Trait object FFI

### Secondary (MEDIUM confidence)
- [NullDeref: Plugins in Rust with abi_stable](https://nullderef.com/blog/plugin-abi-stable/) - Practical patterns, panic handling
- [crates.io API](https://crates.io/api/v1/crates/abi_stable) - Version 0.11.3 confirmed

### Tertiary (LOW confidence)
- WebSearch results on Rust ABI stability - General context only

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - abi_stable is the only viable option for native Rust plugins with stable ABI
- Architecture: HIGH - Three-crate pattern is documented and recommended by abi_stable
- Pitfalls: HIGH - Documented in official sources and community articles
- Version protocol: MEDIUM - Clear pattern but specifics of semver policy are implementation choice

**Research date:** 2026-01-24
**Valid until:** 2026-03-24 (60 days - abi_stable is stable, unlikely to change significantly)
