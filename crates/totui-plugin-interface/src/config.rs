//! FFI-safe configuration types for plugin configuration.
//!
//! This module defines types for defining plugin config schemas and
//! passing typed config values across the FFI boundary.

use abi_stable::std_types::{ROption, RString, RVec};
use abi_stable::StableAbi;

/// FFI-safe config value types.
///
/// Supports string, integer, boolean, and array of strings as specified
/// in CONTEXT.md for plugin configuration.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiConfigValue {
    /// A string value
    String(RString),
    /// A 64-bit signed integer
    Integer(i64),
    /// A boolean value
    Boolean(bool),
    /// An array of strings
    StringArray(RVec<RString>),
}

/// FFI-safe config field type specifier for schema definitions.
///
/// Used in [`FfiConfigField`] to specify the expected type of a config field.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiConfigType {
    /// String type
    String = 0,
    /// 64-bit signed integer type
    Integer = 1,
    /// Boolean type
    Boolean = 2,
    /// Array of strings type
    StringArray = 3,
    /// Select type (string value from a predefined list of options)
    Select = 4,
}

/// FFI-safe config field definition.
///
/// Describes a single configuration field including its name, expected type,
/// whether it's required, and optional default value and description.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiConfigField {
    /// Field name in config.toml
    pub name: RString,
    /// Expected type of the field value
    pub field_type: FfiConfigType,
    /// Whether the field is required (if false, default must be provided)
    pub required: bool,
    /// Default value (used if field not present and not required)
    pub default: ROption<FfiConfigValue>,
    /// Human-readable description (for `totui plugin config <name> --init`)
    pub description: ROption<RString>,
    /// Allowed options for Select type (empty for other types)
    pub options: RVec<RString>,
}

/// FFI-safe config schema.
///
/// A collection of field definitions that describes the expected configuration
/// for a plugin. Used by the host to validate plugin configuration files.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiConfigSchema {
    /// List of field definitions
    pub fields: RVec<FfiConfigField>,
    /// Whether any config is required at all (if true and no config file exists, fail)
    pub config_required: bool,
}

impl FfiConfigSchema {
    /// Create an empty schema indicating no configuration is needed.
    pub fn empty() -> Self {
        Self {
            fields: RVec::new(),
            config_required: false,
        }
    }
}
