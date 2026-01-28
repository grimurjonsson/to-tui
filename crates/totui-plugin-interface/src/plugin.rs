//! Plugin trait definition for FFI-safe plugin interface.
//!
//! This module defines the core Plugin trait using `#[sabi_trait]` from abi_stable,
//! which generates the necessary FFI-safe trait object types.

use abi_stable::sabi_trait;
use abi_stable::std_types::{RBox, RHashMap, RResult, RString, RVec};
use abi_stable::StableAbi;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::config::{FfiConfigSchema, FfiConfigValue};
use crate::events::{FfiEvent, FfiEventType, FfiHookResponse};
use crate::host_api::{FfiCommand, HostApi_TO};
use crate::types::FfiTodoItem;

/// FFI-safe wrapper for the update notifier callback.
///
/// Plugins call this when they have updates ready for the host to collect.
#[derive(StableAbi, Clone, Copy)]
#[repr(transparent)]
pub struct UpdateNotifier {
    /// The callback function to invoke when plugin has updates.
    pub func: extern "C" fn(),
}

/// The main plugin trait that all plugins must implement.
///
/// The `#[sabi_trait]` attribute generates `Plugin_TO`, a type-erased FFI-safe
/// trait object that can be safely passed across dynamic library boundaries.
///
/// # Example (for plugin implementors)
///
/// ```ignore
/// use totui_plugin_interface::{Plugin, FfiTodoItem, FfiTodoState};
/// use abi_stable::std_types::{RResult, RString, RVec, ROption};
///
/// #[derive(Debug)]
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn name(&self) -> RString {
///         "my-plugin".into()
///     }
///
///     fn version(&self) -> RString {
///         "1.0.0".into()
///     }
///
///     fn min_interface_version(&self) -> RString {
///         "0.2.0".into()
///     }
///
///     fn generate(&self, input: RString) -> RResult<RVec<FfiTodoItem>, RString> {
///         // Generate todos from input
///         RResult::ROk(RVec::new())
///     }
/// }
/// ```
#[sabi_trait]
pub trait Plugin: Send + Sync + Debug {
    /// Returns the plugin's display name.
    fn name(&self) -> RString;

    /// Returns the plugin's version in semver format (e.g., "1.0.0").
    fn version(&self) -> RString;

    /// Returns the minimum interface version this plugin requires.
    ///
    /// The host will check this against its interface version to ensure
    /// compatibility before calling any other methods.
    fn min_interface_version(&self) -> RString;

    /// Generate todos from the given input.
    ///
    /// This is the main entry point for plugin functionality. The plugin
    /// receives input (which may be empty) and returns a list of todo items.
    ///
    /// # Arguments
    ///
    /// * `input` - Plugin-specific input string (e.g., Jira ticket ID)
    ///
    /// # Returns
    ///
    /// * `RResult::ROk(items)` - Successfully generated todo items
    /// * `RResult::RErr(msg)` - Error message describing what went wrong
    fn generate(&self, input: RString) -> RResult<RVec<FfiTodoItem>, RString>;

    /// Return the plugin's config schema.
    ///
    /// The schema defines what configuration fields the plugin expects,
    /// their types, whether they're required, and default values.
    ///
    /// Plugins that need no configuration should return [`FfiConfigSchema::empty()`].
    ///
    /// # Returns
    ///
    /// The configuration schema for this plugin.
    fn config_schema(&self) -> FfiConfigSchema;

    /// Execute plugin logic with host API access.
    ///
    /// This method receives the host API for querying current state
    /// and returns a list of commands to be executed atomically.
    ///
    /// # Arguments
    ///
    /// * `input` - Plugin-specific input string
    /// * `host` - Host API trait object for querying todos and projects
    ///
    /// # Returns
    ///
    /// * `RResult::ROk(commands)` - List of commands to execute
    /// * `RResult::RErr(msg)` - Error message
    fn execute_with_host(
        &self,
        input: RString,
        host: HostApi_TO<'_, RBox<()>>,
    ) -> RResult<RVec<FfiCommand>, RString>;

    /// Called after configuration is loaded and validated.
    ///
    /// The host reads the plugin's config.toml file, validates it against
    /// the schema from [`config_schema()`], and passes the typed values here.
    /// This is called once at startup, before any calls to [`generate()`] or
    /// [`execute_with_host()`].
    ///
    /// Plugins should store the config in internal state (e.g., `Mutex<Option<Config>>`)
    /// for later use.
    ///
    /// # Arguments
    ///
    /// * `config` - Map of field names to validated config values
    fn on_config_loaded(&self, config: RHashMap<RString, FfiConfigValue>);

    /// Return event types this plugin wants to receive.
    ///
    /// Empty vec means plugin subscribes to no events.
    /// Called once at plugin load time.
    ///
    /// # Returns
    ///
    /// A vec of event types the plugin wants to handle.
    fn subscribed_events(&self) -> RVec<FfiEventType>;

    /// Handle an event hook.
    ///
    /// Called asynchronously when subscribed events occur.
    /// Should complete quickly (timeout applies).
    ///
    /// # Arguments
    ///
    /// * `event` - The event that occurred
    ///
    /// # Returns
    ///
    /// Commands to apply in response, or error message.
    fn on_event(&self, event: FfiEvent) -> RResult<FfiHookResponse, RString>;

    /// Set a notifier callback that the plugin can use to signal updates.
    ///
    /// When the plugin has updates ready (e.g., file watcher detected changes),
    /// it should call the notifier to tell the host to call `on_event`.
    ///
    /// Plugins should store this and call `notifier.func()` when they have updates.
    ///
    /// # Arguments
    ///
    /// * `notifier` - Wrapper containing the callback function
    #[sabi(last_prefix_field)]
    fn set_notifier(&self, notifier: UpdateNotifier);
}

/// Wrapper for calling plugin.generate() safely.
///
/// This function catches any panics from the plugin and converts them to
/// `RResult::RErr`, preventing panics from crossing the FFI boundary which
/// would cause undefined behavior.
///
/// # Arguments
///
/// * `plugin` - The plugin trait object to call
/// * `input` - Input to pass to the plugin's generate method
///
/// # Returns
///
/// The plugin's result, or an error if the plugin panicked.
pub fn call_plugin_generate(
    plugin: &Plugin_TO<'_, RBox<()>>,
    input: RString,
) -> RResult<RVec<FfiTodoItem>, RString> {
    let result = catch_unwind(AssertUnwindSafe(|| plugin.generate(input)));

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

/// Wrapper for calling plugin.execute_with_host() safely.
///
/// This function catches any panics from the plugin and converts them to
/// `RResult::RErr`, preventing panics from crossing the FFI boundary which
/// would cause undefined behavior.
///
/// # Arguments
///
/// * `plugin` - The plugin trait object to call
/// * `input` - Input to pass to the plugin's execute_with_host method
/// * `host` - Host API trait object for the plugin to query
///
/// # Returns
///
/// The plugin's result, or an error if the plugin panicked.
pub fn call_plugin_execute_with_host(
    plugin: &Plugin_TO<'_, RBox<()>>,
    input: RString,
    host: HostApi_TO<'_, RBox<()>>,
) -> RResult<RVec<FfiCommand>, RString> {
    let result = catch_unwind(AssertUnwindSafe(|| plugin.execute_with_host(input, host)));

    match result {
        Ok(r) => r,
        Err(panic_info) => {
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

/// Wrapper for calling plugin.on_config_loaded() safely.
///
/// This function catches any panics from the plugin, preventing panics from
/// crossing the FFI boundary which would cause undefined behavior.
///
/// # Arguments
///
/// * `plugin` - The plugin trait object to call
/// * `config` - The validated configuration to pass to the plugin
///
/// # Returns
///
/// `Ok(())` if the call succeeded, or an error message if the plugin panicked.
pub fn call_plugin_on_config_loaded(
    plugin: &Plugin_TO<'_, RBox<()>>,
    config: RHashMap<RString, FfiConfigValue>,
) -> Result<(), String> {
    let result = catch_unwind(AssertUnwindSafe(|| plugin.on_config_loaded(config)));

    match result {
        Ok(()) => Ok(()),
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                format!("Plugin panicked during config loading: {}", s)
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                format!("Plugin panicked during config loading: {}", s)
            } else {
                "Plugin panicked during config loading with unknown error".to_string()
            };
            Err(msg)
        }
    }
}

/// Wrapper for calling plugin.on_event() safely.
///
/// This function catches any panics from the plugin and converts them to
/// `RResult::RErr`, preventing panics from crossing the FFI boundary which
/// would cause undefined behavior.
///
/// # Arguments
///
/// * `plugin` - The plugin trait object to call
/// * `event` - The event to pass to the plugin's on_event method
///
/// # Returns
///
/// The plugin's result, or an error if the plugin panicked.
pub fn call_plugin_on_event(
    plugin: &Plugin_TO<'_, RBox<()>>,
    event: FfiEvent,
) -> RResult<FfiHookResponse, RString> {
    let result = catch_unwind(AssertUnwindSafe(|| plugin.on_event(event)));

    match result {
        Ok(r) => r,
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                format!("Plugin hook panicked: {}", s)
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                format!("Plugin hook panicked: {}", s)
            } else {
                "Plugin hook panicked with unknown error".to_string()
            };
            RResult::RErr(msg.into())
        }
    }
}
