# Phase 8: Dynamic Loading - Research

**Researched:** 2026-01-25
**Domain:** Rust dynamic library loading, FFI panic safety, plugin lifecycle management
**Confidence:** HIGH

## Summary

This research covers loading native plugins (.so/.dylib/.dll) at runtime using the abi_stable crate's RootModule system. The key challenges are: (1) ensuring library lifetime management via proxy patterns, (2) catching plugin panics at FFI boundaries to prevent host crashes, and (3) never unloading plugins during app lifetime due to TLS (Thread Local Storage) safety issues.

The codebase already has the foundational abi_stable infrastructure in place from Phase 6-7: the `totui-plugin-interface` crate defines Plugin_TO trait objects and PluginModule as the RootModule entry point. Phase 8 focuses on the actual library loading, error handling, and UI feedback mechanisms.

**Primary recommendation:** Use `PluginModule_Ref::load_from_directory()` for loading, wrap all plugin calls in `std::panic::catch_unwind()` (already partially implemented in `call_plugin_generate`), store loaded plugins in a HashMap with Rc-wrapped library handles to ensure libraries outlive plugin objects, and intentionally leak libraries (abi_stable's default) to satisfy TLS safety requirements.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| [abi_stable](https://docs.rs/abi_stable/) | 0.11.3 | RootModule loading, library management | Already in use; provides load_from_directory, automatic version checking |
| [tracing-appender](https://docs.rs/tracing-appender/) | 0.2.x | File-based panic logging | Rotating file appender for panic backtraces |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | 0.1 | Already in Cargo.toml | Structured logging |
| tracing-subscriber | 0.3 | Already in Cargo.toml | Log formatting |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| abi_stable loading | libloading directly | More control but no type verification, requires manual version checks |
| tracing-appender | Custom file writer | Less features, would need to implement rotation |
| Leak libraries | Manual unload | TLS safety issues, undefined behavior risk |

**Installation:**
```toml
# Add to Cargo.toml
tracing-appender = "0.2"
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── plugin/
│   ├── mod.rs          # Re-exports
│   ├── manager.rs      # PluginManager (already exists) + dynamic loading
│   ├── manifest.rs     # PluginManifest (already exists)
│   ├── loader.rs       # NEW: PluginLoader with library lifetime management
│   ├── ffi_convert.rs  # Type conversions (already exists)
│   └── logging.rs      # NEW: Panic logging to file
├── ui/
│   └── components/
│       └── mod.rs      # Add plugin error popup
└── app/
    └── state.rs        # Add plugin loading state, error queue
```

### Pattern 1: Library Loading with PluginModule_Ref
**What:** Use abi_stable's RootModule pattern to load plugin libraries
**When to use:** Loading any .so/.dylib/.dll plugin at runtime
**Example:**
```rust
// Source: https://docs.rs/abi_stable/latest/abi_stable/library/trait.RootModule.html
use abi_stable::library::RootModule;
use totui_plugin_interface::{PluginModule_Ref, Plugin_TO};
use std::path::Path;

fn load_plugin_library(plugin_dir: &Path) -> Result<Plugin_TO<'static, RBox<()>>, LibraryError> {
    // load_from_directory handles:
    // 1. Platform-specific library naming (lib*.so, lib*.dylib, *.dll)
    // 2. abi_stable version compatibility check
    // 3. Library leaking (intentional for TLS safety)
    let module = PluginModule_Ref::load_from_directory(plugin_dir)?;

    // Call factory function to create plugin instance
    let plugin = (module.create_plugin())();

    Ok(plugin)
}
```

### Pattern 2: Proxy Pattern for Library Lifetime
**What:** Keep library handle alive as long as any plugin object exists
**When to use:** Managing loaded plugin instances
**Example:**
```rust
// Source: https://adventures.michaelfbryan.com/posts/plugins-in-rust/
use std::sync::Arc;
use abi_stable::library::RawLibrary;

/// Loaded plugin with library lifetime guarantee
pub struct LoadedPlugin {
    /// The plugin trait object
    pub plugin: Plugin_TO<'static, RBox<()>>,
    /// Library handle (kept alive via Arc)
    _library: Arc<LoadedLibrary>,
    /// Plugin name for display
    pub name: String,
    /// Whether plugin is disabled for this session (after panic)
    pub session_disabled: bool,
}

/// Wrapper for library handle with drop logging
struct LoadedLibrary {
    path: PathBuf,
    // Note: abi_stable intentionally leaks the library, so this is just for tracking
}

impl PluginLoader {
    pub fn load(&mut self, plugin_dir: &Path) -> Result<LoadedPlugin, LoadError> {
        let module = PluginModule_Ref::load_from_directory(plugin_dir)?;
        let plugin = (module.create_plugin())();

        let library = Arc::new(LoadedLibrary {
            path: plugin_dir.to_path_buf(),
        });

        let name = plugin.name().to_string();

        Ok(LoadedPlugin {
            plugin,
            _library: library,
            name,
            session_disabled: false,
        })
    }
}
```

### Pattern 3: Panic Catching at FFI Boundary
**What:** Wrap every plugin call in catch_unwind to prevent crashes
**When to use:** Any call into plugin code
**Example:**
```rust
// Source: https://nullderef.com/blog/plugin-abi-stable/
// Note: call_plugin_generate already exists in plugin.rs
use std::panic::{catch_unwind, AssertUnwindSafe};

pub fn call_plugin_safely<T, F>(
    plugin: &LoadedPlugin,
    f: F,
) -> Result<T, PluginError>
where
    F: FnOnce(&Plugin_TO<'_, RBox<()>>) -> T,
{
    if plugin.session_disabled {
        return Err(PluginError::SessionDisabled);
    }

    let result = catch_unwind(AssertUnwindSafe(|| f(&plugin.plugin)));

    match result {
        Ok(value) => Ok(value),
        Err(panic_info) => {
            // Extract panic message
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };

            // Log panic to file (always, not just with RUST_LOG)
            log_plugin_panic(&plugin.name, &msg, &panic_info);

            Err(PluginError::Panicked { plugin: plugin.name.clone(), message: msg })
        }
    }
}
```

### Pattern 4: Platform-Specific Library Naming
**What:** Handle .so/.dylib/.dll naming conventions
**When to use:** When locating plugin libraries in directories
**Example:**
```rust
// Source: https://docs.rs/abi_stable/latest/abi_stable/library/struct.RawLibrary.html
// Note: abi_stable handles this internally, but for reference:

#[cfg(target_os = "linux")]
const LIB_PREFIX: &str = "lib";
#[cfg(target_os = "linux")]
const LIB_SUFFIX: &str = ".so";

#[cfg(target_os = "macos")]
const LIB_PREFIX: &str = "lib";
#[cfg(target_os = "macos")]
const LIB_SUFFIX: &str = ".dylib";

#[cfg(target_os = "windows")]
const LIB_PREFIX: &str = "";
#[cfg(target_os = "windows")]
const LIB_SUFFIX: &str = ".dll";

/// Get expected library filename for a plugin
/// Plugin with BASE_NAME "totui_plugin" would be:
/// - Linux: libtotui_plugin.so
/// - macOS: libtotui_plugin.dylib
/// - Windows: totui_plugin.dll
fn get_library_filename(base_name: &str) -> String {
    format!("{}{}{}", LIB_PREFIX, base_name, LIB_SUFFIX)
}
```

### Pattern 5: Error Collection and Popup Display
**What:** Collect loading failures, show combined popup at startup
**When to use:** TUI startup after plugin discovery
**Example:**
```rust
// In app/state.rs
pub struct AppState {
    // ... existing fields ...

    /// Plugin loading errors to display on first render
    pub pending_plugin_errors: Vec<PluginLoadError>,
    /// Whether to show plugin error popup
    pub show_plugin_error_popup: bool,
}

pub struct PluginLoadError {
    pub plugin_name: String,
    pub error_kind: PluginErrorKind,
    pub message: String,
}

pub enum PluginErrorKind {
    VersionMismatch { required: String, actual: String },
    LibraryCorrupted,
    SymbolMissing,
    Other,
}

// In startup code
fn load_all_plugins(manager: &PluginManager) -> (Vec<LoadedPlugin>, Vec<PluginLoadError>) {
    let mut loaded = Vec::new();
    let mut errors = Vec::new();

    for info in manager.enabled_plugins() {
        match load_plugin_library(&info.path) {
            Ok(plugin) => loaded.push(plugin),
            Err(e) => errors.push(PluginLoadError::from_library_error(&info.manifest.name, e)),
        }
    }

    (loaded, errors)
}
```

### Anti-Patterns to Avoid
- **Unloading libraries:** Never call dlclose/FreeLibrary on plugin libraries - TLS safety issues
- **Ignoring load failures silently:** User must know when plugins fail (per CONTEXT.md)
- **Catching panics without logging:** Always log panics to file with backtrace
- **Trusting plugin version self-report:** Verify with abi_stable before calling any methods
- **Re-enabling panicked plugins:** Keep disabled for session (per CONTEXT.md)

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Library loading | dlopen/dlsym directly | PluginModule_Ref::load_from_directory | Handles naming, version checking, leaking |
| Version checking | String comparison | abi_stable's automatic check + is_version_compatible | Semantic version parsing built-in |
| Panic message extraction | Custom parsing | downcast_ref<&str> / downcast_ref<String> | Standard pattern, catches both forms |
| Platform library names | cfg! macros | abi_stable RawLibrary::path_in_directory | Already implemented, tested |
| File logging | println! to file | tracing-appender | Non-blocking, rotation, structured |
| Popup UI | Custom widget | Existing overlay pattern in ui/components | render_*_overlay patterns established |

**Key insight:** abi_stable intentionally leaks libraries to avoid TLS destruction issues. This is not a bug but a feature - accept it and never try to work around it.

## Common Pitfalls

### Pitfall 1: Attempting to Unload Libraries
**What goes wrong:** Use-after-free, TLS destructor crashes, undefined behavior
**Why it happens:** TLS destructors registered via __cxa_thread_atexit_impl prevent safe unloading
**How to avoid:** Never unload plugins during app lifetime. abi_stable leaks by default - accept this.
**Warning signs:** Crash on exit, memory corruption after plugin "unload"

### Pitfall 2: Not Catching Panics at Every Entry Point
**What goes wrong:** Plugin panic crashes the entire host application
**Why it happens:** Rust panics are undefined behavior across extern "C" boundaries
**How to avoid:** Wrap every plugin call in catch_unwind, including name(), version(), etc.
**Warning signs:** Host crashes when calling any plugin method

### Pitfall 3: Silent Loading Failures
**What goes wrong:** User doesn't know why plugin isn't working
**Why it happens:** Errors swallowed without notification
**How to avoid:** Collect all errors, show combined popup (per CONTEXT.md decisions)
**Warning signs:** Plugin listed but never called, no error shown

### Pitfall 4: Loading Before TUI Renders
**What goes wrong:** App appears frozen during slow plugin loads
**Why it happens:** Blocking on load before first render
**How to avoid:** Render TUI first, show loading progress in status bar (per CONTEXT.md)
**Warning signs:** Black screen for several seconds at startup

### Pitfall 5: Not Logging Panic Backtraces
**What goes wrong:** Plugin panics but no way to debug
**Why it happens:** Only logging when RUST_LOG is set
**How to avoid:** Always log panics to file with backtrace (per CONTEXT.md)
**Warning signs:** "Plugin panicked" message but no details in logs

### Pitfall 6: Platform Library Name Mismatch
**What goes wrong:** Plugin not found on some platforms
**Why it happens:** Hardcoded .so when running on macOS/Windows
**How to avoid:** Use abi_stable's built-in path resolution
**Warning signs:** Works on Linux, fails on macOS with "library not found"

## Code Examples

Verified patterns from official sources:

### Complete Plugin Loading Flow
```rust
// Source: abi_stable docs + CONTEXT.md decisions
use abi_stable::library::{LibraryError, RootModule};
use totui_plugin_interface::{PluginModule_Ref, Plugin_TO, INTERFACE_VERSION};
use std::path::Path;

pub struct PluginLoader {
    loaded: HashMap<String, LoadedPlugin>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self { loaded: HashMap::new() }
    }

    pub fn load_all(&mut self, manager: &PluginManager) -> Vec<PluginLoadError> {
        let mut errors = Vec::new();

        for info in manager.enabled_plugins() {
            match self.load_plugin(&info.path, &info.manifest) {
                Ok(plugin) => {
                    self.loaded.insert(info.manifest.name.to_lowercase(), plugin);
                }
                Err(e) => errors.push(e),
            }
        }

        errors
    }

    fn load_plugin(&self, path: &Path, manifest: &PluginManifest) -> Result<LoadedPlugin, PluginLoadError> {
        // 1. Load library via abi_stable
        let module = PluginModule_Ref::load_from_directory(path)
            .map_err(|e| self.map_library_error(&manifest.name, e))?;

        // 2. Create plugin instance
        let plugin = (module.create_plugin())();

        // 3. Verify interface version
        let min_version = plugin.min_interface_version().to_string();
        if !is_version_compatible(&min_version, INTERFACE_VERSION)? {
            return Err(PluginLoadError {
                plugin_name: manifest.name.clone(),
                error_kind: PluginErrorKind::VersionMismatch {
                    required: min_version,
                    actual: INTERFACE_VERSION.to_string(),
                },
                message: format!(
                    "Plugin {} requires to-tui {}+, you have {}",
                    manifest.name, min_version, INTERFACE_VERSION
                ),
            });
        }

        Ok(LoadedPlugin {
            plugin,
            name: manifest.name.clone(),
            session_disabled: false,
        })
    }

    fn map_library_error(&self, name: &str, error: LibraryError) -> PluginLoadError {
        match error {
            LibraryError::OpenError { .. } => PluginLoadError {
                plugin_name: name.to_string(),
                error_kind: PluginErrorKind::LibraryCorrupted,
                message: format!("Plugin {} failed to load - may be corrupted or incompatible", name),
            },
            LibraryError::GetSymbolError { .. } => PluginLoadError {
                plugin_name: name.to_string(),
                error_kind: PluginErrorKind::SymbolMissing,
                message: format!("Plugin {} failed to load - may be corrupted or incompatible", name),
            },
            LibraryError::IncompatibleVersionNumber { expected, found, .. } => PluginLoadError {
                plugin_name: name.to_string(),
                error_kind: PluginErrorKind::VersionMismatch {
                    required: expected.to_string(),
                    actual: found.to_string(),
                },
                message: format!("Plugin {} requires to-tui {}+, you have {}", name, expected, found),
            },
            _ => PluginLoadError {
                plugin_name: name.to_string(),
                error_kind: PluginErrorKind::Other,
                message: format!("Plugin {} failed to load - may be corrupted or incompatible", name),
            },
        }
    }
}
```

### Panic Logging Setup
```rust
// Source: tracing-appender docs + CONTEXT.md decisions
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::prelude::*;
use std::path::PathBuf;

/// Initialize panic logging to file
/// Called at startup before loading any plugins
pub fn init_plugin_panic_logger(log_dir: PathBuf) -> std::io::Result<()> {
    // Create daily rotating log file
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("plugin-panics")
        .filename_suffix("log")
        .max_log_files(7) // Keep 7 days
        .build(&log_dir)?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Create file-only layer for panic logging
    let file_layer = Layer::new()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Combine with existing subscriber
    tracing_subscriber::registry()
        .with(file_layer)
        .init();

    Ok(())
}

/// Log a plugin panic with full backtrace
pub fn log_plugin_panic(plugin_name: &str, message: &str, panic_info: &Box<dyn std::any::Any + Send>) {
    let backtrace = std::backtrace::Backtrace::force_capture();

    tracing::error!(
        plugin = %plugin_name,
        message = %message,
        backtrace = %backtrace,
        "Plugin panicked during execution"
    );
}
```

### Error Popup UI Component
```rust
// Source: Existing overlay patterns in ui/components/mod.rs
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_plugin_error_popup(f: &mut Frame, errors: &[PluginLoadError]) {
    if errors.is_empty() {
        return;
    }

    let area = f.area();

    // Center popup, 60% width, height based on error count
    let popup_width = (area.width * 60) / 100;
    let popup_height = (errors.len() as u16 * 2 + 6).min(area.height - 4);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height - popup_height) / 2),
            Constraint::Length(popup_height),
            Constraint::Min(0),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width - popup_width) / 2),
            Constraint::Length(popup_width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1];

    // Clear background
    f.render_widget(Clear, popup_area);

    // Build error text
    let mut lines = vec![
        Line::from(Span::styled(
            format!("{} plugin(s) failed to load:", errors.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for error in errors {
        lines.push(Line::from(vec![
            Span::styled("  - ", Style::default().fg(Color::Red)),
            Span::styled(&error.plugin_name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(": "),
            Span::raw(&error.message),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Run `totui plugin status` for details",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press any key to dismiss",
        Style::default().fg(Color::Yellow),
    )));

    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .title(" Plugin Loading Errors ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, popup_area);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| dlopen/dlsym | abi_stable RootModule | 2019 | Type-safe loading with version checks |
| Manual library tracking | abi_stable leaks libraries | abi_stable 0.5+ | TLS safety guaranteed |
| Simple file logging | tracing-appender | 2022 | Non-blocking, rotating logs |
| Panic = abort | catch_unwind + continue | Rust 1.9 | Graceful degradation |

**Deprecated/outdated:**
- libloading alone without abi_stable: Works but no type verification
- Attempting to unload libraries: TLS safety issues, undefined behavior

## Open Questions

Things that couldn't be fully resolved:

1. **Exact status bar format during loading**
   - What we know: Show per-plugin loading progress (per CONTEXT.md)
   - What's unclear: Exact format (spinner, progress bar, plugin name)
   - Recommendation: Claude's discretion per CONTEXT.md

2. **Log file location**
   - What we know: Always log panics to file (per CONTEXT.md)
   - What's unclear: Exact path (~/.local/share/to-tui/logs/ vs ~/.cache/to-tui/)
   - Recommendation: Use ~/.local/share/to-tui/logs/ for consistency with other data

3. **Log rotation policy**
   - What we know: Need rotating logs (tracing-appender supports this)
   - What's unclear: How many days to keep, max file size
   - Recommendation: Keep 7 days, daily rotation (Claude's discretion)

## Sources

### Primary (HIGH confidence)
- [abi_stable RootModule](https://docs.rs/abi_stable/latest/abi_stable/library/trait.RootModule.html) - load_from_directory, library lifecycle
- [abi_stable LibraryError](https://docs.rs/abi_stable/latest/abi_stable/library/enum.LibraryError.html) - Error variants and handling
- [Rust panic::catch_unwind](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html) - FFI panic safety
- [tracing-appender docs](https://docs.rs/tracing-appender/latest/tracing_appender/) - Rolling file appender

### Secondary (MEDIUM confidence)
- [NullDeref: Plugins with abi_stable](https://nullderef.com/blog/plugin-abi-stable/) - Practical patterns, panic handling
- [Michael Bryan: Plugins in Rust](https://adventures.michaelfbryan.com/posts/plugins-in-rust/) - Proxy pattern for lifetime
- [Rust TLS issue #59629](https://github.com/rust-lang/rust/issues/59629) - Why library unloading is unsafe

### Tertiary (LOW confidence)
- WebSearch on library loading patterns - General context only

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - abi_stable 0.11 is already in use, tracing-appender is well-documented
- Architecture: HIGH - Loading pattern is documented, existing code provides good foundation
- Pitfalls: HIGH - TLS issues are well-documented, panic handling is standard
- UI patterns: MEDIUM - Following existing overlay patterns, but popup design is new

**Research date:** 2026-01-25
**Valid until:** 2026-03-25 (60 days - stable domain, unlikely to change)
