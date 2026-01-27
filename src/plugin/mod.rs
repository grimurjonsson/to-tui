pub mod actions;
pub mod command_executor;
pub mod config;
pub mod ffi_convert;
pub mod hooks;
pub mod host_impl;
pub mod installer;
pub mod loader;
pub mod manager;
pub mod manifest;
pub mod marketplace;
pub mod subprocess;

pub use actions::{PluginAction, PluginActionRegistry};
pub use command_executor::CommandExecutor;
pub use hooks::{HookDispatcher, HookResult};
pub use host_impl::PluginHostApiImpl;
pub use loader::{ConfigError, LoadedPlugin, PluginErrorKind, PluginLoadError, PluginLoader};
pub use manager::{PluginInfo, PluginManager, PluginSource};

/// Legacy plugin registry - now empty (built-in generators removed).
///
/// Kept for backwards compatibility during the transition to external plugins.
/// Use PluginLoader and PluginManager for external plugin support.
pub struct PluginRegistry;

impl PluginRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Vec<GeneratorInfo> {
        Vec::new()
    }

    pub fn get(&self, _name: &str) -> Option<&()> {
        None
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Info about a generator (legacy - kept for compatibility).
#[derive(Debug, Clone)]
pub struct GeneratorInfo {
    pub name: String,
    pub description: String,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}
