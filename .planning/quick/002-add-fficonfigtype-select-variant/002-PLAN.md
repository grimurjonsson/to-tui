---
phase: quick-002
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/totui-plugin-interface/src/config.rs
  - src/plugin/config.rs
  - src/main.rs
autonomous: true

must_haves:
  truths:
    - "Plugin schemas can define Select fields with a list of allowed options"
    - "Config validation rejects values not in the allowed options list"
    - "Config template generation shows select type with available options"
  artifacts:
    - path: "crates/totui-plugin-interface/src/config.rs"
      provides: "FfiConfigType::Select variant and options field"
      contains: "Select = 4"
    - path: "src/plugin/config.rs"
      provides: "Select validation and template generation"
      contains: "FfiConfigType::Select"
    - path: "src/main.rs"
      provides: "Select type display in plugin config status"
      contains: "FfiConfigType::Select"
  key_links:
    - from: "src/plugin/config.rs"
      to: "crates/totui-plugin-interface/src/config.rs"
      via: "FfiConfigType import"
      pattern: "use totui_plugin_interface.*FfiConfigType"
---

<objective>
Add FfiConfigType::Select variant to totui-plugin-interface for dropdown/select configuration fields.

Purpose: Allow plugins to define config fields that accept one value from a predefined list of options (e.g., environment selection like "dev", "staging", "prod").

Output: Working Select config type that validates values against allowed options and generates appropriate config templates.
</objective>

<execution_context>
@/Users/gimmi/.claude/get-shit-done/workflows/execute-plan.md
@/Users/gimmi/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@crates/totui-plugin-interface/src/config.rs
@src/plugin/config.rs
@src/main.rs (lines 976-984 for FfiConfigType match)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add Select variant to FfiConfigType and options field to FfiConfigField</name>
  <files>crates/totui-plugin-interface/src/config.rs</files>
  <action>
1. Add `Select = 4` variant to `FfiConfigType` enum (after StringArray = 3)
2. Add `options` field to `FfiConfigField` struct:
   - Type: `RVec<RString>` (list of allowed string values)
   - Position: After `description` field
   - Purpose: Holds allowed values for Select type (empty for other types)

The Select type stores its value as a string (existing FfiConfigValue::String), so no changes needed to FfiConfigValue enum.

Example usage:
```rust
FfiConfigField {
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
}
```
  </action>
  <verify>cargo build -p totui-plugin-interface</verify>
  <done>FfiConfigType has Select = 4 variant, FfiConfigField has options: RVec<RString> field</done>
</task>

<task type="auto">
  <name>Task 2: Update host-side config validation and template generation</name>
  <files>src/plugin/config.rs</files>
  <action>
1. In `validate_field_type()`:
   - Add match arm for `(FfiConfigType::Select, Value::String(s))`
   - The function signature needs access to the field's options to validate
   - APPROACH: Change signature to accept `&FfiConfigField` instead of just `FfiConfigType`, OR pass options separately
   - RECOMMENDED: Pass options as optional parameter `options: Option<&RVec<RString>>`
   - Validation: If value is String AND (options is empty OR value is in options), return Ok(ConfigValue::String)
   - Error if value not in options: "{field_name}: value '{s}' is not one of the allowed options: {options_list}"

2. Update the call site in `load_and_validate()` to pass `&field.options` for Select validation

3. In `generate_config_template()`:
   - Add match arm for `FfiConfigType::Select => "select"`
   - After type comment, add options comment: `# Options: "opt1", "opt2", "opt3"`

4. In `get_example_value()`:
   - This function only has field_type, not options, so return `"\"option\""` as placeholder
   - OR change signature to accept options and return first option if available

5. Update `format_config_value()` if needed (Select uses String, should already work)

6. Add test for Select validation:
   - Test valid select value (value in options)
   - Test invalid select value (value not in options)
   - Test template generation shows options
  </action>
  <verify>cargo test -p to-tui config -- --nocapture</verify>
  <done>Select config fields validate against allowed options, templates show available options</done>
</task>

<task type="auto">
  <name>Task 3: Update main.rs type display and run full test suite</name>
  <files>src/main.rs</files>
  <action>
1. In `handle_plugin_config()` (around line 976-984), update the match on field.field_type:
   - Add: `FfiConfigType::Select => "select"`

2. Run full test suite to ensure no regressions

3. Optionally: In the schema fields display section, show options for Select fields:
   ```rust
   let type_name = match field.field_type {
       FfiConfigType::String => "string",
       FfiConfigType::Integer => "integer",
       FfiConfigType::Boolean => "boolean",
       FfiConfigType::StringArray => "string[]",
       FfiConfigType::Select => "select",
   };
   // After printing type, if Select and has options:
   if field.field_type == FfiConfigType::Select && !field.options.is_empty() {
       let opts: Vec<_> = field.options.iter().map(|s| s.as_str()).collect();
       println!("      Options: {}", opts.join(", "));
   }
   ```
  </action>
  <verify>cargo test && cargo clippy</verify>
  <done>All tests pass, clippy clean, Select type displays correctly in plugin config status</done>
</task>

</tasks>

<verification>
- [ ] `cargo build -p totui-plugin-interface` succeeds
- [ ] `cargo test` all tests pass
- [ ] `cargo clippy` no warnings
- [ ] Manual: Create test schema with Select field, verify validation works
</verification>

<success_criteria>
1. FfiConfigType::Select = 4 exists in plugin interface
2. FfiConfigField has `options: RVec<RString>` field
3. Config validation rejects values not in options list
4. Template generation shows "select" type and lists options
5. `totui plugin config <name>` shows "select" for Select fields
6. All existing tests pass (backwards compatible)
</success_criteria>

<output>
After completion, create `.planning/quick/002-add-fficonfigtype-select-variant/002-SUMMARY.md`
</output>
