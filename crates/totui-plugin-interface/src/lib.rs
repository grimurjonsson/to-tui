//! FFI-safe types for the to-tui plugin interface.
//!
//! This crate provides stable ABI types that can be safely passed across
//! dynamic library boundaries between the host application and plugins.

// Allow non-local definitions from abi_stable's sabi_trait macro.
// This is a known issue with the macro that generates impl blocks in a const.
// See: https://github.com/rust-lang/rust/issues/59629
#![allow(non_local_definitions)]

pub mod config;
pub mod events;
pub mod host_api;
pub mod plugin;
pub mod types;
pub mod version;

pub use config::{FfiConfigField, FfiConfigSchema, FfiConfigType, FfiConfigValue};
pub use events::{FfiEvent, FfiEventSource, FfiEventType, FfiFieldChange, FfiHookResponse};
pub use host_api::{
    FfiCommand, FfiMovePosition, FfiProjectContext, FfiStateFilter, FfiTodoMetadata, FfiTodoNode,
    FfiTodoQuery, HostApi, HostApi_TO,
};
pub use plugin::{
    call_plugin_execute_with_host, call_plugin_generate, call_plugin_on_config_loaded,
    call_plugin_on_event, Plugin, Plugin_TO,
};
pub use types::{FfiPriority, FfiTodoItem, FfiTodoState};
pub use version::{is_version_compatible, PluginModule, PluginModule_Ref, INTERFACE_VERSION};
