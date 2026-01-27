//! Version protocol and plugin module definition.
//!
//! This module provides the RootModule-based entry point for plugins and
//! version compatibility checking between plugins and the host application.

// abi_stable uses underscore naming convention for generated types
#![allow(non_camel_case_types)]

use abi_stable::{
    library::RootModule, package_version_strings, sabi_types::VersionStrings, std_types::RBox,
    StableAbi,
};
use semver::Version;

use crate::plugin::Plugin_TO;

/// Current interface crate version.
///
/// Plugins declare their minimum required interface version, and the host
/// uses this constant to check compatibility at load time.
pub const INTERFACE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Plugin library module - loaded from .so/.dylib/.dll.
///
/// This struct serves as the entry point for dynamically loaded plugins.
/// The host uses `PluginModule_Ref::load_from_directory()` to load plugins
/// and then calls `create_plugin()` to instantiate them.
///
/// # Example (for plugin implementors)
///
/// ```ignore
/// use totui_plugin_interface::{PluginModule, PluginModule_Ref, Plugin_TO};
/// use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, std_types::RBox};
///
/// #[export_root_module]
/// fn get_library() -> PluginModule_Ref {
///     PluginModule {
///         create_plugin,
///     }.leak_into_prefix()
/// }
///
/// extern "C" fn create_plugin() -> Plugin_TO<'static, RBox<()>> {
///     Plugin_TO::from_value(MyPlugin::new(), abi_stable::type_level::TD_Opaque)
/// }
/// ```
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginModule_Ref)))]
pub struct PluginModule {
    /// Factory function to create a plugin instance.
    ///
    /// Called by the host after loading the library and verifying version
    /// compatibility.
    #[sabi(last_prefix_field)]
    pub create_plugin: extern "C" fn() -> Plugin_TO<'static, RBox<()>>,
}

impl RootModule for PluginModule_Ref {
    abi_stable::declare_root_module_statics! {PluginModule_Ref}

    const BASE_NAME: &'static str = "totui_plugin";
    const NAME: &'static str = "to-tui plugin interface";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

/// Check if a plugin's minimum interface version is compatible with the host.
///
/// # Compatibility rules
///
/// - Same major version required (breaking changes only in major versions)
/// - Host version must be >= plugin's minimum version
///
/// This follows standard semver compatibility: a plugin compiled against
/// interface 0.1.0 will work with host 0.1.5 (same major, host newer),
/// but not with host 0.0.9 (host older) or host 1.0.0 (different major).
///
/// # Arguments
///
/// * `plugin_min_version` - The minimum interface version the plugin requires
/// * `host_version` - The interface version the host provides
///
/// # Returns
///
/// * `Ok(true)` - Versions are compatible
/// * `Ok(false)` - Versions are incompatible
/// * `Err(msg)` - Version string parsing failed
///
/// # Example
///
/// ```
/// use totui_plugin_interface::is_version_compatible;
///
/// // Same version - compatible
/// assert!(is_version_compatible("0.1.0", "0.1.0").unwrap());
///
/// // Host newer, same major - compatible
/// assert!(is_version_compatible("0.1.0", "0.2.0").unwrap());
///
/// // Host older - incompatible
/// assert!(!is_version_compatible("0.2.0", "0.1.0").unwrap());
///
/// // Different major - incompatible
/// assert!(!is_version_compatible("1.0.0", "0.9.0").unwrap());
/// ```
pub fn is_version_compatible(plugin_min_version: &str, host_version: &str) -> Result<bool, String> {
    let plugin_min = Version::parse(plugin_min_version)
        .map_err(|e| format!("Invalid plugin version '{}': {}", plugin_min_version, e))?;
    let host = Version::parse(host_version)
        .map_err(|e| format!("Invalid host version '{}': {}", host_version, e))?;

    // Compatible if same major and host >= plugin_min
    Ok(host.major == plugin_min.major && host >= plugin_min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compatible_same_major_same_version() {
        assert!(is_version_compatible("0.1.0", "0.1.0").unwrap());
    }

    #[test]
    fn test_compatible_same_major_host_newer() {
        assert!(is_version_compatible("0.1.0", "0.2.0").unwrap());
        assert!(is_version_compatible("0.1.0", "0.1.5").unwrap());
    }

    #[test]
    fn test_incompatible_different_major() {
        assert!(!is_version_compatible("1.0.0", "0.9.0").unwrap());
        assert!(!is_version_compatible("0.1.0", "1.0.0").unwrap());
    }

    #[test]
    fn test_incompatible_host_older() {
        assert!(!is_version_compatible("0.2.0", "0.1.0").unwrap());
    }

    #[test]
    fn test_invalid_version_string() {
        assert!(is_version_compatible("invalid", "0.1.0").is_err());
        assert!(is_version_compatible("0.1.0", "invalid").is_err());
    }

    #[test]
    fn test_interface_version_constant() {
        // Just verify it parses as valid semver
        Version::parse(INTERFACE_VERSION).expect("INTERFACE_VERSION should be valid semver");
    }
}
