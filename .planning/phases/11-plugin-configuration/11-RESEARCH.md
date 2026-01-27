# Phase 11: Plugin Configuration - Research

**Researched:** 2026-01-26
**Domain:** Plugin configuration schema, TOML parsing, FFI-safe typed config passing, validation
**Confidence:** HIGH

## Summary

This phase implements per-plugin configuration with schema validation. Each plugin gets isolated config at `~/.config/to-tui/plugins/<name>/config.toml`, where the plugin defines a schema specifying expected fields, types, and optionality. The host reads/parses TOML, validates against the schema, and passes typed values to the plugin via FFI.

Key architectural insight: The existing codebase already has robust TOML parsing via the `toml` crate with serde, and abi_stable provides FFI-safe types (RString, RVec, ROption, RHashMap) for passing structured data across the plugin boundary. The decisions from CONTEXT.md constrain the scope: basic types only (string, integer, boolean, array of strings), strict enforcement (invalid config prevents loading), and a single read at startup with an `on_config_loaded()` callback.

**Primary recommendation:** Define an FFI-safe `FfiConfigSchema` struct with field definitions (name, type, required, default). Plugin exposes `config_schema()` method returning this schema. Host reads plugin's config.toml, validates against schema using serde_json for type checking, and passes an `RHashMap<RString, FfiConfigValue>` to the plugin's `on_config_loaded()` callback. Reuse the existing error popup pattern from Phase 8 for config validation errors.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| [toml](https://docs.rs/toml/) | 0.9 | Parse plugin config.toml files | Already in use for main config; serde integration |
| [serde](https://serde.rs/) | 1.0 | Deserialize to intermediate `toml::Value` | Already in use; handles unknown fields gracefully |
| [abi_stable](https://docs.rs/abi_stable/) | 0.11.3 | FFI-safe types for config values | Already in use; provides RString, RVec, ROption, RHashMap |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs | 6.0 | XDG config directory (`~/.config`) | Already in use; cross-platform config paths |
| anyhow | 1.0 | Error handling with context | Already in use throughout |
| clap | 4.5 | CLI for `totui plugin validate <name>` | Already in use for existing plugin commands |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `toml::Value` intermediate | Direct serde deserialize | toml::Value allows schema validation before typed access |
| RHashMap for config | Custom FfiConfig struct | RHashMap is more flexible for arbitrary plugin configs |
| JSON schema | Custom schema DSL | JSON schema is overkill for basic types; custom is simpler |

**No new dependencies required.** All libraries already in Cargo.toml.

## Architecture Patterns

### Recommended Project Structure
```
src/plugin/
├── mod.rs              # Add `pub mod config;`
├── config.rs           # NEW: PluginConfig, ConfigSchema, validation
├── manifest.rs         # Existing (extend to optionally include config schema)
├── manager.rs          # Existing (add config loading)
├── loader.rs           # Existing (call on_config_loaded)
└── ...

crates/totui-plugin-interface/src/
├── config.rs           # NEW: FfiConfigSchema, FfiConfigValue, FfiConfigField
├── plugin.rs           # Add config_schema() and on_config_loaded() methods
└── ...

~/.config/to-tui/plugins/<plugin-name>/
└── config.toml         # Plugin's user config file
```

### Pattern 1: FFI-Safe Config Value Enum
**What:** Define an enum for the supported config value types that can cross FFI boundary
**When to use:** Passing typed config values to plugins
**Example:**
```rust
// In crates/totui-plugin-interface/src/config.rs
use abi_stable::std_types::{ROption, RString, RVec};
use abi_stable::StableAbi;

/// FFI-safe config value types (CONTEXT.md: string, integer, boolean, array of strings)
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiConfigValue {
    String(RString),
    Integer(i64),
    Boolean(bool),
    StringArray(RVec<RString>),
}

/// FFI-safe config field type specifier for schema
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiConfigType {
    String = 0,
    Integer = 1,
    Boolean = 2,
    StringArray = 3,
}

/// FFI-safe config field definition
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiConfigField {
    /// Field name in config.toml
    pub name: RString,
    /// Expected type
    pub field_type: FfiConfigType,
    /// Whether field is required (if false, default must be provided)
    pub required: bool,
    /// Default value (used if field not present and not required)
    pub default: ROption<FfiConfigValue>,
    /// Human-readable description (for `totui plugin config <name> --init`)
    pub description: ROption<RString>,
}

/// FFI-safe config schema (collection of field definitions)
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiConfigSchema {
    /// List of field definitions
    pub fields: RVec<FfiConfigField>,
    /// Whether any config is required at all (empty = no config needed)
    pub config_required: bool,
}
```

### Pattern 2: Plugin Trait Extension for Config
**What:** Add config methods to the Plugin trait
**When to use:** Plugin initialization with config
**Example:**
```rust
// In crates/totui-plugin-interface/src/plugin.rs
use abi_stable::std_types::RHashMap;
use crate::config::{FfiConfigSchema, FfiConfigValue};

#[sabi_trait]
pub trait Plugin: Send + Sync + Debug {
    // ... existing methods ...

    /// Return the plugin's config schema.
    /// Return empty schema if plugin needs no config.
    fn config_schema(&self) -> FfiConfigSchema;

    /// Called after config is loaded and validated.
    /// Receives a map of field_name -> value.
    /// Called before generate() or execute_with_host() are ever invoked.
    #[sabi(last_prefix_field)]
    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>);
}
```

### Pattern 3: Host-Side Config Loading and Validation
**What:** Host reads, parses, and validates config before passing to plugin
**When to use:** Plugin initialization sequence
**Example:**
```rust
// In src/plugin/config.rs
use std::collections::HashMap;
use std::path::Path;
use anyhow::{Context, Result, bail};
use toml::Value;

pub struct PluginConfigLoader;

impl PluginConfigLoader {
    /// Load and validate plugin config from ~/.config/to-tui/plugins/<name>/config.toml
    pub fn load_and_validate(
        plugin_name: &str,
        schema: &FfiConfigSchema,
    ) -> Result<HashMap<String, ConfigValue>> {
        let config_path = get_plugin_config_path(plugin_name)?;

        // If no config file and config is required, fail
        if !config_path.exists() {
            if schema.config_required {
                bail!(
                    "Plugin '{}' requires configuration. Create: {}",
                    plugin_name,
                    config_path.display()
                );
            }
            // Return defaults for all optional fields
            return Ok(Self::collect_defaults(schema));
        }

        // Read and parse TOML
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;

        let toml_value: Value = toml::from_str(&content)
            .with_context(|| format!("Invalid TOML in {}", config_path.display()))?;

        let table = match toml_value {
            Value::Table(t) => t,
            _ => bail!("Config must be a TOML table"),
        };

        // Validate each field in schema
        let mut result = HashMap::new();
        for field in schema.fields.iter() {
            let field_name = field.name.to_string();

            match table.get(&field_name) {
                Some(value) => {
                    // Validate type matches schema
                    let typed_value = Self::validate_field_type(&field_name, value, field.field_type)?;
                    result.insert(field_name, typed_value);
                }
                None => {
                    if field.required {
                        bail!("{}: required field is missing", field_name);
                    }
                    // Use default if provided
                    if let ROption::RSome(ref default) = field.default {
                        result.insert(field_name, default.clone().into());
                    }
                }
            }
        }

        Ok(result)
    }

    fn validate_field_type(
        field_name: &str,
        value: &Value,
        expected_type: FfiConfigType,
    ) -> Result<ConfigValue> {
        match (expected_type, value) {
            (FfiConfigType::String, Value::String(s)) => Ok(ConfigValue::String(s.clone())),
            (FfiConfigType::Integer, Value::Integer(i)) => Ok(ConfigValue::Integer(*i)),
            (FfiConfigType::Boolean, Value::Boolean(b)) => Ok(ConfigValue::Boolean(*b)),
            (FfiConfigType::StringArray, Value::Array(arr)) => {
                let strings: Result<Vec<String>> = arr.iter().map(|v| {
                    match v {
                        Value::String(s) => Ok(s.clone()),
                        _ => bail!("{}: array must contain only strings", field_name),
                    }
                }).collect();
                Ok(ConfigValue::StringArray(strings?))
            }
            _ => bail!(
                "{}: expected {:?}, got {:?}",
                field_name,
                expected_type,
                value.type_str()
            ),
        }
    }
}
```

### Pattern 4: Config Directory Path Helper
**What:** Get the XDG config path for a plugin
**When to use:** Locating plugin config files
**Example:**
```rust
// In src/utils/paths.rs
use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Get the config directory for a specific plugin.
/// Returns ~/.config/to-tui/plugins/<name>/
pub fn get_plugin_config_dir(plugin_name: &str) -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not find config directory"))?;
    Ok(config_dir.join("to-tui").join("plugins").join(plugin_name))
}

/// Get the config file path for a specific plugin.
/// Returns ~/.config/to-tui/plugins/<name>/config.toml
pub fn get_plugin_config_path(plugin_name: &str) -> Result<PathBuf> {
    Ok(get_plugin_config_dir(plugin_name)?.join("config.toml"))
}
```

### Pattern 5: CLI Validate Command
**What:** Validate plugin config without starting TUI
**When to use:** CI/scripting, debugging config issues
**Example:**
```rust
// In src/cli.rs - extend PluginCommand
#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommand {
    // ... existing commands ...

    /// Validate a plugin's configuration
    Validate {
        /// Plugin name
        name: String,
    },

    /// Show or initialize plugin config
    Config {
        /// Plugin name
        name: String,
        /// Generate template config from schema
        #[arg(long)]
        init: bool,
    },
}

// In src/main.rs handler
fn handle_plugin_validate(name: &str) -> Result<()> {
    let manager = PluginManager::discover()?;
    let plugin_info = manager.get(name)
        .ok_or_else(|| anyhow!("Plugin '{}' not found", name))?;

    // Load the plugin to get its schema
    let mut loader = PluginLoader::new();
    let loaded = loader.load_plugin(&plugin_info.path, plugin_info)?;

    let schema = loaded.plugin.config_schema();

    match PluginConfigLoader::load_and_validate(name, &schema) {
        Ok(_) => {
            println!("Plugin '{}' configuration is valid.", name);
            Ok(())
        }
        Err(e) => {
            eprintln!("Configuration error for plugin '{}':", name);
            eprintln!("  {}", e);
            std::process::exit(1);
        }
    }
}
```

### Pattern 6: Error Aggregation and Popup Display
**What:** Collect config errors from all plugins and show in single popup
**When to use:** TUI startup when multiple plugins have config issues
**Example:**
```rust
// Reuse existing error popup pattern from Phase 8
// In src/plugin/loader.rs

pub struct ConfigError {
    pub plugin_name: String,
    pub message: String,
}

impl PluginLoader {
    pub fn load_all_with_config(
        &mut self,
        manager: &PluginManager,
    ) -> (Vec<PluginLoadError>, Vec<ConfigError>) {
        let mut load_errors = Vec::new();
        let mut config_errors = Vec::new();

        for plugin_info in manager.enabled_plugins() {
            match self.load_plugin(&plugin_info.path, plugin_info) {
                Ok(loaded) => {
                    // Get schema and validate config
                    let schema = loaded.plugin.config_schema();
                    match PluginConfigLoader::load_and_validate(&plugin_info.manifest.name, &schema) {
                        Ok(config) => {
                            // Convert to FFI and call on_config_loaded
                            let ffi_config = Self::to_ffi_config(&config);
                            loaded.plugin.on_config_loaded(ffi_config);
                            self.plugins.insert(loaded.name.to_lowercase(), loaded);
                        }
                        Err(e) => {
                            config_errors.push(ConfigError {
                                plugin_name: plugin_info.manifest.name.clone(),
                                message: e.to_string(),
                            });
                            // Don't add plugin to loaded - it failed config validation
                        }
                    }
                }
                Err(err) => load_errors.push(err),
            }
        }

        (load_errors, config_errors)
    }
}
```

### Anti-Patterns to Avoid
- **Passing raw TOML string to plugin:** Parse and validate in host, pass typed values
- **Hot-reload config:** CONTEXT.md says config read once at startup, no hot reload
- **Complex types:** Stick to basic types per CONTEXT.md (string, int, bool, string array)
- **Silencing config errors:** Always surface to user with field names and expected types
- **Creating config directory automatically:** Let user create it; only create on `--init`

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config file location | Hardcoded paths | `dirs::config_dir()` | Cross-platform XDG support |
| TOML parsing | Custom parser | `toml` crate | Handles edge cases, good errors |
| Type validation | String parsing | `toml::Value` type checks | Already parsed, type info available |
| FFI map type | Custom struct | abi_stable `RHashMap` | Already FFI-safe, tested |
| Error display | New popup widget | Existing `render_plugin_error_popup` | Reuse Phase 8 pattern |
| CLI structure | Manual parsing | clap derive | Already in use, type-safe |

**Key insight:** The host does all parsing and validation. Plugins only define schemas and receive pre-validated typed values via FFI.

## Common Pitfalls

### Pitfall 1: Type Mismatch Errors Without Field Names
**What goes wrong:** User gets "expected string, got integer" with no context
**Why it happens:** Not including field name in error message
**How to avoid:** Always prefix errors with field name: "`api_key`: expected string, got integer"
**Warning signs:** Users can't figure out which field has the wrong type

### Pitfall 2: Missing Config Silently Uses Empty Map
**What goes wrong:** Plugin runs without required config, produces cryptic errors
**Why it happens:** Not checking `config_required` flag before returning empty defaults
**How to avoid:** If `config_required` is true and file doesn't exist, fail with clear message showing path
**Warning signs:** Plugin crashes or misbehaves when config file doesn't exist

### Pitfall 3: Schema Changes Break Existing Configs
**What goes wrong:** Plugin update adds required field, existing configs become invalid
**Why it happens:** New required fields with no default
**How to avoid:** New fields should either have defaults or be optional; document migration in changelog
**Warning signs:** Plugin that worked before suddenly fails after update

### Pitfall 4: Config Directory Not Created
**What goes wrong:** User tries to create config.toml but parent directory doesn't exist
**Why it happens:** `~/.config/to-tui/plugins/<name>/` doesn't exist
**How to avoid:** `totui plugin config <name> --init` creates directory and template file
**Warning signs:** "No such file or directory" when user tries to create config

### Pitfall 5: RHashMap Key Lifetime Issues
**What goes wrong:** Compile errors with RHashMap across FFI
**Why it happens:** Using `&str` keys instead of `RString`
**How to avoid:** Keys must be `RString` not `&str`; convert at FFI boundary
**Warning signs:** Lifetime errors in FFI code

### Pitfall 6: Calling Plugin Methods Before Config Loaded
**What goes wrong:** Plugin accesses config values that aren't set yet
**Why it happens:** Calling `generate()` before `on_config_loaded()`
**How to avoid:** Always call `on_config_loaded()` immediately after loading, before any other plugin method
**Warning signs:** Plugin panics on first use

## Code Examples

Verified patterns from official sources and existing codebase:

### Complete FfiConfigValue and Conversion
```rust
// Source: abi_stable docs + existing FfiTodoItem pattern
use abi_stable::std_types::{RHashMap, ROption, RString, RVec};
use abi_stable::StableAbi;

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiConfigValue {
    String(RString),
    Integer(i64),
    Boolean(bool),
    StringArray(RVec<RString>),
}

impl From<ConfigValue> for FfiConfigValue {
    fn from(value: ConfigValue) -> Self {
        match value {
            ConfigValue::String(s) => FfiConfigValue::String(s.into()),
            ConfigValue::Integer(i) => FfiConfigValue::Integer(i),
            ConfigValue::Boolean(b) => FfiConfigValue::Boolean(b),
            ConfigValue::StringArray(arr) => {
                FfiConfigValue::StringArray(arr.into_iter().map(RString::from).collect())
            }
        }
    }
}

// Convert HashMap to RHashMap for FFI
fn to_ffi_config(config: &HashMap<String, ConfigValue>) -> RHashMap<RString, FfiConfigValue> {
    let mut ffi_map = RHashMap::new();
    for (key, value) in config {
        ffi_map.insert(RString::from(key.as_str()), FfiConfigValue::from(value.clone()));
    }
    ffi_map
}
```

### Config Template Generation
```rust
// For `totui plugin config <name> --init`
fn generate_config_template(schema: &FfiConfigSchema) -> String {
    let mut output = String::new();
    output.push_str("# Plugin configuration\n\n");

    for field in schema.fields.iter() {
        // Add description as comment
        if let ROption::RSome(ref desc) = field.description {
            output.push_str(&format!("# {}\n", desc));
        }

        let required_marker = if field.required { " (required)" } else { " (optional)" };
        output.push_str(&format!("# Type: {:?}{}\n", field.field_type, required_marker));

        // Add example/default value
        let example = match (&field.field_type, &field.default) {
            (_, ROption::RSome(FfiConfigValue::String(s))) => format!("\"{}\"", s),
            (_, ROption::RSome(FfiConfigValue::Integer(i))) => format!("{}", i),
            (_, ROption::RSome(FfiConfigValue::Boolean(b))) => format!("{}", b),
            (_, ROption::RSome(FfiConfigValue::StringArray(arr))) => {
                let items: Vec<String> = arr.iter().map(|s| format!("\"{}\"", s)).collect();
                format!("[{}]", items.join(", "))
            }
            (FfiConfigType::String, _) => "\"value\"".to_string(),
            (FfiConfigType::Integer, _) => "0".to_string(),
            (FfiConfigType::Boolean, _) => "false".to_string(),
            (FfiConfigType::StringArray, _) => "[\"item1\", \"item2\"]".to_string(),
        };

        if field.required {
            output.push_str(&format!("{} = {}\n\n", field.name, example));
        } else {
            output.push_str(&format!("# {} = {}\n\n", field.name, example));
        }
    }

    output
}
```

### Plugin Side: Defining Schema and Receiving Config
```rust
// Example plugin implementation
use totui_plugin_interface::{
    FfiConfigSchema, FfiConfigField, FfiConfigType, FfiConfigValue,
    Plugin, RHashMap, ROption, RString, RVec,
};
use std::sync::Mutex;

struct MyPlugin {
    config: Mutex<Option<MyConfig>>,
}

struct MyConfig {
    api_key: String,
    timeout_seconds: i64,
    debug: bool,
}

impl Plugin for MyPlugin {
    fn config_schema(&self) -> FfiConfigSchema {
        FfiConfigSchema {
            config_required: true,
            fields: vec![
                FfiConfigField {
                    name: "api_key".into(),
                    field_type: FfiConfigType::String,
                    required: true,
                    default: ROption::RNone,
                    description: ROption::RSome("API key for authentication".into()),
                },
                FfiConfigField {
                    name: "timeout_seconds".into(),
                    field_type: FfiConfigType::Integer,
                    required: false,
                    default: ROption::RSome(FfiConfigValue::Integer(30)),
                    description: ROption::RSome("Request timeout in seconds".into()),
                },
                FfiConfigField {
                    name: "debug".into(),
                    field_type: FfiConfigType::Boolean,
                    required: false,
                    default: ROption::RSome(FfiConfigValue::Boolean(false)),
                    description: ROption::RSome("Enable debug logging".into()),
                },
            ].into(),
        }
    }

    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>) {
        let api_key = match config.get(&RString::from("api_key")) {
            Some(FfiConfigValue::String(s)) => s.to_string(),
            _ => panic!("api_key must be present and be a string"), // Shouldn't happen if host validates
        };

        let timeout = match config.get(&RString::from("timeout_seconds")) {
            Some(FfiConfigValue::Integer(i)) => *i,
            _ => 30, // Default
        };

        let debug = match config.get(&RString::from("debug")) {
            Some(FfiConfigValue::Boolean(b)) => *b,
            _ => false, // Default
        };

        let my_config = MyConfig {
            api_key,
            timeout_seconds: timeout,
            debug,
        };

        *self.config.lock().unwrap() = Some(my_config);
    }

    // ... rest of Plugin trait ...
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| JSON config | TOML config | Project standard | Better readability, comments supported |
| Raw string via FFI | Typed values via FFI | This phase | Type safety, validation at boundary |
| Manual type parsing | serde + toml crate | Stable | Handles edge cases automatically |

**Deprecated/outdated:**
- Passing raw TOML/JSON strings to plugins and letting them parse
- Hot-reloading config (explicitly out of scope per CONTEXT.md)

## Open Questions

Things that couldn't be fully resolved:

1. **Schema location: inline in manifest vs separate method**
   - What we know: CONTEXT.md says "Claude's discretion"
   - Recommendation: Use `config_schema()` method on Plugin trait (like existing methods), not in manifest file. Schema is code, not metadata.

2. **Default values: in schema vs plugin code**
   - What we know: CONTEXT.md says "Claude's discretion"
   - Recommendation: Defaults in schema (via `FfiConfigField.default`). This allows host to apply defaults before calling plugin, and enables template generation.

3. **Template generation: `--init` command**
   - What we know: CONTEXT.md says "Claude's discretion"
   - Recommendation: Implement `totui plugin config <name> --init` that creates directory and writes template. Useful for discoverability.

4. **RHashMap vs RVec of tuples for config**
   - What we know: Both are FFI-safe
   - Recommendation: Use `RHashMap<RString, FfiConfigValue>` - more natural API for key-value access

## Sources

### Primary (HIGH confidence)
- [toml crate docs](https://docs.rs/toml/) - Value type, parsing, validation
- [serde field attributes](https://serde.rs/field-attrs) - Default values, optional fields
- [abi_stable std_types](https://docs.rs/abi_stable/latest/abi_stable/std_types/index.html) - RHashMap, RString, RVec
- Existing codebase: `src/config.rs`, `src/plugin/loader.rs`, `render_plugin_error_popup`

### Secondary (MEDIUM confidence)
- [dirs crate docs](https://docs.rs/dirs/) - config_dir() for XDG paths
- Existing Phase 8 patterns: Error popup, plugin loading sequence

### Tertiary (LOW confidence)
- WebSearch on plugin config patterns - General architecture context

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in use, no new dependencies
- Architecture: HIGH - Following established FFI patterns from totui-plugin-interface
- Pitfalls: HIGH - Based on existing error handling and Phase 8 patterns
- FFI types: HIGH - Using documented abi_stable types

**Research date:** 2026-01-26
**Valid until:** 2026-03-26 (60 days - patterns are stable, internal implementation)
