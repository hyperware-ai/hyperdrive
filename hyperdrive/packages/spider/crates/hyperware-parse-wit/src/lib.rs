use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::io::Read;
use std::path::Path;
use wit_parser::Resolve;
use zip::ZipArchive;

// Include the default hyperware.wit file
const DEFAULT_HYPERWARE_WIT: &str = include_str!("hyperware.wit");

/// Parse WIT files from a zip archive and return JSON representation
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip(zip_bytes: &[u8], fallback_wit: Option<Vec<u8>>) -> Result<String> {
    let resolve = parse_wit_from_zip_to_resolve(zip_bytes, fallback_wit)?;
    let json = serde_json::to_string_pretty(&resolve).context("failed to serialize to JSON")?;
    Ok(json)
}

/// Parse WIT files from a zip archive and return parsed Resolve
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip_to_resolve(
    zip_bytes: &[u8],
    fallback_wit: Option<Vec<u8>>,
) -> Result<Resolve> {
    // Open the zip archive from bytes
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).context("failed to open zip archive")?;

    let mut resolve = Resolve::default();
    let mut has_package_header = false;
    let mut wit_files = Vec::new();

    // First pass: collect all WIT files and check for package headers
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("failed to access zip entry")?;
        let name = file.name().to_string();

        // Skip directories
        if name.ends_with('/') {
            continue;
        }

        // Only process .wit files
        if !name.ends_with(".wit") {
            continue;
        }

        // Read the file contents
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("failed to read file: {}", name))?;

        // Check if this file contains a package header
        if contents.contains("package ") && contents.contains("@") {
            has_package_header = true;
        }

        wit_files.push((name, contents));
    }

    // If no package header found, combine all WIT files with the fallback package header
    if !has_package_header && !wit_files.is_empty() {
        let fallback_content = match fallback_wit {
            Some(bytes) => {
                String::from_utf8(bytes).context("fallback WIT content is not valid UTF-8")?
            }
            None => DEFAULT_HYPERWARE_WIT.to_string(),
        };

        // Combine all WIT files into a single document with the package header
        let mut combined_wit = fallback_content;
        combined_wit.push_str("\n\n");

        for (_, contents) in &wit_files {
            combined_wit.push_str(&contents);
            combined_wit.push_str("\n\n");
        }

        // Parse the combined WIT document
        resolve
            .push_str(Path::new("combined.wit"), &combined_wit)
            .context("failed to parse combined WIT package")?;
    } else {
        // Parse each file individually if package headers are present
        for (name, contents) in wit_files {
            let path = Path::new(&name);
            resolve
                .push_str(path, &contents)
                .with_context(|| format!("failed to parse WIT file: {}", name))?;
        }
    }

    Ok(resolve)
}

/// Parse WIT files from a zip archive and return serde_json::Value
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip_to_value(
    zip_bytes: &[u8],
    fallback_wit: Option<Vec<u8>>,
) -> Result<serde_json::Value> {
    let resolve = parse_wit_from_zip_to_resolve(zip_bytes, fallback_wit)?;
    let value = serde_json::to_value(&resolve).context("failed to convert to JSON value")?;
    Ok(value)
}

/// Parse WIT files from a zip archive and return rustified serde_json::Value
///
/// This is a wrapper around `parse_wit_from_zip_to_value` that converts
/// WIT naming conventions to Rust conventions:
/// - Type names: kebab-case → PascalCase
/// - Enum values: kebab-case → PascalCase
/// - Variant case names: kebab-case → PascalCase
/// - Property keys: kebab-case → snake_case
/// - Type references: kebab-case → PascalCase
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip_to_value_rustified(
    zip_bytes: &[u8],
    fallback_wit: Option<Vec<u8>>,
) -> Result<serde_json::Value> {
    let value = parse_wit_from_zip_to_value(zip_bytes, fallback_wit)?;
    Ok(rustify_value(value))
}

/// Convert kebab-case to PascalCase
pub fn to_pascal_case(s: &str) -> String {
    let parts = s.split('-');
    let mut result = String::new();

    for part in parts {
        if !part.is_empty() {
            let mut chars = part.chars();
            if let Some(first_char) = chars.next() {
                result.push(first_char.to_uppercase().next().unwrap());
                result.extend(chars);
            }
        }
    }

    result
}

/// Convert kebab-case to snake_case
pub fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

/// Recursively rustify a JSON value from WIT conventions to Rust conventions
fn rustify_value(value: Value) -> Value {
    match value {
        Value::Array(arr) => Value::Array(arr.into_iter().map(rustify_value).collect()),
        Value::Object(obj) => rustify_object(obj),
        other => other,
    }
}

/// Rustify a JSON object, handling WIT-specific structures
fn rustify_object(obj: Map<String, Value>) -> Value {
    // Check what kind of WIT structure this is
    let obj_type = obj.get("type").and_then(|v| v.as_str());

    match obj_type {
        Some("enum") => rustify_enum(obj),
        Some("variant") => rustify_variant(obj),
        Some("object") => rustify_struct(obj),
        Some("option") | Some("result") | Some("array") | Some("tuple") => {
            rustify_generic_type(obj)
        }
        None => {
            // Could be a top-level type definition or other structure
            rustify_type_definition(obj)
        }
        _ => {
            // Unknown type, just recurse
            let new_obj: Map<String, Value> = obj
                .into_iter()
                .map(|(k, v)| (k, rustify_value(v)))
                .collect();
            Value::Object(new_obj)
        }
    }
}

/// Rustify an enum definition - convert values to PascalCase
fn rustify_enum(mut obj: Map<String, Value>) -> Value {
    if let Some(Value::Array(values)) = obj.remove("values") {
        let rustified_values: Vec<Value> = values
            .into_iter()
            .map(|v| {
                if let Value::String(s) = v {
                    Value::String(to_pascal_case(&s))
                } else {
                    v
                }
            })
            .collect();
        obj.insert("values".to_string(), Value::Array(rustified_values));
    }

    // Recurse on other fields
    let new_obj: Map<String, Value> = obj
        .into_iter()
        .map(|(k, v)| (k, rustify_value(v)))
        .collect();
    Value::Object(new_obj)
}

/// Rustify a variant definition - convert case names to PascalCase
fn rustify_variant(mut obj: Map<String, Value>) -> Value {
    if let Some(Value::Array(cases)) = obj.remove("cases") {
        let rustified_cases: Vec<Value> = cases
            .into_iter()
            .map(|case| {
                if let Value::Object(mut case_obj) = case {
                    // Convert the case name to PascalCase
                    if let Some(Value::String(name)) = case_obj.remove("name") {
                        case_obj.insert("name".to_string(), Value::String(to_pascal_case(&name)));
                    }
                    // Recurse on the type field
                    if let Some(type_val) = case_obj.remove("type") {
                        case_obj.insert("type".to_string(), rustify_type_reference(type_val));
                    }
                    Value::Object(case_obj)
                } else {
                    case
                }
            })
            .collect();
        obj.insert("cases".to_string(), Value::Array(rustified_cases));
    }

    let new_obj: Map<String, Value> = obj
        .into_iter()
        .map(|(k, v)| (k, rustify_value(v)))
        .collect();
    Value::Object(new_obj)
}

/// Rustify a struct/object definition - convert property keys to snake_case
fn rustify_struct(mut obj: Map<String, Value>) -> Value {
    if let Some(Value::Object(props)) = obj.remove("properties") {
        let rustified_props: Map<String, Value> = props
            .into_iter()
            .map(|(k, v)| (to_snake_case(&k), rustify_type_reference(v)))
            .collect();
        obj.insert("properties".to_string(), Value::Object(rustified_props));
    }

    // Don't recurse on remaining fields - they're just metadata like "type": "object"
    Value::Object(obj)
}

/// Rustify generic types (option, result, array, tuple)
fn rustify_generic_type(mut obj: Map<String, Value>) -> Value {
    // Handle "value" field (option, result ok/err)
    if let Some(val) = obj.remove("value") {
        obj.insert("value".to_string(), rustify_type_reference(val));
    }
    if let Some(val) = obj.remove("ok") {
        obj.insert("ok".to_string(), rustify_type_reference(val));
    }
    if let Some(val) = obj.remove("err") {
        obj.insert("err".to_string(), rustify_type_reference(val));
    }
    // Handle "items" field (array, tuple)
    if let Some(items) = obj.remove("items") {
        obj.insert("items".to_string(), rustify_type_reference(items));
    }

    Value::Object(obj)
}

/// Rustify a top-level type definition
fn rustify_type_definition(mut obj: Map<String, Value>) -> Value {
    // Convert the type name to PascalCase
    if let Some(Value::String(name)) = obj.remove("name") {
        obj.insert("name".to_string(), Value::String(to_pascal_case(&name)));
    }

    // Recurse on definition
    if let Some(def) = obj.remove("definition") {
        obj.insert("definition".to_string(), rustify_type_reference(def));
    }

    // Recurse on other fields (args, returning, target, etc.)
    let new_obj: Map<String, Value> = obj
        .into_iter()
        .map(|(k, v)| (k, rustify_value(v)))
        .collect();
    Value::Object(new_obj)
}

/// Rustify a type reference - could be a string type name or a complex type
fn rustify_type_reference(value: Value) -> Value {
    match value {
        Value::String(s) => {
            // Convert WIT type to Rust type (handles primitives and custom types)
            Value::String(wit_type_to_rust(&s))
        }
        Value::Object(obj) => rustify_object(obj),
        Value::Array(arr) => Value::Array(arr.into_iter().map(rustify_type_reference).collect()),
        other => other,
    }
}

/// Convert a WIT type string to its Rust equivalent
/// This handles primitives and keeps custom types for PascalCase conversion
fn wit_type_to_rust(wit_type: &str) -> String {
    match wit_type {
        // Signed integer types: WIT uses s8/s16/s32/s64, Rust uses i8/i16/i32/i64
        "s8" => "i8".to_string(),
        "s16" => "i16".to_string(),
        "s32" => "i32".to_string(),
        "s64" => "i64".to_string(),
        // Unsigned integer types (same in both)
        "u8" => "u8".to_string(),
        "u16" => "u16".to_string(),
        "u32" => "u32".to_string(),
        "u64" => "u64".to_string(),
        // Floating point types (same in both)
        "f32" => "f32".to_string(),
        "f64" => "f64".to_string(),
        // String type: WIT uses lowercase, Rust uses String
        "string" => "String".to_string(),
        // Other primitives (same in both)
        "bool" => "bool".to_string(),
        "char" => "char".to_string(),
        // Unit type
        "_" => "()".to_string(),
        // Custom types get PascalCase conversion
        _ => to_pascal_case(wit_type),
    }
}

/// Check if a type name is a WIT primitive (used for deciding whether to recurse)
#[allow(dead_code)]
fn is_wit_primitive(s: &str) -> bool {
    matches!(
        s,
        "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "s8"
            | "s16"
            | "s32"
            | "s64"
            | "f32"
            | "f64"
            | "char"
            | "string"
            | "_"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("up-next"), "UpNext");
        assert_eq!(to_pascal_case("in-progress"), "InProgress");
        assert_eq!(to_pascal_case("this-week"), "ThisWeek");
        assert_eq!(to_pascal_case("low"), "Low");
        assert_eq!(to_pascal_case("node-id"), "NodeId");
        assert_eq!(to_pascal_case("NodeId"), "NodeId");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("publisher-node"), "publisher_node");
        assert_eq!(to_snake_case("package-name"), "package_name");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn test_is_wit_primitive() {
        assert!(is_wit_primitive("bool"));
        assert!(is_wit_primitive("u8"));
        assert!(is_wit_primitive("u64"));
        assert!(is_wit_primitive("s64"));
        assert!(is_wit_primitive("string"));
        assert!(is_wit_primitive("_"));
        assert!(!is_wit_primitive("NodeId"));
        assert!(!is_wit_primitive("my-type"));
    }

    #[test]
    fn test_wit_type_to_rust() {
        // Signed integers: s* -> i*
        assert_eq!(wit_type_to_rust("s8"), "i8");
        assert_eq!(wit_type_to_rust("s16"), "i16");
        assert_eq!(wit_type_to_rust("s32"), "i32");
        assert_eq!(wit_type_to_rust("s64"), "i64");
        // Unsigned stay the same
        assert_eq!(wit_type_to_rust("u8"), "u8");
        assert_eq!(wit_type_to_rust("u64"), "u64");
        // String -> String (capitalized)
        assert_eq!(wit_type_to_rust("string"), "String");
        // Unit type
        assert_eq!(wit_type_to_rust("_"), "()");
        // Custom types get PascalCase
        assert_eq!(wit_type_to_rust("my-custom-type"), "MyCustomType");
        assert_eq!(wit_type_to_rust("NodeId"), "NodeId");
    }

    #[test]
    fn test_rustify_enum() {
        let input = json!({
            "type": "enum",
            "values": ["low", "medium", "high", "up-next", "in-progress"]
        });
        let expected = json!({
            "type": "enum",
            "values": ["Low", "Medium", "High", "UpNext", "InProgress"]
        });
        assert_eq!(rustify_value(input), expected);
    }

    #[test]
    fn test_rustify_variant() {
        let input = json!({
            "type": "variant",
            "cases": [
                {"name": "request", "type": "Request"},
                {"name": "response", "type": "Response"}
            ]
        });
        let expected = json!({
            "type": "variant",
            "cases": [
                {"name": "Request", "type": "Request"},
                {"name": "Response", "type": "Response"}
            ]
        });
        assert_eq!(rustify_value(input), expected);
    }

    #[test]
    fn test_rustify_struct() {
        let input = json!({
            "type": "object",
            "properties": {
                "publisher-node": "NodeId",
                "package-name": "string"
            }
        });
        let expected = json!({
            "type": "object",
            "properties": {
                "publisher_node": "NodeId",
                "package_name": "String"
            }
        });
        assert_eq!(rustify_value(input), expected);
    }

    #[test]
    fn test_rustify_type_definition() {
        let input = json!({
            "name": "entry-status",
            "definition": {
                "type": "enum",
                "values": ["backlog", "up-next", "in-progress", "done"]
            }
        });
        let expected = json!({
            "name": "EntryStatus",
            "definition": {
                "type": "enum",
                "values": ["Backlog", "UpNext", "InProgress", "Done"]
            }
        });
        assert_eq!(rustify_value(input), expected);
    }

    #[test]
    fn test_rustify_converts_wit_primitives() {
        // WIT primitives get converted to Rust equivalents
        // string -> String, s64 -> i64, etc.
        let input = json!({
            "type": "object",
            "properties": {
                "count": "u64",
                "name": "string",
                "active": "bool",
                "timestamp": "s64"
            }
        });
        let expected = json!({
            "type": "object",
            "properties": {
                "count": "u64",
                "name": "String",
                "active": "bool",
                "timestamp": "i64"
            }
        });
        assert_eq!(rustify_value(input), expected);
    }

    #[test]
    fn test_rustify_nested_types() {
        let input = json!({
            "type": "option",
            "value": "my-custom-type"
        });
        let expected = json!({
            "type": "option",
            "value": "MyCustomType"
        });
        assert_eq!(rustify_value(input), expected);
    }

    #[test]
    fn test_rustify_full_enum_type_definition() {
        // Test the exact format from parse_wit_from_zip_to_value output
        let input = json!({
            "definition": {
                "type": "enum",
                "values": ["backlog", "up-next", "in-progress", "blocked", "review", "done"]
            },
            "documentation": null,
            "name": "EntryStatus",
            "process_name": "todo"
        });
        let result = rustify_value(input);

        // Check that enum values are converted to PascalCase
        let definition = result.get("definition").unwrap();
        let values = definition.get("values").unwrap().as_array().unwrap();
        let value_strings: Vec<&str> = values.iter().map(|v| v.as_str().unwrap()).collect();

        assert_eq!(
            value_strings,
            vec!["Backlog", "UpNext", "InProgress", "Blocked", "Review", "Done"]
        );
    }

    #[test]
    fn test_rustify_array_of_definitions() {
        // Test array input (like from parse_wit_from_zip_to_value)
        let input = json!([
            {
                "definition": {"type": "enum", "values": ["low", "medium", "high"]},
                "name": "EntryPriority",
                "process_name": "todo"
            },
            {
                "definition": {"type": "enum", "values": ["backlog", "up-next"]},
                "name": "EntryStatus"
            }
        ]);
        let result = rustify_value(input);
        let arr = result.as_array().unwrap();

        // Check first enum
        let first_values = arr[0]["definition"]["values"].as_array().unwrap();
        let first_strings: Vec<&str> = first_values.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(first_strings, vec!["Low", "Medium", "High"]);

        // Check second enum
        let second_values = arr[1]["definition"]["values"].as_array().unwrap();
        let second_strings: Vec<&str> = second_values.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(second_strings, vec!["Backlog", "UpNext"]);
    }
}
