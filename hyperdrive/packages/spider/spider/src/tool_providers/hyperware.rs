use crate::tool_providers::{ToolExecutionCommand, ToolProvider};
use crate::types::{SpiderState, Tool};
use hyperware_parse_wit::parse_wit_from_zip_to_resolve;
use hyperware_process_lib::{get_blob, hyperapp::send, ProcessId, ProcessIdParseError, Request};
use serde_json::{json, Value};
use wit_parser::Docs;

pub struct HyperwareToolProvider;

impl HyperwareToolProvider {
    pub fn new() -> Self {
        Self
    }

    fn create_search_apis_tool(&self) -> Tool {
        Tool {
            name: "hyperware_search_apis".to_string(),
            description: "Search available APIs on Hyperware by querying the app store and filtering based on a search term.".to_string(),
            parameters: r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string","description":"Search term to filter available APIs (e.g., 'weather', 'database', 'auth')"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string","description":"Search term to filter available APIs (e.g., 'weather', 'database', 'auth')"}}}"#.to_string()),
        }
    }

    fn create_get_api_tool(&self) -> Tool {
        Tool {
            name: "hyperware_get_api".to_string(),
            description: "Get the detailed API documentation for a specific package, including all available types and methods.".to_string(),
            parameters: r#"{"type":"object","required":["package_id"],"properties":{"package_id":{"type":"string","description":"The package ID in the format 'package-name:publisher-node' (e.g., 'weather-app-9000:foo.os')"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["package_id"],"properties":{"package_id":{"type":"string","description":"The package ID in the format 'package-name:publisher-node' (e.g., 'weather-app-9000:foo.os')"}}}"#.to_string()),
        }
    }

    fn create_call_api_tool(&self) -> Tool {
        Tool {
            name: "hyperware_call_api".to_string(),
            description: "Call a specific API method on a Hyperware process to execute functionality.".to_string(),
            parameters: r#"{"type":"object","required":["process_id","method","args"],"properties":{"process_id":{"type":"string","description":"The process ID in the format 'process-name:package-name:publisher-node'"},"method":{"type":"string","description":"The method name to call on the process. By convention UpperCamelCase"},"args":{"type":"string","description":"JSON string of arguments to pass to the method"},"timeout":{"type":"number","description":"Optional timeout in seconds (default: 15)"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["process_id","method","args"],"properties":{"process_id":{"type":"string","description":"The process ID in the format 'process-name:package-name:publisher-node'"},"method":{"type":"string","description":"The method name to call on the process. By convention UpperCamelCase"},"args":{"type":"string","description":"JSON string of arguments to pass to the method"},"timeout":{"type":"number","description":"Optional timeout in seconds (default: 15)"}}}"#.to_string()),
        }
    }
}

impl ToolProvider for HyperwareToolProvider {
    fn get_tools(&self, _state: &SpiderState) -> Vec<Tool> {
        vec![
            self.create_search_apis_tool(),
            self.create_get_api_tool(),
            self.create_call_api_tool(),
        ]
    }

    fn should_include_tool(&self, tool_name: &str, _state: &SpiderState) -> bool {
        match tool_name {
            "hyperware_search_apis" | "hyperware_get_api" | "hyperware_call_api" => true,
            _ => false,
        }
    }

    fn prepare_execution(
        &self,
        tool_name: &str,
        parameters: &Value,
        _state: &SpiderState,
    ) -> Result<ToolExecutionCommand, String> {
        match tool_name {
            "hyperware_search_apis" => {
                let query = parameters
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing query parameter".to_string())?
                    .to_string();

                Ok(ToolExecutionCommand::HyperwareSearchApis { query })
            }
            "hyperware_get_api" => {
                let package_id = parameters
                    .get("package_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing package_id parameter".to_string())?
                    .to_string();

                Ok(ToolExecutionCommand::HyperwareGetApi { package_id })
            }
            "hyperware_call_api" => {
                let process_id = parameters
                    .get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing package_id parameter".to_string())?
                    .to_string();

                let method = parameters
                    .get("method")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing method parameter".to_string())?
                    .to_string();

                let args = parameters
                    .get("args")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing args parameter".to_string())?
                    .to_string();

                let timeout = parameters
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(15);

                Ok(ToolExecutionCommand::HyperwareCallApi {
                    process_id,
                    method,
                    args,
                    timeout,
                })
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    fn get_provider_id(&self) -> &str {
        "hyperware"
    }
}

// Helper functions for executing Hyperware operations

pub async fn search_apis(query: &str) -> Result<Value, String> {
    // First, get the list of all APIs from app-store
    let apis_request = serde_json::to_vec(&json!("Apis")).unwrap();
    let request = Request::to(("our", "main", "app-store", "sys"))
        .body(apis_request)
        .expects_response(5);

    let apis_response: Value = send(request)
        .await
        .map_err(|e| format!("Failed to get APIs list: {:?}", e))?;

    // Extract the APIs list
    let apis = apis_response
        .get("ApisResponse")
        .and_then(|r| r.get("apis"))
        .and_then(|a| a.as_array())
        .ok_or_else(|| "Invalid APIs response format".to_string())?;

    // Process each API to get its documentation
    let mut results: Vec<(String, Option<String>)> = Vec::new();

    for api in apis {
        let package_name = api
            .get("package_name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| "Missing package_name".to_string())?;
        let publisher_node = api
            .get("publisher_node")
            .and_then(|n| n.as_str())
            .ok_or_else(|| "Missing publisher_node".to_string())?;
        let package_id = format!("{}:{}", package_name, publisher_node);

        // Skip if package doesn't match the query (case-insensitive)
        let query_lower = query.to_lowercase();
        if !package_id.to_lowercase().contains(&query_lower) {
            continue;
        }

        // Try to get the package documentation
        match get_package_documentation(&package_id).await {
            Ok(docs) => {
                results.push((package_id, Some(docs)));
            }
            Err(_) => {
                // If we can't get docs, still include the package without docs
                results.push((package_id, None));
            }
        }
    }

    Ok(json!(results))
}

pub async fn get_api(package_id: &str) -> Result<Value, String> {
    // Split package_id into package_name and publisher_node
    let parts: Vec<&str> = package_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid package_id format. Expected 'package_name:publisher_node', got '{}'",
            package_id
        ));
    }
    let package_name = parts[0];
    let publisher_node = parts[1];

    // Request API zip from app-store
    let get_api_request = serde_json::to_vec(&json!({
        "GetApi": {
            "package_name": package_name,
            "publisher_node": publisher_node,
        }
    }))
    .unwrap();

    let request = Request::to(("our", "main", "app-store", "sys"))
        .body(get_api_request)
        .expects_response(5);

    let _response: Value = send(request)
        .await
        .map_err(|e| format!("Failed to get API: {:?}", e))?;

    // Check if we got a blob (zip file)
    let blob = get_blob();
    if blob.is_none() {
        return Err(format!("No API zip found for package {}", package_id));
    }

    let blob_bytes = blob.ok_or_else(|| "No blob received".to_string())?.bytes;

    // Parse the WIT files from the zip
    let resolve = parse_wit_from_zip_to_resolve(&blob_bytes, None)
        .map_err(|e| format!("Failed to parse WIT files: {:?}", e))?;

    // Extract type information with full definitions and documentation
    let mut types_with_definitions: Vec<Value> = Vec::new();
    let mut seen_types = std::collections::HashSet::new();

    // Iterate through all packages in the resolve
    for (_pkg_id, package) in resolve.packages.iter() {
        // Process interfaces
        for (iface_name, iface_id) in &package.interfaces {
            let iface = &resolve.interfaces[*iface_id];

            // Skip standard or lib interfaces but include important types
            if iface_name == "standard" || iface_name == "lib" {
                for (type_name, type_id) in &iface.types {
                    // Only include certain standard types that are commonly used
                    if matches!(
                        type_name.as_str(),
                        "address"
                            | "process-id"
                            | "package-id"
                            | "node-id"
                            | "capability"
                            | "request"
                            | "response"
                            | "message"
                    ) {
                        let rust_type_name = to_upper_camel_case(type_name);
                        if seen_types.insert(rust_type_name.clone()) {
                            let type_def = &resolve.types[*type_id];
                            let docs = extract_docs(&type_def.docs);
                            let type_schema = type_to_json_schema(type_def, &resolve);
                            types_with_definitions.push(json!({
                                "name": rust_type_name,
                                "definition": type_schema,
                                "documentation": docs
                            }));
                        }
                    }
                }
                continue;
            }

            // Keep process name in kebab-case
            let process_name = iface_name.clone();

            // Add types within the interface
            for (type_name, type_id) in &iface.types {
                let type_name_camel = to_upper_camel_case(type_name);

                // Skip types ending with SignatureHttp or SignatureRemote
                if type_name_camel.ends_with("SignatureHttp")
                    || type_name_camel.ends_with("SignatureRemote")
                {
                    continue;
                }

                if seen_types.insert(format!("{}::{}", process_name, type_name_camel)) {
                    let type_def = &resolve.types[*type_id];
                    let docs = extract_docs(&type_def.docs);
                    let type_schema = type_to_json_schema(type_def, &resolve);
                    types_with_definitions.push(json!({
                        "name": type_name_camel,
                        "process_name": process_name,
                        "definition": type_schema,
                        "documentation": docs
                    }));
                }
            }

            // Add functions within the interface with parameter/return type info
            for (func_name, func) in &iface.functions {
                let func_name_formatted = func_name.clone();

                if seen_types.insert(format!("{}::{}", process_name, func_name_formatted)) {
                    let docs = extract_docs(&func.docs);
                    let params_schema = func
                        .params
                        .iter()
                        .map(|(param_name, param_type)| {
                            json!({
                                "name": to_snake_case(param_name),
                                "type": type_ref_to_json(&param_type, &resolve)
                            })
                        })
                        .collect::<Vec<_>>();

                    let returns_schema = match &func.results {
                        wit_parser::Results::Named(named) => named
                            .iter()
                            .map(|(name, type_ref)| {
                                json!({
                                    "name": to_snake_case(name),
                                    "type": type_ref_to_json(&type_ref, &resolve)
                                })
                            })
                            .collect::<Vec<_>>(),
                        wit_parser::Results::Anon(type_ref) => {
                            vec![json!({"type": type_ref_to_json(&type_ref, &resolve)})]
                        }
                    };

                    types_with_definitions.push(json!({
                        "name": func_name_formatted,
                        "process_name": process_name,
                        "type": "function",
                        "parameters": params_schema,
                        "returns": returns_schema,
                        "documentation": docs
                    }));
                }
            }
        }
    }

    Ok(json!(types_with_definitions))
}

pub async fn call_api(
    process_id: &str,
    method: &str,
    args: &str,
    timeout: u64,
) -> Result<Value, String> {
    let process_id: ProcessId = process_id
        .parse()
        .map_err(|e: ProcessIdParseError| e.to_string())?;

    // Create request body with method and args
    let request_body = serde_json::to_vec(&json!({
        method: serde_json::from_str::<Value>(args).unwrap_or_else(|_| json!(args))
    }))
    .unwrap();

    // Send the request to the package
    let request = Request::to(("our", process_id))
        .body(request_body)
        .expects_response(timeout);

    let response: Value = send(request)
        .await
        .map_err(|e| format!("Failed to call API: {:?}", e))?;

    Ok(response)
}

async fn get_package_documentation(package_id: &str) -> Result<String, String> {
    // Split package_id into package_name and publisher_node
    let parts: Vec<&str> = package_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err("Invalid package_id format".to_string());
    }
    let package_name = parts[0];
    let publisher_node = parts[1];

    // Request API zip from app-store to get package-level docs
    let get_api_request = serde_json::to_vec(&json!({
        "GetApi": {
            "package_name": package_name,
            "publisher_node": publisher_node,
        }
    }))
    .unwrap();

    let request = Request::to(("our", "main", "app-store", "sys"))
        .body(get_api_request)
        .expects_response(5);

    let _response: Value = send(request)
        .await
        .map_err(|e| format!("Failed to get API: {:?}", e))?;

    // Check if we got a blob (zip file)
    let blob = get_blob();
    if blob.is_none() {
        return Err(format!("No API zip found for package {}", package_id));
    }

    let blob_bytes = blob.ok_or_else(|| "No blob received".to_string())?.bytes;

    // Parse the WIT files from the zip
    let resolve = parse_wit_from_zip_to_resolve(&blob_bytes, None)
        .map_err(|e| format!("Failed to parse WIT files: {:?}", e))?;

    // Try to find package-level documentation
    // Look through packages for one matching our package name
    for (_pkg_id, package) in resolve.packages.iter() {
        let pkg_name = &package.name;
        if pkg_name.name.contains(package_name) {
            // Check if package has documentation
            // Note: wit-parser Package doesn't have a docs field directly
            // Package docs would typically be in a README or main interface
            // For now, return formatted package info
            return Ok(format!(
                "Package: {} - Provides API interfaces and types",
                pkg_name.name
            ));
        }
    }

    // Fallback to basic description
    Ok(format!("API package: {}", package_name))
}

fn extract_docs(docs: &Docs) -> Option<String> {
    docs.contents.clone()
}

// Helper function to convert snake_case to UpperCamelCase
fn to_upper_camel_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>()
}

// Helper function to convert kebab-case to snake_case
fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

// Convert a WIT type definition to a JSON schema representation
fn type_to_json_schema(type_def: &wit_parser::TypeDef, resolve: &wit_parser::Resolve) -> Value {
    use wit_parser::TypeDefKind;

    match &type_def.kind {
        TypeDefKind::Record(record) => {
            let fields = record
                .fields
                .iter()
                .map(|field| {
                    (
                        to_snake_case(&field.name),
                        type_ref_to_json(&field.ty, resolve),
                    )
                })
                .collect::<serde_json::Map<String, Value>>();

            json!({
                "type": "object",
                "properties": fields
            })
        }
        TypeDefKind::Variant(variant) => {
            let cases = variant
                .cases
                .iter()
                .map(|case| {
                    let case_schema = match &case.ty {
                        Some(ty) => type_ref_to_json(ty, resolve),
                        None => json!("null"),
                    };
                    json!({
                        "name": case.name,
                        "type": case_schema
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "type": "variant",
                "cases": cases
            })
        }
        TypeDefKind::Enum(enum_def) => {
            let cases = enum_def
                .cases
                .iter()
                .map(|case| &case.name)
                .collect::<Vec<_>>();
            json!({
                "type": "enum",
                "values": cases
            })
        }
        TypeDefKind::List(ty) => {
            json!({
                "type": "array",
                "items": type_ref_to_json(ty, resolve)
            })
        }
        TypeDefKind::Tuple(tuple) => {
            let types = tuple
                .types
                .iter()
                .map(|ty| type_ref_to_json(ty, resolve))
                .collect::<Vec<_>>();
            json!({
                "type": "tuple",
                "items": types
            })
        }
        TypeDefKind::Option(ty) => {
            json!({
                "type": "option",
                "value": type_ref_to_json(ty, resolve)
            })
        }
        TypeDefKind::Result(result) => {
            json!({
                "type": "result",
                "ok": result.ok.as_ref().map(|ty| type_ref_to_json(ty, resolve)),
                "err": result.err.as_ref().map(|ty| type_ref_to_json(ty, resolve))
            })
        }
        TypeDefKind::Flags(flags) => {
            let flag_names = flags
                .flags
                .iter()
                .map(|flag| &flag.name)
                .collect::<Vec<_>>();
            json!({
                "type": "flags",
                "flags": flag_names
            })
        }
        TypeDefKind::Type(ty) => type_ref_to_json(ty, resolve),
        _ => json!("unknown"),
    }
}

// Convert a WIT type reference to a JSON representation
fn type_ref_to_json(type_ref: &wit_parser::Type, resolve: &wit_parser::Resolve) -> Value {
    use wit_parser::Type;

    match type_ref {
        Type::Bool => json!("bool"),
        Type::U8 => json!("u8"),
        Type::U16 => json!("u16"),
        Type::U32 => json!("u32"),
        Type::U64 => json!("u64"),
        Type::S8 => json!("s8"),
        Type::S16 => json!("s16"),
        Type::S32 => json!("s32"),
        Type::S64 => json!("s64"),
        Type::F32 => json!("f32"),
        Type::F64 => json!("f64"),
        Type::Char => json!("char"),
        Type::String => json!("string"),
        Type::Id(id) => {
            // Look up the referenced type
            if let Some(type_def) = resolve.types.get(*id) {
                // If it has a name, use the name; otherwise, inline the definition
                if let Some(name) = &type_def.name {
                    json!(to_upper_camel_case(name))
                } else {
                    type_to_json_schema(type_def, resolve)
                }
            } else {
                json!("unknown")
            }
        }
    }
}
