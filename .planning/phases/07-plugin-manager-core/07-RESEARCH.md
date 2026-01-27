# Phase 7: Plugin Manager Core - Research

**Researched:** 2026-01-24
**Domain:** Plugin discovery, manifest parsing, configuration management, lifecycle state
**Confidence:** HIGH

## Summary

This phase implements the PluginManager that discovers plugins from disk, parses their TOML manifests, tracks enable/disable state, and reports availability status. The codebase already uses the `toml` crate (v0.9) and `serde` for configuration parsing, so we extend these patterns for plugin manifests.

Key architectural insight: The existing `ProjectRegistry` pattern in `src/project/registry.rs` provides an excellent template for the PluginManager - both are registries that load from disk at startup, track entities in memory, and support CRUD operations. The existing `Config` struct in `src/config.rs` already uses TOML serialization with `#[serde(default)]` patterns that work for plugin enable/disable state.

**Primary recommendation:** Create a `PluginManager` in `src/plugin/manager.rs` following the ProjectRegistry pattern. Extend the existing `Config` struct with a `plugins` section for enabled/disabled state. Add CLI subcommands under `totui plugin <list|enable|disable|status>`.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| [toml](https://docs.rs/toml/) | 0.9 | TOML manifest parsing | Already in use for config.toml; serde integration |
| [serde](https://serde.rs/) | 1.0 | Serialization/deserialization | Already in use; handles defaults, optional fields |
| [semver](https://docs.rs/semver/) | 1.0 | Version parsing and comparison | Already in totui-plugin-interface; standard for Rust |
| [clap](https://docs.rs/clap/) | 4.5 | CLI subcommand structure | Already in use for existing commands |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs | 6.0 | Plugin directory location | Already in use for paths |
| anyhow | 1.0 | Error handling with context | Already in use throughout |
| tracing | 0.1 | Discovery process logging | Already in use; respects RUST_LOG |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| toml 0.9 | toml 0.8 | 0.9 has improved error messages, already in Cargo.toml |
| fs::read_dir | walkdir | walkdir is overkill for single-level flat structure |
| Custom errors | thiserror | anyhow is already in use and sufficient |

**No new dependencies required.** All libraries already in Cargo.toml.

## Architecture Patterns

### Recommended Project Structure
```
src/plugin/
├── mod.rs              # Existing: adds `pub mod manager;`
├── manager.rs          # NEW: PluginManager, PluginManifest, PluginInfo
├── ffi_convert.rs      # Existing: FFI type conversions
├── generators/         # Existing: built-in generators
└── subprocess.rs       # Existing: subprocess execution
```

### Pattern 1: Plugin Manifest Struct
**What:** Define a serde struct for plugin.toml with all required/optional fields
**When to use:** Parsing discovered plugin manifests
**Example:**
```rust
// Source: Existing Config pattern + CONTEXT.md decisions
use serde::{Deserialize, Serialize};

/// Plugin manifest parsed from plugin.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,

    // Optional metadata (parse what we can, ignore unknown)
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,

    // Min interface version (for Phase 8 loading)
    #[serde(default)]
    pub min_interface_version: Option<String>,
}
```

### Pattern 2: PluginManager Registry (following ProjectRegistry)
**What:** Registry that discovers, loads, and tracks plugins
**When to use:** Main plugin management logic
**Example:**
```rust
// Source: Modeled after src/project/registry.rs
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub path: PathBuf,        // Path to plugin directory
    pub enabled: bool,        // From config
    pub error: Option<String>, // Manifest parse error or missing binary
}

#[derive(Debug, Default)]
pub struct PluginManager {
    plugins: HashMap<String, PluginInfo>,
}

impl PluginManager {
    /// Discover plugins from ~/.local/share/to-tui/plugins/
    pub fn discover() -> Result<Self> {
        let plugins_dir = get_plugins_dir()?;
        let mut manager = Self::default();

        if !plugins_dir.exists() {
            return Ok(manager);
        }

        for entry in fs::read_dir(&plugins_dir)? {
            let entry = entry?;
            let plugin_dir = entry.path();

            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("plugin.toml");
            let info = Self::load_plugin_info(&plugin_dir, &manifest_path);

            if let Some(name) = plugin_dir.file_name().and_then(|n| n.to_str()) {
                manager.plugins.insert(name.to_string(), info);
            }
        }

        Ok(manager)
    }

    fn load_plugin_info(plugin_dir: &Path, manifest_path: &Path) -> PluginInfo {
        // Parse manifest, return PluginInfo with error if parsing fails
    }

    pub fn list(&self) -> Vec<&PluginInfo> {
        self.plugins.values().collect()
    }

    pub fn get(&self, name: &str) -> Option<&PluginInfo> {
        self.plugins.get(name)
    }

    pub fn enabled_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.values().filter(|p| p.enabled && p.error.is_none()).collect()
    }
}
```

### Pattern 3: Config Extension for Plugin State
**What:** Add plugins section to existing Config struct
**When to use:** Persisting enable/disable state across restarts
**Example:**
```rust
// Source: Extend existing src/config.rs
use std::collections::HashSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Explicitly disabled plugins (enabled by default)
    #[serde(default)]
    pub disabled: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ... existing fields ...

    #[serde(default)]
    pub plugins: PluginsConfig,
}

impl PluginsConfig {
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.contains(name)
    }

    pub fn enable(&mut self, name: &str) {
        self.disabled.remove(name);
    }

    pub fn disable(&mut self, name: &str) {
        self.disabled.insert(name.to_string());
    }
}
```

### Pattern 4: CLI Subcommand Structure
**What:** Add plugin subcommands to existing CLI
**When to use:** User interaction via terminal
**Example:**
```rust
// Source: Extend existing src/cli.rs
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ... existing commands ...

    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum PluginCommand {
    /// List installed plugins
    List,
    /// Enable a plugin
    Enable { name: String },
    /// Disable a plugin
    Disable { name: String },
    /// Show detailed plugin status
    Status { name: String },
}
```

### Pattern 5: Status Bar Warning Display
**What:** Show plugin errors in TUI status bar briefly
**When to use:** Malformed manifest or missing binary at startup
**Example:**
```rust
// Source: Existing status_message pattern in AppState
impl AppState {
    pub fn set_plugin_warnings(&mut self, warnings: Vec<String>) {
        if !warnings.is_empty() {
            let msg = format!("Plugin warnings: {} issue(s) - run 'totui plugin list' for details",
                             warnings.len());
            self.set_status_message(msg);
        }
    }
}
```

### Anti-Patterns to Avoid
- **Recursive directory scanning:** Context specifies flat structure only; don't use walkdir
- **Loading plugin binaries in this phase:** Phase 7 is discovery only; Phase 8 handles loading
- **Hardcoding plugin paths:** Use existing `get_*_dir()` pattern from utils/paths.rs
- **Blocking on discovery:** Discovery should be fast; don't do network calls
- **Mutable global state:** Follow ProjectRegistry pattern with explicit load/save

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Directory location | Hardcoded strings | `dirs` + existing path utils | Cross-platform, XDG-compliant |
| TOML parsing | Custom parser | `toml` crate with serde | Handles edge cases, good errors |
| Config serialization | Custom format | `toml::to_string_pretty` | Already in use for config.toml |
| Version validation | String comparison | `semver::Version::parse` | Proper semver parsing |
| Error context | Raw anyhow | `.with_context()` | Consistent error chain |
| CLI structure | Manual arg parsing | clap derive macros | Already used, type-safe |

**Key insight:** The codebase already has patterns for everything needed. Follow existing code style rather than introducing new approaches.

## Common Pitfalls

### Pitfall 1: Silently Ignoring Malformed Manifests
**What goes wrong:** User doesn't know their plugin has a bad manifest
**Why it happens:** Error handling that logs but doesn't surface to user
**How to avoid:** Store error in PluginInfo.error field; show count in status bar; full details in `plugin status`
**Warning signs:** Plugin doesn't appear in list with no explanation

### Pitfall 2: Case-Sensitive Plugin Names
**What goes wrong:** `totui plugin enable MyPlugin` fails when directory is `myplugin`
**Why it happens:** Direct string comparison without normalization
**How to avoid:** Normalize to lowercase for lookups; preserve original for display
**Warning signs:** Enable/disable works inconsistently

### Pitfall 3: Race Condition on Config Save
**What goes wrong:** Config changes lost when multiple enables/disables happen quickly
**Why it happens:** Load-modify-save without locking
**How to avoid:** CLI commands are sequential so not an issue; if adding TUI toggle later, reload before save
**Warning signs:** Disabled plugins become enabled after restart

### Pitfall 4: Missing Plugin Directory
**What goes wrong:** Crash on first run before any plugins installed
**Why it happens:** Not checking if plugins directory exists
**How to avoid:** Return empty PluginManager if directory doesn't exist (don't create it)
**Warning signs:** Panic with "No such file or directory"

### Pitfall 5: Ignoring Unknown TOML Fields
**What goes wrong:** Future plugin.toml fields cause parse errors
**Why it happens:** Strict deserialization
**How to avoid:** Use `#[serde(flatten)] extra: HashMap<String, toml::Value>` or just let unknown fields be ignored (serde default behavior)
**Warning signs:** Old host can't load new plugins

### Pitfall 6: Binary Existence Check Before Phase 8
**What goes wrong:** Reporting "available" when binary might not load
**Why it happens:** Only checking file exists, not that it's loadable
**How to avoid:** Phase 7 only checks: manifest valid + expected files exist. "Available" means "passed basic checks"
**Warning signs:** User enables plugin, gets cryptic load error in Phase 8

## Code Examples

Verified patterns from official sources and existing codebase:

### Complete PluginManifest with Validation
```rust
// Source: Existing patterns + serde docs
use semver::Version;

impl PluginManifest {
    /// Validate manifest fields
    pub fn validate(&self) -> Result<(), String> {
        // Validate version is valid semver
        if Version::parse(&self.version).is_err() {
            return Err(format!("Invalid version '{}': must be valid semver", self.version));
        }

        // Validate name
        if self.name.is_empty() {
            return Err("Plugin name cannot be empty".to_string());
        }

        // Validate min_interface_version if present
        if let Some(ref min_ver) = self.min_interface_version {
            if Version::parse(min_ver).is_err() {
                return Err(format!("Invalid min_interface_version '{}'", min_ver));
            }
        }

        Ok(())
    }
}
```

### Discovery with Error Collection
```rust
// Source: Following existing error handling patterns
use std::fs;
use anyhow::{Context, Result};

impl PluginManager {
    pub fn discover() -> Result<Self> {
        let plugins_dir = get_plugins_dir()?;
        let mut manager = Self::default();

        if !plugins_dir.exists() {
            tracing::debug!("Plugins directory does not exist: {:?}", plugins_dir);
            return Ok(manager);
        }

        let entries = fs::read_dir(&plugins_dir)
            .with_context(|| format!("Failed to read plugins directory: {:?}", plugins_dir))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let name = match plugin_dir.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let info = Self::load_plugin_info(&plugin_dir);
            tracing::debug!("Discovered plugin '{}': {:?}", name, info.error);
            manager.plugins.insert(name, info);
        }

        Ok(manager)
    }

    fn load_plugin_info(plugin_dir: &Path) -> PluginInfo {
        let manifest_path = plugin_dir.join("plugin.toml");

        // Check manifest exists
        if !manifest_path.exists() {
            return PluginInfo {
                manifest: PluginManifest::default(),
                path: plugin_dir.to_path_buf(),
                enabled: true,
                error: Some("Missing plugin.toml".to_string()),
            };
        }

        // Parse manifest
        let content = match fs::read_to_string(&manifest_path) {
            Ok(c) => c,
            Err(e) => {
                return PluginInfo {
                    manifest: PluginManifest::default(),
                    path: plugin_dir.to_path_buf(),
                    enabled: true,
                    error: Some(format!("Failed to read plugin.toml: {}", e)),
                };
            }
        };

        let manifest: PluginManifest = match toml::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                return PluginInfo {
                    manifest: PluginManifest::default(),
                    path: plugin_dir.to_path_buf(),
                    enabled: true,
                    error: Some(format!("Invalid plugin.toml: {}", e)),
                };
            }
        };

        // Validate manifest
        if let Err(e) = manifest.validate() {
            return PluginInfo {
                manifest,
                path: plugin_dir.to_path_buf(),
                enabled: true,
                error: Some(e),
            };
        }

        PluginInfo {
            manifest,
            path: plugin_dir.to_path_buf(),
            enabled: true,
            error: None,
        }
    }
}
```

### CLI Handler Implementation
```rust
// Source: Following existing handle_* pattern in main.rs
fn handle_plugin_command(command: PluginCommand) -> Result<()> {
    match command {
        PluginCommand::List => {
            let config = Config::load()?;
            let mut manager = PluginManager::discover()?;
            manager.apply_config(&config.plugins);

            println!("\nInstalled plugins:\n");

            let plugins = manager.list();
            if plugins.is_empty() {
                println!("  (no plugins installed)");
                println!("\n  Plugins directory: {:?}", get_plugins_dir()?);
            } else {
                for info in plugins {
                    let status = if let Some(ref err) = info.error {
                        format!("\x1b[31m[error: {}]\x1b[0m", err)
                    } else if info.enabled {
                        "\x1b[32m[enabled]\x1b[0m".to_string()
                    } else {
                        "\x1b[33m[disabled]\x1b[0m".to_string()
                    };

                    println!("  {} v{} - {} {}",
                             info.manifest.name,
                             info.manifest.version,
                             info.manifest.description,
                             status);
                }
            }

            println!();
            Ok(())
        }
        PluginCommand::Enable { name } => {
            let mut config = Config::load()?;
            config.plugins.enable(&name);
            config.save()?;
            println!("Plugin '{}' enabled", name);
            Ok(())
        }
        PluginCommand::Disable { name } => {
            let mut config = Config::load()?;
            config.plugins.disable(&name);
            config.save()?;
            println!("Plugin '{}' disabled", name);
            Ok(())
        }
        PluginCommand::Status { name } => {
            let config = Config::load()?;
            let mut manager = PluginManager::discover()?;
            manager.apply_config(&config.plugins);

            match manager.get(&name) {
                Some(info) => {
                    println!("\nPlugin: {}", info.manifest.name);
                    println!("Version: {}", info.manifest.version);
                    println!("Description: {}", info.manifest.description);
                    println!("Path: {:?}", info.path);
                    println!("Enabled: {}", info.enabled);

                    if let Some(ref author) = info.manifest.author {
                        println!("Author: {}", author);
                    }
                    if let Some(ref license) = info.manifest.license {
                        println!("License: {}", license);
                    }
                    if let Some(ref err) = info.error {
                        println!("\n\x1b[31mError: {}\x1b[0m", err);
                    }
                    println!();
                }
                None => {
                    println!("Plugin '{}' not found", name);
                    println!("Run 'totui plugin list' to see installed plugins");
                }
            }
            Ok(())
        }
    }
}
```

### Plugins Directory Path
```rust
// Source: Extend existing utils/paths.rs
pub fn get_plugins_dir() -> Result<PathBuf> {
    // Following CONTEXT.md: ~/.local/share/to-tui/plugins/
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("Could not find local data directory"))?;
    Ok(data_dir.join("to-tui").join("plugins"))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ~/.to-tui location | ~/.local/share/to-tui | Project v2.0 | XDG compliance |
| JSON config | TOML config | v0.2.0 | Better readability |
| Hardcoded paths | dirs crate | v0.1.0 | Cross-platform |

**Deprecated/outdated:**
- None for this phase; all recommended approaches are current

## Open Questions

Things that couldn't be fully resolved:

1. **Immediate effect without restart**
   - What we know: CONTEXT.md says "enable/disable takes effect without restart"
   - What's unclear: Does this mean hot-reload in TUI, or just that next TUI launch uses new state?
   - Recommendation: For Phase 7, "immediate" means CLI updates config and next operation uses it. TUI hot-reload can be Phase 12 enhancement.

2. **Per-project plugin overrides**
   - What we know: CONTEXT.md mentions "global with per-project override"
   - What's unclear: What triggers per-project config? Project-local config file?
   - Recommendation: Start with global only. Add project-local `.totui-plugins.toml` if user requests. Not blocking for Phase 7.

3. **Plugin binary location within plugin directory**
   - What we know: Phase 8 will load binaries, Phase 7 just checks existence
   - What's unclear: Expected filename convention (e.g., `lib{name}.so`)?
   - Recommendation: Phase 7 only validates manifest exists. Leave binary detection to Phase 8 which has RootModule loading logic.

## Sources

### Primary (HIGH confidence)
- [toml crate docs](https://docs.rs/toml/) - Parsing and serialization
- [serde field attributes](https://serde.rs/field-attrs) - Default values, optional fields
- [semver crate docs](https://docs.rs/semver/) - Version parsing
- Existing codebase: `src/config.rs`, `src/project/registry.rs`, `src/utils/paths.rs`

### Secondary (MEDIUM confidence)
- [Rust Cookbook - Directory Traversal](https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html) - fs::read_dir patterns
- [VS Code Extension Manifest](https://code.visualstudio.com/api/references/extension-manifest) - Plugin manifest inspiration

### Tertiary (LOW confidence)
- WebSearch results on plugin discovery patterns - General architecture context

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in use, no new dependencies
- Architecture: HIGH - Following established patterns from ProjectRegistry and Config
- Pitfalls: HIGH - Based on existing error handling patterns in codebase
- CLI integration: HIGH - Extending existing clap-based CLI

**Research date:** 2026-01-24
**Valid until:** 2026-03-24 (60 days - patterns are stable, no external API changes expected)
