# Pitfalls Research: Dynamic Plugin System

**Domain:** Rust dynamic plugin loading for to-tui
**Researched:** 2026-01-24
**Overall Confidence:** HIGH (verified via official documentation and authoritative sources)

## Executive Summary

Adding dynamic plugin loading to Rust applications is deceptively difficult. Rust's intentionally unstable ABI means that even minor compiler version differences between host and plugin can cause undefined behavior. The pitfalls fall into five categories: ABI stability, memory/lifetime management, cross-platform issues, panic handling, and security. Most failures manifest as silent memory corruption or segfaults rather than helpful error messages.

---

## Critical Pitfalls

These mistakes cause crashes, undefined behavior, or require major rewrites.

### 1. Assuming Rust Has a Stable ABI

**What happens**: Plugin compiled with Rust 1.75 is loaded into host compiled with Rust 1.76. Memory layouts differ silently. Fields are read from wrong offsets. Data corruption occurs, possibly without immediate crash.

**Warning signs**:
- Struct fields contain garbage values after loading
- Segfaults that only occur with certain plugin/host version combinations
- "Works on my machine" but fails in CI or for users

**Prevention**:
- Use `#[repr(C)]` for ALL types crossing FFI boundary
- Use `abi_stable` crate for automatic layout verification at load time
- Document and enforce exact Rust version matching (less flexible)
- Consider `cdylib` crate type with explicit C ABI exports

**Phase**: Must be addressed in Phase 1 (Plugin Trait Design). The trait and all shared types must be designed for ABI stability from the start.

**Sources**:
- [abi_stable crate documentation](https://docs.rs/abi_stable/)
- [Rust RFC on stable ABI](https://github.com/rust-lang/rfcs/issues/3075)

---

### 2. Library Outliving Its Contents (Use-After-Free)

**What happens**: Plugin returns a trait object or data structure. Host stores reference. Plugin library is dropped. Host calls method on stale vtable. Segfault.

**Warning signs**:
- Crashes on second or third plugin method call
- Crashes only after plugin reload
- Crashes that happen "randomly" during normal operation

**Prevention**:
- Use the Proxy Pattern: wrap all plugin objects so they hold a reference to the library
- Never extract plugin function pointers into separate variables
- Store `Library` alongside all `Symbol`s in the same struct with appropriate lifetimes
- Design API so plugins cannot outlive their library (enforce at type level)

**Phase**: Must be addressed in Phase 1-2. The plugin loading infrastructure must prevent this structurally.

**Example of the trap** (from [NullDeref](https://nullderef.com/blog/plugin-dynload/)):
```rust
// DANGEROUS: library will be dropped, leaving dangling vtable
fn load_plugin() -> Box<dyn Plugin> {
    let library = Library::new("plugin.so").unwrap();
    let create: Symbol<fn() -> Box<dyn Plugin>> = library.get(b"create_plugin").unwrap();
    create()  // library dropped here, vtable now invalid
}
```

---

### 3. Type Signature Mismatch (Silent UB)

**What happens**: Plugin exports function with signature `fn(i32) -> i32`. Host loads it as `fn(i64) -> i64`. No compile-time check. No runtime check. Garbage in, garbage out.

**Warning signs**:
- Return values are always zero or garbage
- Arguments seem to be "shifted" or corrupted
- Crashes in release but not debug (or vice versa)

**Prevention**:
- Use `abi_stable`'s load-time type checking
- Implement manual version/signature verification protocol
- Export a schema or type descriptor that can be validated before calling functions
- Keep exported function signatures minimal and stable

**Phase**: Phase 1 (Plugin Trait Design) - design verification protocol into the loading sequence.

---

### 4. Panic Across FFI Boundary

**What happens**: Plugin code panics. Panic attempts to unwind through C ABI boundary. Undefined behavior. Usually immediate abort, but could corrupt memory silently.

**Warning signs**:
- Application aborts with "panic in a function that cannot unwind"
- Crashes with no stack trace or error message
- Different behavior between `panic = "unwind"` and `panic = "abort"`

**Prevention**:
- Wrap ALL exported functions in `std::panic::catch_unwind`
- Use `abi_stable` which handles this automatically via "AbortBomb"
- Design plugin API to return `Result` types for all operations
- Test plugins with intentional panics to verify handling

**Phase**: Phase 1-2. Must be baked into the plugin export macros/conventions.

**Code pattern** (from [Rustonomicon](https://doc.rust-lang.org/nomicon/unwinding.html)):
```rust
#[no_mangle]
pub extern "C" fn plugin_function() -> i32 {
    match std::panic::catch_unwind(|| {
        // actual implementation
        do_work()
    }) {
        Ok(result) => result,
        Err(_) => -1, // or some error sentinel
    }
}
```

---

### 5. Thread Local Storage (TLS) and Unloading

**What happens**: Plugin uses `thread_local!` or any crate that does internally. Plugin is unloaded with `dlclose`. Thread later exits and tries to run TLS destructor. Destructor code no longer exists. Segfault.

**Warning signs**:
- Crashes on program exit (not during normal operation)
- Crashes specifically when threads terminate after plugin unload
- Works fine if you never unload plugins
- Platform-specific: especially problematic on macOS and older glibc

**Prevention**:
- **Best option: Don't unload plugins** - `abi_stable` explicitly doesn't support unloading
- If unloading required, audit all plugin dependencies for TLS usage
- Use `relib` crate designed specifically for safe unloading
- Keep plugins loaded for entire application lifetime

**Phase**: Phase 2 (Plugin Loading). Design decision: support unloading or not?

**Sources**:
- [Rust issue #52138](https://github.com/rust-lang/rust/issues/52138)
- [Rust issue #28794](https://github.com/rust-lang/rust/issues/28794)
- [Relib announcement](https://users.rust-lang.org/t/relib-reloadable-dynamic-libraries-without-memory-leaks/122786)

---

## ABI Stability Pitfalls

These are Rust-specific issues stemming from the lack of a stable ABI.

### 6. Using `dylib` Instead of `cdylib`

**What happens**: Plugin uses `crate-type = ["dylib"]`. This produces a Rust dynamic library that can only be loaded by the exact same compiler version. Host compiled with different rustc version fails to link or crashes mysteriously.

**Warning signs**:
- "cannot satisfy dependencies so `std` only shows up once" errors
- Plugin works locally but fails for users
- Upgrading rustc breaks all existing plugins

**Prevention**:
- Use `crate-type = ["cdylib"]` for plugins
- Document this requirement clearly for plugin authors
- Provide plugin template/scaffold with correct configuration

**Phase**: Phase 1 (Plugin Trait Design) - establish correct crate type from start.

**Sources**:
- [Rust Linkage Reference](https://doc.rust-lang.org/reference/linkage.html)
- [cdylib RFC](https://rust-lang.github.io/rfcs/1510-cdylib.html)

---

### 7. Exposing Rust Generics in Plugin Interface

**What happens**: Plugin trait uses generics like `fn process<T: Serialize>(&self, data: T)`. Generics require monomorphization at compile time. Plugin was not compiled with host's types. Linker error or missing symbol at runtime.

**Warning signs**:
- "undefined symbol" errors mentioning mangled generic names
- Plugin compiles but host can't find expected functions
- Works for some types but not others

**Prevention**:
- No generics in FFI boundary - use concrete types only
- Use trait objects with FFI-safe vtables (via `abi_stable`'s `sabi_trait`)
- Pre-monomorphize common types if needed

**Phase**: Phase 1 (Plugin Trait Design) - design trait without generics.

---

### 8. Using Standard Library Types Directly

**What happens**: Plugin returns `Vec<String>` or `HashMap`. These types have no stable ABI. Memory layout can change between Rust versions. Host reads garbage or crashes.

**Warning signs**:
- Collection lengths are wrong
- String contents are corrupted
- Works in debug, crashes in release (or vice versa)

**Prevention**:
- Use `abi_stable`'s `RVec`, `RString`, `RHashMap` etc.
- Or define C-compatible structs with explicit `#[repr(C)]`
- Never pass `String`, `Vec`, `Box`, `Option`, `Result` directly across FFI

**Phase**: Phase 1 - all shared types must be FFI-safe.

---

### 9. Trait Objects Across FFI

**What happens**: Plugin returns `Box<dyn MyTrait>`. Trait objects have two pointers: data and vtable. Neither has stable ABI. Vtable layout varies by compiler.

**Warning signs**:
- Method calls invoke wrong methods
- First call works, subsequent calls crash
- Different behavior on different platforms

**Prevention**:
- Use `abi_stable`'s `#[sabi_trait]` macro
- Or implement manual vtable with explicit function pointers
- Or avoid trait objects entirely, use enum-based dispatch

**Phase**: Phase 1 - trait design must account for this.

**Sources**:
- [abi_stable sabi_trait docs](https://docs.rs/abi_stable/latest/abi_stable/attr.sabi_trait.html)

---

## Cross-Platform Pitfalls

Platform-specific issues that cause "works on my machine" problems.

### 10. Hardcoding Library Extensions

**What happens**: Code loads `"plugin.so"`. Works on Linux. Fails on macOS (expects `.dylib`) and Windows (expects `.dll`).

**Warning signs**:
- "library not found" on specific platforms
- Tests pass on CI but fail locally (or vice versa)

**Prevention**:
- Use `libloading::library_filename()` to get platform-correct name
- Or use conditional compilation with `cfg` attributes
- Store plugin names without extension, add programmatically

**Phase**: Phase 2 (Plugin Loading) - straightforward to address.

**Code pattern**:
```rust
let lib_name = libloading::library_filename("myplugin");
// Returns "libmyplugin.so" on Linux, "libmyplugin.dylib" on macOS, "myplugin.dll" on Windows
```

---

### 11. Windows Symbol Export Requirements

**What happens**: Plugin compiles on Windows but functions aren't exported. `GetProcAddress` returns null. Host reports "undefined symbol".

**Warning signs**:
- Plugin works on Linux/macOS but not Windows
- `nm` or `dumpbin` shows no exported symbols
- Functions are marked `pub` and `extern "C"` but still missing

**Prevention**:
- Use `#[no_mangle]` on all exported functions
- On Windows, may also need `__declspec(dllexport)` or a `.def` file
- Test symbol visibility with `nm -D` (Unix) or `dumpbin /exports` (Windows)

**Phase**: Phase 2-3 (Cross-platform testing).

---

### 12. macOS Code Signing for Dynamic Libraries

**What happens**: Plugin loads successfully once. Hot reload attempted. macOS refuses to load unsigned modified library. Cryptic error or silent failure.

**Warning signs**:
- Hot reload works on Linux but not macOS
- "code signature invalid" errors in Console.app
- Plugin loads first time but not after modification

**Prevention**:
- Use `codesign` to sign plugins after compilation
- `hot-lib-reloader` handles this automatically
- For development, use `codesign -s -` for ad-hoc signing
- Document signing requirements for plugin authors

**Phase**: Phase 2-3 (if hot reload supported).

**Sources**:
- [hot-lib-reloader docs](https://docs.rs/hot-lib-reloader/latest/hot_lib_reloader/)

---

### 13. Path Handling Differences

**What happens**: Plugin paths work on Unix but fail on Windows due to backslash vs forward slash, drive letters, or UNC paths.

**Warning signs**:
- "file not found" on Windows with correct relative path
- Paths with spaces fail on some platforms
- Network paths fail entirely

**Prevention**:
- Use `std::path::PathBuf` consistently
- Use `canonicalize()` for absolute paths
- Test with paths containing spaces and special characters
- Handle Windows UNC paths if network storage is supported

**Phase**: Phase 2-3.

---

## Security Pitfalls

These create vulnerabilities when loading third-party plugins.

### 14. No Plugin Verification

**What happens**: User downloads plugin from untrusted source. Plugin contains malware. Host loads and executes it with full process privileges. System compromised.

**Warning signs**:
- N/A - this is a design flaw, not a runtime symptom

**Prevention**:
- Implement checksum verification (SHA-256 minimum)
- Sign plugins cryptographically and verify signatures
- Maintain a trusted plugin registry
- Warn users about untrusted plugins
- Consider sandboxing (see below)

**Phase**: Phase 3-4 (Registry and distribution).

---

### 15. Plugins Run with Full Process Privileges

**What happens**: Plugin code can access filesystem, network, and all resources the host process can. Malicious or buggy plugin can delete files, exfiltrate data, or cause denial of service.

**Warning signs**:
- N/A - architectural issue

**Prevention**:
- **Accept the risk**: For a personal todo app, full trust may be acceptable
- **Document clearly**: Make users aware plugins are trusted code
- **Consider WASM**: WebAssembly plugins can be sandboxed with explicit permissions
- **OS-level isolation**: Run plugin code in separate process with restricted permissions

**Phase**: Architectural decision in Phase 1. WASM would be Phase 4+.

**Sources**:
- [wasm_sandbox crate](https://docs.rs/wasm-sandbox/latest/wasm_sandbox/)
- [wasmtime for Rust](https://docs.rs/wasmtime/)

---

### 16. Supply Chain Attacks via Plugin Registry

**What happens**: Attacker compromises plugin in registry. Users auto-update to malicious version. Wide-scale compromise.

**Warning signs**:
- Sudden version bump with minimal changelog
- Plugin author account compromised
- Checksum changed without version change

**Prevention**:
- Pin specific plugin versions by default
- Require explicit user action for updates
- Show changelogs before updating
- Maintain audit trail of plugin versions
- Consider reproducible builds verification

**Phase**: Phase 3-4 (Registry design).

---

## Moderate Pitfalls

These cause development friction or subtle bugs but are recoverable.

### 17. Forgetting `#[no_mangle]`

**What happens**: Plugin exports `pub extern "C" fn create_plugin()`. Rust mangles the name to something like `_ZN6plugin13create_plugin17h4e8c3b2a1d5f6g7hE`. Host looks for `create_plugin`. Not found.

**Warning signs**:
- "undefined symbol: create_plugin" error
- Symbol exists but with mangled name (visible via `nm`)

**Prevention**:
- Always use `#[no_mangle]` with `extern "C"`
- Create macro that enforces correct attributes
- Test symbol visibility as part of plugin compilation

**Phase**: Phase 1-2. Plugin author education.

---

### 18. Version Mismatch Without Detection

**What happens**: Host expects plugin API v2. Plugin implements API v1. No version check. Host calls method that doesn't exist. Crash or garbage.

**Warning signs**:
- Plugins that "used to work" fail after host update
- Crashes in specific plugin methods, not others
- User reports differ based on plugin age

**Prevention**:
- Export version constant from all plugins
- Check version compatibility at load time, before any method calls
- Use semantic versioning with clear compatibility rules
- `abi_stable` provides automatic version checking

**Phase**: Phase 1 (Plugin Trait Design) - build into loading protocol.

**Example**:
```rust
// Plugin exports
#[no_mangle]
pub static PLUGIN_API_VERSION: u32 = 2;

// Host checks
let version: Symbol<*const u32> = lib.get(b"PLUGIN_API_VERSION")?;
if unsafe { *version } != EXPECTED_VERSION {
    return Err("incompatible plugin version");
}
```

---

### 19. State Leakage Between Plugin Loads

**What happens**: Plugin is unloaded. Static variables in plugin retain values in memory. Plugin reloaded at same address. Old state persists unexpectedly.

**Warning signs**:
- Plugin behavior differs on first load vs reload
- Counter variables don't reset
- Cached data from previous instance appears

**Prevention**:
- Explicitly initialize all plugin state on load
- Call cleanup function before unload
- Don't rely on static initialization order
- Test plugin reload sequences

**Phase**: Phase 2 (if supporting plugin reload).

---

### 20. Missing Symbol Discovery

**What happens**: Plugin doesn't export required function. Host calls `lib.get()`. Returns error but error is ignored or poorly handled. Null function pointer is called. Crash.

**Warning signs**:
- Crashes immediately after loading new plugin
- "expected symbol X" in logs
- Works with some plugins but not others

**Prevention**:
- Verify all required symbols exist before returning "loaded" status
- Use Result types properly, don't unwrap Symbol loads
- Provide clear error messages listing missing symbols
- Consider required symbol list in plugin metadata

**Phase**: Phase 2 (Plugin Loading).

---

## Minor Pitfalls

These cause annoyance but are easily fixable.

### 21. Debug vs Release Build Mismatch

**What happens**: Host is release build. Plugin is debug build. ABI may differ (especially with debug assertions affecting layout). Subtle corruption.

**Warning signs**:
- Works in development, fails in production
- Performance is unexpectedly slow (debug plugin)
- Assertion failures inside plugin code

**Prevention**:
- Document build requirements for plugins
- Detect and warn about debug/release mismatch
- Distribute release-built plugins only

**Phase**: Phase 3-4 (Documentation and distribution).

---

### 22. Hot Reload with TypeId Breakage

**What happens**: Plugin uses `TypeId` for type identification (common in ECS systems like Bevy). Plugin reloaded. `TypeId` changes because it's based on compiler internals. Type system becomes inconsistent.

**Warning signs**:
- Type lookups fail after reload
- Components "disappear" from entities
- Deserialization fails with "unknown type" errors

**Prevention**:
- Don't use `TypeId` across plugin boundary
- Use stable string identifiers for type registration
- Or don't support hot reload with TypeId-dependent systems

**Phase**: Phase 2 (if supporting hot reload).

**Sources**:
- [hot-lib-reloader TypeId issue](https://github.com/rksm/hot-lib-reloader-rs)

---

### 23. Async Not Directly Supported

**What happens**: Plugin trait has `async fn`. This isn't FFI-safe. Compilation fails or runtime misbehavior.

**Warning signs**:
- Compiler errors about async traits not being object-safe
- "Future is not FFI-safe" warnings

**Prevention**:
- Use synchronous API at plugin boundary
- Use `async_ffi` crate if async is required
- Have plugin return poll-based state machine instead

**Phase**: Phase 1 (Trait Design) - decide sync vs async upfront.

---

## Phase-Specific Risk Summary

| Phase | High-Risk Pitfalls | Mitigation Focus |
|-------|-------------------|------------------|
| Phase 1: Trait Design | #1, #3, #6, #7, #8, #9 | Design FFI-safe trait with abi_stable, no generics, concrete types |
| Phase 2: Loading | #2, #4, #5, #10, #17, #18, #20 | Implement proxy pattern, panic handling, version checks |
| Phase 3: Cross-Platform | #11, #12, #13, #21 | Test on all platforms, handle symbol export, signing |
| Phase 4: Registry | #14, #15, #16 | Implement checksum verification, security warnings |

---

## Recommendations for to-tui

Given to-tui's context (personal todo app, existing `TodoGenerator` trait, adding GitHub plugin downloads):

1. **Use abi_stable**: Worth the complexity. It solves #1, #3, #4, #8, #9, #18 automatically.

2. **Don't support plugin unloading**: Avoids #5 entirely. Plugins stay loaded for app lifetime.

3. **Design new FFI trait**: The existing `TodoGenerator` trait uses `Result`, `Vec<TodoItem>`, `String` - none FFI-safe. Create parallel FFI-safe trait for dynamic plugins.

4. **TodoItem challenge**: Your `TodoItem` struct contains `Uuid`, `DateTime`, `Option`, `String` - all non-FFI-safe. Need FFI-safe equivalent with conversion functions.

5. **Checksum verification for downloads**: Implement SHA-256 verification for GitHub releases.

6. **Clear documentation**: Plugin authors need to know: use cdylib, specific Rust version, required exports.

---

## Sources

### Official Documentation
- [libloading docs](https://docs.rs/libloading/latest/libloading/)
- [abi_stable docs](https://docs.rs/abi_stable/latest/abi_stable/)
- [Rust Linkage Reference](https://doc.rust-lang.org/reference/linkage.html)
- [Rustonomicon - FFI](https://doc.rust-lang.org/nomicon/ffi.html)
- [Rustonomicon - Unwinding](https://doc.rust-lang.org/nomicon/unwinding.html)

### Community Resources
- [Plugins in Rust: Dynamic Loading - NullDeref](https://nullderef.com/blog/plugin-dynload/)
- [Plugins in Rust: Reducing Pain with abi_stable - NullDeref](https://nullderef.com/blog/plugin-abi-stable/)
- [Plugins in Rust - Michael Bryan](https://adventures.michaelfbryan.com/posts/plugins-in-rust/)
- [Hot Reloading Rust - Robert Krahn](https://robert.kra.hn/posts/hot-reloading-rust/)

### Issue Trackers
- [Rust #52138: Unloading SOs can segfault](https://github.com/rust-lang/rust/issues/52138)
- [Rust #28794: Unloading dylib with TLS segfaults on macOS](https://github.com/rust-lang/rust/issues/28794)
- [RFC #3075: Stable Rust ABI discussion](https://github.com/rust-lang/rfcs/issues/3075)

### Alternative Approaches
- [stabby crate](https://github.com/ZettaScaleLabs/stabby) - alternative to abi_stable
- [relib crate](https://users.rust-lang.org/t/relib-reloadable-dynamic-libraries-without-memory-leaks/122786) - for safe unloading
- [hot-lib-reloader](https://docs.rs/hot-lib-reloader/latest/hot_lib_reloader/) - for development hot reload
