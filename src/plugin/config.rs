//! Host-side plugin configuration loading and validation.
//!
//! This module handles reading, parsing, and validating plugin configuration files
//! against the schema provided by each plugin.

use abi_stable::std_types::{RHashMap, RString, RVec};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use toml::Value;

use totui_plugin_interface::{FfiConfigSchema, FfiConfigType, FfiConfigValue};

use crate::utils::paths::get_plugin_config_path;

/// Host-side configuration value type.
///
/// This is the native Rust equivalent of [`FfiConfigValue`] for use in the host
/// before conversion to FFI types.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// A string value
    String(String),
    /// A 64-bit signed integer
    Integer(i64),
    /// A boolean value
    Boolean(bool),
    /// An array of strings
    StringArray(Vec<String>),
}

/// Plugin configuration loader.
///
/// Handles loading and validating plugin configuration files against their schemas.
pub struct PluginConfigLoader;

impl PluginConfigLoader {
    /// Load and validate plugin config from ~/.config/to-tui/plugins/<name>/config.toml
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin
    /// * `schema` - The plugin's config schema from config_schema()
    ///
    /// # Returns
    ///
    /// A map of field names to validated config values, or an error if validation fails.
    pub fn load_and_validate(
        plugin_name: &str,
        schema: &FfiConfigSchema,
    ) -> Result<HashMap<String, ConfigValue>> {
        let config_path = get_plugin_config_path(plugin_name)?;

        // If no config file exists
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
                    let options = if field.field_type == FfiConfigType::Select {
                        Some(&field.options)
                    } else {
                        None
                    };
                    let typed_value =
                        Self::validate_field_type(&field_name, value, field.field_type, options)?;
                    result.insert(field_name, typed_value);
                }
                None => {
                    if field.required {
                        bail!("{}: required field is missing", field_name);
                    }
                    // Use default if provided
                    if let abi_stable::std_types::ROption::RSome(ref default) = field.default {
                        result.insert(field_name, Self::ffi_value_to_config_value(default));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Collect default values from the schema for all optional fields.
    pub fn collect_defaults(schema: &FfiConfigSchema) -> HashMap<String, ConfigValue> {
        let mut defaults = HashMap::new();
        for field in schema.fields.iter() {
            if let abi_stable::std_types::ROption::RSome(ref default) = field.default {
                defaults.insert(field.name.to_string(), Self::ffi_value_to_config_value(default));
            }
        }
        defaults
    }

    /// Validate that a TOML value matches the expected type.
    ///
    /// # Arguments
    ///
    /// * `field_name` - The field name for error messages
    /// * `value` - The TOML value to validate
    /// * `expected` - The expected type from the schema
    /// * `options` - For Select type, the allowed options (ignored for other types)
    ///
    /// # Returns
    ///
    /// The validated value converted to ConfigValue, or an error with field name context.
    pub fn validate_field_type(
        field_name: &str,
        value: &Value,
        expected: FfiConfigType,
        options: Option<&RVec<RString>>,
    ) -> Result<ConfigValue> {
        match (expected, value) {
            (FfiConfigType::String, Value::String(s)) => Ok(ConfigValue::String(s.clone())),
            (FfiConfigType::Integer, Value::Integer(i)) => Ok(ConfigValue::Integer(*i)),
            (FfiConfigType::Boolean, Value::Boolean(b)) => Ok(ConfigValue::Boolean(*b)),
            (FfiConfigType::StringArray, Value::Array(arr)) => {
                let strings: Result<Vec<String>> = arr
                    .iter()
                    .map(|v| match v {
                        Value::String(s) => Ok(s.clone()),
                        _ => bail!("{}: array must contain only strings", field_name),
                    })
                    .collect();
                Ok(ConfigValue::StringArray(strings?))
            }
            (FfiConfigType::Select, Value::String(s)) => {
                // Validate that value is in allowed options (if options provided)
                if let Some(opts) = options
                    && !opts.is_empty() && !opts.iter().any(|opt| opt.as_str() == s)
                {
                    let opts_list: Vec<_> = opts.iter().map(|o| format!("\"{}\"", o)).collect();
                    bail!(
                        "{}: value '{}' is not one of the allowed options: {}",
                        field_name,
                        s,
                        opts_list.join(", ")
                    );
                }
                Ok(ConfigValue::String(s.clone()))
            }
            _ => bail!(
                "{}: expected {:?}, got {}",
                field_name,
                expected,
                value.type_str()
            ),
        }
    }

    /// Convert an FFI config value to a host ConfigValue.
    fn ffi_value_to_config_value(ffi: &FfiConfigValue) -> ConfigValue {
        match ffi {
            FfiConfigValue::String(s) => ConfigValue::String(s.to_string()),
            FfiConfigValue::Integer(i) => ConfigValue::Integer(*i),
            FfiConfigValue::Boolean(b) => ConfigValue::Boolean(*b),
            FfiConfigValue::StringArray(arr) => {
                ConfigValue::StringArray(arr.iter().map(|s| s.to_string()).collect())
            }
        }
    }
}

/// Convert a host config map to FFI format for passing to plugins.
///
/// # Arguments
///
/// * `config` - The host-side config map
///
/// # Returns
///
/// An RHashMap suitable for passing across the FFI boundary.
pub fn to_ffi_config(config: &HashMap<String, ConfigValue>) -> RHashMap<RString, FfiConfigValue> {
    let mut ffi_map = RHashMap::new();
    for (key, value) in config {
        let ffi_value = match value {
            ConfigValue::String(s) => FfiConfigValue::String(RString::from(s.as_str())),
            ConfigValue::Integer(i) => FfiConfigValue::Integer(*i),
            ConfigValue::Boolean(b) => FfiConfigValue::Boolean(*b),
            ConfigValue::StringArray(arr) => {
                let rvec: RVec<RString> = arr.iter().map(|s| RString::from(s.as_str())).collect();
                FfiConfigValue::StringArray(rvec)
            }
        };
        ffi_map.insert(RString::from(key.as_str()), ffi_value);
    }
    ffi_map
}

/// Generate a template config file from a plugin's schema.
///
/// Creates a TOML-formatted string with:
/// - Comments for descriptions and type information
/// - Required fields with example values
/// - Optional fields commented out with defaults or examples
///
/// # Arguments
///
/// * `schema` - The plugin's config schema
///
/// # Returns
///
/// A TOML template string suitable for writing to config.toml
pub fn generate_config_template(schema: &FfiConfigSchema) -> String {
    let mut lines = Vec::new();

    lines.push("# Plugin Configuration".to_string());
    lines.push("# Generated from plugin schema".to_string());
    lines.push(String::new());

    for field in schema.fields.iter() {
        let field_name = field.name.to_string();
        let type_name = match field.field_type {
            FfiConfigType::String => "string",
            FfiConfigType::Integer => "integer",
            FfiConfigType::Boolean => "boolean",
            FfiConfigType::StringArray => "string array",
            FfiConfigType::Select => "select",
        };

        // Add description as comment if present
        if let abi_stable::std_types::ROption::RSome(ref desc) = field.description {
            lines.push(format!("# {}", desc));
        }

        // Add type and required/optional info
        let req_str = if field.required { "required" } else { "optional" };
        lines.push(format!("# Type: {} ({})", type_name, req_str));

        // For Select type, add options comment
        if field.field_type == FfiConfigType::Select && !field.options.is_empty() {
            let opts: Vec<_> = field.options.iter().map(|s| format!("\"{}\"", s)).collect();
            lines.push(format!("# Options: {}", opts.join(", ")));
        }

        // Generate the field line
        let example_value = match &field.default {
            abi_stable::std_types::ROption::RSome(default) => format_config_value(default),
            abi_stable::std_types::ROption::RNone => get_example_value(field.field_type),
        };

        if field.required {
            // Required fields are uncommented with example/default value
            lines.push(format!("{} = {}", field_name, example_value));
        } else {
            // Optional fields are commented out
            lines.push(format!("# {} = {}", field_name, example_value));
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

/// Format an FfiConfigValue for TOML output.
fn format_config_value(value: &FfiConfigValue) -> String {
    match value {
        FfiConfigValue::String(s) => format!("\"{}\"", s),
        FfiConfigValue::Integer(i) => i.to_string(),
        FfiConfigValue::Boolean(b) => b.to_string(),
        FfiConfigValue::StringArray(arr) => {
            let items: Vec<String> = arr.iter().map(|s| format!("\"{}\"", s)).collect();
            format!("[{}]", items.join(", "))
        }
    }
}

/// Get an example value for a given config type.
fn get_example_value(field_type: FfiConfigType) -> String {
    match field_type {
        FfiConfigType::String => "\"example\"".to_string(),
        FfiConfigType::Integer => "0".to_string(),
        FfiConfigType::Boolean => "false".to_string(),
        FfiConfigType::StringArray => "[\"item1\", \"item2\"]".to_string(),
        FfiConfigType::Select => "\"option\"".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::{ROption, RString, RVec};
    use totui_plugin_interface::{FfiConfigField, FfiConfigSchema};

    #[test]
    fn test_validate_field_type_string() {
        let value = Value::String("hello".to_string());
        let result = PluginConfigLoader::validate_field_type("test", &value, FfiConfigType::String, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::String("hello".to_string()));
    }

    #[test]
    fn test_validate_field_type_integer() {
        let value = Value::Integer(42);
        let result =
            PluginConfigLoader::validate_field_type("test", &value, FfiConfigType::Integer, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::Integer(42));
    }

    #[test]
    fn test_validate_field_type_boolean() {
        let value = Value::Boolean(true);
        let result =
            PluginConfigLoader::validate_field_type("test", &value, FfiConfigType::Boolean, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::Boolean(true));
    }

    #[test]
    fn test_validate_field_type_string_array() {
        let value = Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ]);
        let result =
            PluginConfigLoader::validate_field_type("test", &value, FfiConfigType::StringArray, None);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            ConfigValue::StringArray(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_validate_field_type_mismatch_includes_field_name() {
        let value = Value::String("not an integer".to_string());
        let result =
            PluginConfigLoader::validate_field_type("my_field", &value, FfiConfigType::Integer, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("my_field"),
            "Error message should contain field name: {}",
            err
        );
    }

    #[test]
    fn test_validate_field_type_string_array_mixed_types() {
        let value = Value::Array(vec![Value::String("a".to_string()), Value::Integer(42)]);
        let result =
            PluginConfigLoader::validate_field_type("tags", &value, FfiConfigType::StringArray, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("tags"), "Error should contain field name: {}", err);
    }

    #[test]
    fn test_collect_defaults_uses_schema_defaults() {
        let schema = FfiConfigSchema {
            config_required: false,
            fields: RVec::from(vec![
                FfiConfigField {
                    name: RString::from("timeout"),
                    field_type: FfiConfigType::Integer,
                    required: false,
                    default: ROption::RSome(FfiConfigValue::Integer(30)),
                    description: ROption::RNone,
                    options: RVec::new(),
                },
                FfiConfigField {
                    name: RString::from("debug"),
                    field_type: FfiConfigType::Boolean,
                    required: false,
                    default: ROption::RSome(FfiConfigValue::Boolean(false)),
                    description: ROption::RNone,
                    options: RVec::new(),
                },
                FfiConfigField {
                    name: RString::from("api_key"),
                    field_type: FfiConfigType::String,
                    required: true,
                    default: ROption::RNone,
                    description: ROption::RNone,
                    options: RVec::new(),
                },
            ]),
        };

        let defaults = PluginConfigLoader::collect_defaults(&schema);

        assert_eq!(defaults.len(), 2); // Only fields with defaults
        assert_eq!(defaults.get("timeout"), Some(&ConfigValue::Integer(30)));
        assert_eq!(defaults.get("debug"), Some(&ConfigValue::Boolean(false)));
        assert!(defaults.get("api_key").is_none()); // Required field has no default
    }

    #[test]
    fn test_to_ffi_config_converts_all_types() {
        let mut config = HashMap::new();
        config.insert("name".to_string(), ConfigValue::String("test".to_string()));
        config.insert("count".to_string(), ConfigValue::Integer(42));
        config.insert("enabled".to_string(), ConfigValue::Boolean(true));
        config.insert(
            "tags".to_string(),
            ConfigValue::StringArray(vec!["a".to_string(), "b".to_string()]),
        );

        let ffi = to_ffi_config(&config);

        assert_eq!(ffi.len(), 4);

        // Check string
        match ffi.get(&RString::from("name")) {
            Some(FfiConfigValue::String(s)) => assert_eq!(s.as_str(), "test"),
            _ => panic!("Expected string value for 'name'"),
        }

        // Check integer
        match ffi.get(&RString::from("count")) {
            Some(FfiConfigValue::Integer(i)) => assert_eq!(*i, 42),
            _ => panic!("Expected integer value for 'count'"),
        }

        // Check boolean
        match ffi.get(&RString::from("enabled")) {
            Some(FfiConfigValue::Boolean(b)) => assert!(*b),
            _ => panic!("Expected boolean value for 'enabled'"),
        }

        // Check string array
        match ffi.get(&RString::from("tags")) {
            Some(FfiConfigValue::StringArray(arr)) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0].as_str(), "a");
                assert_eq!(arr[1].as_str(), "b");
            }
            _ => panic!("Expected string array value for 'tags'"),
        }
    }

    #[test]
    fn test_generate_template_required_fields() {
        use super::generate_config_template;

        let schema = FfiConfigSchema {
            config_required: true,
            fields: RVec::from(vec![FfiConfigField {
                name: RString::from("api_key"),
                field_type: FfiConfigType::String,
                required: true,
                default: ROption::RNone,
                description: ROption::RNone,
                options: RVec::new(),
            }]),
        };

        let template = generate_config_template(&schema);

        // Required field should be uncommented
        assert!(template.contains("api_key = "));
        assert!(!template.contains("# api_key = "));
        // Should have type comment
        assert!(template.contains("# Type: string (required)"));
    }

    #[test]
    fn test_generate_template_optional_with_defaults() {
        use super::generate_config_template;

        let schema = FfiConfigSchema {
            config_required: false,
            fields: RVec::from(vec![FfiConfigField {
                name: RString::from("timeout"),
                field_type: FfiConfigType::Integer,
                required: false,
                default: ROption::RSome(FfiConfigValue::Integer(30)),
                description: ROption::RNone,
                options: RVec::new(),
            }]),
        };

        let template = generate_config_template(&schema);

        // Optional field should be commented out with default value
        assert!(template.contains("# timeout = 30"));
        // Should have type comment
        assert!(template.contains("# Type: integer (optional)"));
    }

    #[test]
    fn test_generate_template_with_descriptions() {
        use super::generate_config_template;

        let schema = FfiConfigSchema {
            config_required: true,
            fields: RVec::from(vec![FfiConfigField {
                name: RString::from("api_key"),
                field_type: FfiConfigType::String,
                required: true,
                default: ROption::RNone,
                description: ROption::RSome(RString::from("Your API key for authentication")),
                options: RVec::new(),
            }]),
        };

        let template = generate_config_template(&schema);

        // Description should appear as comment
        assert!(template.contains("# Your API key for authentication"));
    }

    #[test]
    fn test_validate_field_type_select_valid_option() {
        let value = Value::String("prod".to_string());
        let options = RVec::from(vec![
            RString::from("dev"),
            RString::from("staging"),
            RString::from("prod"),
        ]);
        let result = PluginConfigLoader::validate_field_type(
            "environment",
            &value,
            FfiConfigType::Select,
            Some(&options),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::String("prod".to_string()));
    }

    #[test]
    fn test_validate_field_type_select_invalid_option() {
        let value = Value::String("production".to_string());
        let options = RVec::from(vec![
            RString::from("dev"),
            RString::from("staging"),
            RString::from("prod"),
        ]);
        let result = PluginConfigLoader::validate_field_type(
            "environment",
            &value,
            FfiConfigType::Select,
            Some(&options),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("environment"));
        assert!(err.contains("production"));
        assert!(err.contains("not one of the allowed options"));
    }

    #[test]
    fn test_validate_field_type_select_empty_options() {
        // Empty options means any string is valid
        let value = Value::String("anything".to_string());
        let options = RVec::new();
        let result = PluginConfigLoader::validate_field_type(
            "freeform",
            &value,
            FfiConfigType::Select,
            Some(&options),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::String("anything".to_string()));
    }

    #[test]
    fn test_generate_template_select_with_options() {
        use super::generate_config_template;

        let schema = FfiConfigSchema {
            config_required: true,
            fields: RVec::from(vec![FfiConfigField {
                name: RString::from("environment"),
                field_type: FfiConfigType::Select,
                required: true,
                default: ROption::RSome(FfiConfigValue::String(RString::from("dev"))),
                description: ROption::RSome(RString::from("Target environment")),
                options: RVec::from(vec![
                    RString::from("dev"),
                    RString::from("staging"),
                    RString::from("prod"),
                ]),
            }]),
        };

        let template = generate_config_template(&schema);

        // Should show select type
        assert!(template.contains("# Type: select (required)"));
        // Should list options
        assert!(template.contains("# Options: \"dev\", \"staging\", \"prod\""));
        // Should use default value
        assert!(template.contains("environment = \"dev\""));
    }
}
