//! Plugin loader for dynamically loading native plugins.
//!
//! This module provides the `PluginLoader` struct which loads native plugins
//! (.so/.dylib/.dll) using abi_stable. It handles:
//! - Loading plugins via `PluginModule_Ref::load_from_directory`
//! - Categorizing loading errors (version mismatch vs corruption)
//! - Catching plugin panics at the FFI boundary
//! - Session-based disabling for panicked plugins

use abi_stable::{
    library::{LibraryError, LibraryPath, RootModule},
    std_types::{RBox, RString},
};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use totui_plugin_interface::{
    call_plugin_on_config_loaded, FfiEventType, PluginModule_Ref, Plugin_TO, INTERFACE_VERSION,
};

use crate::plugin::config::{to_ffi_config, PluginConfigLoader};
use crate::plugin::{PluginInfo, PluginManager};

/// Kinds of plugin loading/execution errors.
#[derive(Debug, Clone)]
pub enum PluginErrorKind {
    /// Plugin requires a newer interface version than the host provides.
    VersionMismatch { required: String, actual: String },
    /// Library file is corrupted or cannot be opened.
    LibraryCorrupted,
    /// Required symbol is missing from the library.
    SymbolMissing,
    /// Plugin was disabled for this session after a panic.
    SessionDisabled,
    /// Plugin panicked during execution.
    Panicked { message: String },
    /// Other error (catch-all).
    Other(String),
}

/// Error when loading or calling a plugin.
#[derive(Debug, Clone)]
pub struct PluginLoadError {
    /// Name of the plugin that failed.
    pub plugin_name: String,
    /// Category of the error.
    pub error_kind: PluginErrorKind,
    /// Human-readable error message.
    pub message: String,
}

impl std::fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PluginLoadError {}

/// Error when loading or validating plugin configuration.
#[derive(Debug, Clone)]
pub struct ConfigError {
    /// Name of the plugin with the config error.
    pub plugin_name: String,
    /// Human-readable error message.
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.plugin_name, self.message)
    }
}

impl std::error::Error for ConfigError {}

/// A loaded plugin instance.
pub struct LoadedPlugin {
    /// Plugin trait object with 'static lifetime - this is the proxy pattern.
    /// abi_stable intentionally leaks the library (never unloads) to guarantee
    /// the library outlives all plugin objects, avoiding TLS destructor issues.
    /// See: https://github.com/rust-lang/rust/issues/59629
    pub plugin: Plugin_TO<'static, RBox<()>>,
    /// Plugin name (from manifest).
    pub name: String,
    /// Plugin version (from manifest).
    pub version: String,
    /// Plugin description (from manifest).
    pub description: String,
    /// Disabled for current session only (after runtime panic).
    /// Loading failures do NOT set this - they persist across launches.
    pub session_disabled: bool,
}

/// Plugin loader that manages loaded plugin instances.
///
/// Uses abi_stable's `load_from_directory` which leaks the library (proxy pattern)
/// to ensure plugins remain loaded for the entire application lifetime.
#[derive(Default)]
pub struct PluginLoader {
    /// Loaded plugins keyed by lowercase name.
    plugins: HashMap<String, LoadedPlugin>,
    /// Config errors from the most recent load_all call.
    config_errors: Vec<ConfigError>,
    /// Event subscriptions per plugin (populated at load time).
    event_subscriptions: HashMap<String, Vec<FfiEventType>>,
}

impl PluginLoader {
    /// Create a new empty plugin loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all enabled plugins from the plugin manager.
    ///
    /// Returns a list of errors for plugins that failed to load.
    /// Successfully loaded plugins are stored in the loader.
    /// Config errors are stored internally and can be retrieved via `get_config_errors()`.
    pub fn load_all(&mut self, manager: &PluginManager) -> Vec<PluginLoadError> {
        let (load_errors, _config_errors) = self.load_all_with_config(manager);
        load_errors
    }

    /// Load all enabled plugins with config validation.
    ///
    /// For each enabled plugin:
    /// 1. Load the plugin dylib
    /// 2. Get the config schema from the plugin
    /// 3. Load and validate config against the schema
    /// 4. If valid, call on_config_loaded() and add plugin to loaded map
    /// 5. If invalid, add to config_errors and skip the plugin
    ///
    /// Returns both load errors and config errors separately.
    pub fn load_all_with_config(
        &mut self,
        manager: &PluginManager,
    ) -> (Vec<PluginLoadError>, Vec<ConfigError>) {
        let mut load_errors = Vec::new();
        let mut config_errors = Vec::new();

        for plugin_info in manager.enabled_plugins() {
            // Step 1: Load the dylib
            match self.load_plugin(&plugin_info.path, plugin_info) {
                Ok(loaded) => {
                    let plugin_name = loaded.name.clone();

                    // Step 2: Get config schema from plugin
                    let schema = loaded.plugin.config_schema();

                    // Step 3: Load and validate config
                    match PluginConfigLoader::load_and_validate(&plugin_name, &schema) {
                        Ok(config) => {
                            // Step 4a: Convert to FFI and call on_config_loaded
                            let ffi_config = to_ffi_config(&config);
                            if let Err(panic_msg) =
                                call_plugin_on_config_loaded(&loaded.plugin, ffi_config)
                            {
                                // Plugin panicked during config loading
                                tracing::warn!(
                                    plugin = %plugin_name,
                                    "Plugin panicked during config loading: {}",
                                    panic_msg
                                );
                                config_errors.push(ConfigError {
                                    plugin_name: plugin_name.clone(),
                                    message: panic_msg,
                                });
                                // Don't add to loaded plugins
                                continue;
                            }

                            // Get event subscriptions
                            let subscriptions: Vec<FfiEventType> =
                                loaded.plugin.subscribed_events().into_iter().collect();
                            if !subscriptions.is_empty() {
                                tracing::info!(
                                    plugin = %plugin_name,
                                    events = ?subscriptions,
                                    "Plugin subscribed to events"
                                );
                            }
                            self.event_subscriptions
                                .insert(plugin_name.to_lowercase(), subscriptions);

                            tracing::info!("Loaded plugin with config: {}", plugin_name);
                            self.plugins.insert(plugin_name.to_lowercase(), loaded);
                        }
                        Err(e) => {
                            // Step 4b: Config validation failed
                            tracing::warn!(
                                plugin = %plugin_name,
                                config = true,
                                "Config validation failed: {}",
                                e
                            );
                            config_errors.push(ConfigError {
                                plugin_name,
                                message: e.to_string(),
                            });
                            // Don't add to loaded plugins
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("Failed to load plugin: {}", err.message);
                    load_errors.push(err);
                }
            }
        }

        // Store config errors for later retrieval
        self.config_errors = config_errors.clone();

        (load_errors, config_errors)
    }

    /// Get config errors from the most recent load operation.
    pub fn get_config_errors(&self) -> &[ConfigError] {
        &self.config_errors
    }

    /// Load a single plugin from its directory.
    ///
    /// Finds the dylib file in the plugin directory and loads it via abi_stable.
    /// The library is leaked (never unloaded) - this IS the proxy pattern.
    pub fn load_plugin(
        &self,
        path: &Path,
        plugin_info: &PluginInfo,
    ) -> Result<LoadedPlugin, PluginLoadError> {
        let plugin_name = &plugin_info.manifest.name;

        // Find the dylib file in the plugin directory
        let dylib_path = Self::find_dylib_in_directory(path, plugin_name)?;

        // Load the library using abi_stable
        // This leaks the library intentionally - it will never be unloaded
        let module = PluginModule_Ref::load_from(LibraryPath::FullPath(&dylib_path)).map_err(|lib_err| {
            Self::map_library_error(plugin_name, &lib_err)
        })?;

        // Create plugin instance by calling the factory function
        let plugin = (module.create_plugin())();

        // Verify the plugin's minimum interface version matches
        let plugin_min_version = plugin.min_interface_version().to_string();
        match totui_plugin_interface::is_version_compatible(&plugin_min_version, INTERFACE_VERSION)
        {
            Ok(true) => {
                // Compatible - continue
            }
            Ok(false) => {
                return Err(PluginLoadError {
                    plugin_name: plugin_name.clone(),
                    error_kind: PluginErrorKind::VersionMismatch {
                        required: plugin_min_version.clone(),
                        actual: INTERFACE_VERSION.to_string(),
                    },
                    message: format!(
                        "Plugin {} requires to-tui {}+, you have {}",
                        plugin_name, plugin_min_version, INTERFACE_VERSION
                    ),
                });
            }
            Err(e) => {
                return Err(PluginLoadError {
                    plugin_name: plugin_name.clone(),
                    error_kind: PluginErrorKind::Other(e.clone()),
                    message: format!("Version check failed for {}: {}", plugin_name, e),
                });
            }
        }

        Ok(LoadedPlugin {
            plugin,
            name: plugin_name.clone(),
            version: plugin_info.manifest.version.clone(),
            description: plugin_info.manifest.description.clone(),
            session_disabled: false,
        })
    }

    /// Find the dylib file in a plugin directory.
    ///
    /// Looks for .dylib (macOS), .so (Linux), or .dll (Windows) files.
    /// Returns the path to the first matching library file.
    fn find_dylib_in_directory(dir: &Path, plugin_name: &str) -> Result<std::path::PathBuf, PluginLoadError> {
        let extensions = if cfg!(target_os = "macos") {
            &["dylib"][..]
        } else if cfg!(target_os = "windows") {
            &["dll"][..]
        } else {
            &["so"][..]
        };

        // Try to find any library file in the directory
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(ext) = path.extension()
                    && extensions.iter().any(|e| ext == *e)
                {
                    return Ok(path);
                }
            }
        }

        Err(PluginLoadError {
            plugin_name: plugin_name.to_string(),
            error_kind: PluginErrorKind::LibraryCorrupted,
            message: format!(
                "Plugin {} has no library file in {}",
                plugin_name,
                dir.display()
            ),
        })
    }

    /// Map abi_stable LibraryError to our PluginLoadError.
    fn map_library_error(plugin_name: &str, lib_err: &LibraryError) -> PluginLoadError {
        // Include full error details in the message for debugging
        let error_detail = format!("{:?}", lib_err);

        match lib_err {
            LibraryError::OpenError { .. } => PluginLoadError {
                plugin_name: plugin_name.to_string(),
                error_kind: PluginErrorKind::LibraryCorrupted,
                message: format!(
                    "Plugin {} failed to open library: {}",
                    plugin_name, error_detail
                ),
            },
            LibraryError::GetSymbolError { .. } => PluginLoadError {
                plugin_name: plugin_name.to_string(),
                error_kind: PluginErrorKind::SymbolMissing,
                message: format!(
                    "Plugin {} missing required symbol: {}",
                    plugin_name, error_detail
                ),
            },
            LibraryError::IncompatibleVersionNumber {
                expected_version,
                actual_version,
                ..
            } => PluginLoadError {
                plugin_name: plugin_name.to_string(),
                error_kind: PluginErrorKind::VersionMismatch {
                    required: expected_version.to_string(),
                    actual: actual_version.to_string(),
                },
                message: format!(
                    "Plugin {} requires interface version {}, but totui provides {}",
                    plugin_name, expected_version, actual_version
                ),
            },
            _ => PluginLoadError {
                plugin_name: plugin_name.to_string(),
                error_kind: PluginErrorKind::Other(error_detail.clone()),
                message: format!(
                    "Plugin {} failed to load: {}",
                    plugin_name, error_detail
                ),
            },
        }
    }

    /// Get a loaded plugin by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(&name.to_lowercase())
    }

    /// Get a mutable reference to a loaded plugin by name (case-insensitive).
    pub fn get_mut(&mut self, name: &str) -> Option<&mut LoadedPlugin> {
        self.plugins.get_mut(&name.to_lowercase())
    }

    /// Iterate over all loaded plugins.
    pub fn loaded_plugins(&self) -> impl Iterator<Item = &LoadedPlugin> {
        self.plugins.values()
    }

    /// Get plugins subscribed to a specific event type.
    ///
    /// Returns list of (plugin reference, timeout_duration) for each subscribed plugin.
    pub fn plugins_for_event(&self, event_type: FfiEventType) -> Vec<(&LoadedPlugin, Duration)> {
        use crate::plugin::hooks::DEFAULT_HOOK_TIMEOUT;

        self.event_subscriptions
            .iter()
            .filter(|(_, events)| events.contains(&event_type))
            .filter_map(|(name, _)| {
                self.plugins.get(name).map(|p| {
                    // Get timeout from manifest if plugin manager available
                    // For now use default - will be wired in plan 03
                    (p, DEFAULT_HOOK_TIMEOUT)
                })
            })
            .collect()
    }

    /// Call a plugin method safely, catching panics.
    ///
    /// If the plugin panics, it will be disabled for the rest of the session
    /// and the panic will be logged with a backtrace.
    pub fn call_safely<T, F>(&mut self, plugin_name: &str, f: F) -> Result<T, PluginLoadError>
    where
        F: FnOnce(&Plugin_TO<'_, RBox<()>>) -> T,
    {
        // Check if plugin exists
        let plugin = self.get(plugin_name).ok_or_else(|| PluginLoadError {
            plugin_name: plugin_name.to_string(),
            error_kind: PluginErrorKind::Other("Plugin not loaded".to_string()),
            message: format!("Plugin {} is not loaded", plugin_name),
        })?;

        // Check if plugin is disabled for this session
        if plugin.session_disabled {
            return Err(PluginLoadError {
                plugin_name: plugin_name.to_string(),
                error_kind: PluginErrorKind::SessionDisabled,
                message: format!(
                    "Plugin {} is disabled for this session after a previous error",
                    plugin_name
                ),
            });
        }

        // Get a reference to the plugin for use in the closure
        // We need to borrow the plugin again after the check
        let plugin_ref = &self.plugins.get(&plugin_name.to_lowercase()).unwrap().plugin;

        // Call the function with panic catching
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(plugin_ref)));

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

                // Log panic to file (always, per CONTEXT.md)
                Self::log_plugin_panic(plugin_name, &msg);

                // Disable plugin for session (per CONTEXT.md)
                if let Some(p) = self.plugins.get_mut(&plugin_name.to_lowercase()) {
                    p.session_disabled = true;
                }

                Err(PluginLoadError {
                    plugin_name: plugin_name.to_string(),
                    error_kind: PluginErrorKind::Panicked { message: msg.clone() },
                    message: format!("Plugin {} panicked: {}", plugin_name, msg),
                })
            }
        }
    }

    /// Log a plugin panic with backtrace.
    fn log_plugin_panic(plugin_name: &str, message: &str) {
        // Get backtrace
        let backtrace = std::backtrace::Backtrace::force_capture();

        // Log using tracing (will be captured by file appender when configured)
        tracing::error!(
            plugin = %plugin_name,
            message = %message,
            backtrace = %backtrace,
            "Plugin panicked during execution"
        );
    }

    /// Call a plugin's generate method safely.
    ///
    /// This is a convenience wrapper around `call_safely` for the common
    /// case of calling `plugin.generate(input)`.
    pub fn call_generate(
        &mut self,
        plugin_name: &str,
        input: &str,
    ) -> Result<Vec<crate::todo::TodoItem>, PluginLoadError> {
        self.call_safely(plugin_name, |plugin| {
            let ffi_result = plugin.generate(RString::from(input));
            match ffi_result.into_result() {
                Ok(items) => {
                    // Convert FFI items to native TodoItems
                    items
                        .into_iter()
                        .map(|ffi_item| {
                            crate::todo::TodoItem::try_from(ffi_item)
                                .map_err(|e| e.to_string())
                        })
                        .collect::<Result<Vec<_>, _>>()
                }
                Err(err) => Err(err.to_string()),
            }
        })?
        .map_err(|e| PluginLoadError {
            plugin_name: plugin_name.to_string(),
            error_kind: PluginErrorKind::Other(e.clone()),
            message: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_loader_new() {
        let loader = PluginLoader::new();
        assert_eq!(loader.plugins.len(), 0);
    }

    #[test]
    fn test_get_nonexistent_plugin() {
        let loader = PluginLoader::new();
        assert!(loader.get("nonexistent").is_none());
    }

    #[test]
    fn test_call_safely_plugin_not_loaded() {
        let mut loader = PluginLoader::new();
        let result = loader.call_safely("nonexistent", |_| ());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err.error_kind, PluginErrorKind::Other(_)));
        assert!(err.message.contains("not loaded"));
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginLoadError {
            plugin_name: "test".to_string(),
            error_kind: PluginErrorKind::LibraryCorrupted,
            message: "Plugin test failed to load".to_string(),
        };
        assert_eq!(format!("{}", err), "Plugin test failed to load");
    }

    #[test]
    fn test_plugin_error_kind_version_mismatch() {
        let kind = PluginErrorKind::VersionMismatch {
            required: "1.0.0".to_string(),
            actual: "0.1.0".to_string(),
        };
        if let PluginErrorKind::VersionMismatch { required, actual } = kind {
            assert_eq!(required, "1.0.0");
            assert_eq!(actual, "0.1.0");
        } else {
            panic!("Expected VersionMismatch");
        }
    }

    #[test]
    fn test_plugin_error_kind_panicked() {
        let kind = PluginErrorKind::Panicked {
            message: "something went wrong".to_string(),
        };
        if let PluginErrorKind::Panicked { message } = kind {
            assert_eq!(message, "something went wrong");
        } else {
            panic!("Expected Panicked");
        }
    }

    #[test]
    fn test_loaded_plugins_iterator_empty() {
        let loader = PluginLoader::new();
        assert_eq!(loader.loaded_plugins().count(), 0);
    }

    #[test]
    fn test_plugin_error_kind_session_disabled() {
        let kind = PluginErrorKind::SessionDisabled;
        assert!(matches!(kind, PluginErrorKind::SessionDisabled));
    }

    #[test]
    fn test_plugin_error_kind_library_corrupted() {
        let kind = PluginErrorKind::LibraryCorrupted;
        assert!(matches!(kind, PluginErrorKind::LibraryCorrupted));
    }

    #[test]
    fn test_plugin_error_kind_symbol_missing() {
        let kind = PluginErrorKind::SymbolMissing;
        assert!(matches!(kind, PluginErrorKind::SymbolMissing));
    }

    #[test]
    fn test_plugin_error_kind_other() {
        let kind = PluginErrorKind::Other("some error".to_string());
        if let PluginErrorKind::Other(msg) = kind {
            assert_eq!(msg, "some error");
        } else {
            panic!("Expected Other");
        }
    }

    #[test]
    fn test_plugin_load_error_is_error() {
        let err = PluginLoadError {
            plugin_name: "test".to_string(),
            error_kind: PluginErrorKind::LibraryCorrupted,
            message: "Plugin test failed to load".to_string(),
        };
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_get_mut_nonexistent() {
        let mut loader = PluginLoader::new();
        assert!(loader.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_case_insensitive_lookup_get() {
        // Test would need a real plugin to fully test, but we verify the
        // implementation uses to_lowercase()
        let loader = PluginLoader::new();
        // Both should return None since no plugins loaded
        assert!(loader.get("Plugin").is_none());
        assert!(loader.get("plugin").is_none());
        assert!(loader.get("PLUGIN").is_none());
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError {
            plugin_name: "test-plugin".to_string(),
            message: "Missing required field 'api_key'".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "test-plugin: Missing required field 'api_key'"
        );
    }

    #[test]
    fn test_config_error_is_error() {
        let err = ConfigError {
            plugin_name: "test-plugin".to_string(),
            message: "Config validation failed".to_string(),
        };
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_get_config_errors_empty_on_new() {
        let loader = PluginLoader::new();
        assert!(loader.get_config_errors().is_empty());
    }

    #[test]
    fn test_event_subscriptions_empty_on_new() {
        let loader = PluginLoader::new();
        let subs = loader.plugins_for_event(totui_plugin_interface::FfiEventType::OnAdd);
        assert!(subs.is_empty());
    }

    #[test]
    fn test_event_subscriptions_empty_for_all_types() {
        let loader = PluginLoader::new();
        assert!(
            loader
                .plugins_for_event(totui_plugin_interface::FfiEventType::OnAdd)
                .is_empty()
        );
        assert!(
            loader
                .plugins_for_event(totui_plugin_interface::FfiEventType::OnModify)
                .is_empty()
        );
        assert!(
            loader
                .plugins_for_event(totui_plugin_interface::FfiEventType::OnComplete)
                .is_empty()
        );
        assert!(
            loader
                .plugins_for_event(totui_plugin_interface::FfiEventType::OnDelete)
                .is_empty()
        );
        assert!(
            loader
                .plugins_for_event(totui_plugin_interface::FfiEventType::OnLoad)
                .is_empty()
        );
    }
}
