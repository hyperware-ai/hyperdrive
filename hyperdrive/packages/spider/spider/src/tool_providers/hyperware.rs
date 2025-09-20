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
            parameters: r#"{"type":"object","required":["process_id","method","args"],"properties":{"process_id":{"type":"string","description":"The process ID in the format 'process-name:package-name:publisher-node'"},"method":{"type":"string","description":"The method name to call on the package"},"args":{"type":"string","description":"JSON string of arguments to pass to the method"},"timeout":{"type":"number","description":"Optional timeout in seconds (default: 15)"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["package_id","method","args"],"properties":{"package_id":{"type":"string","description":"The package ID in the format 'package_name:publisher_node'"},"method":{"type":"string","description":"The method name to call on the package"},"args":{"type":"string","description":"JSON string of arguments to pass to the method"},"timeout":{"type":"number","description":"Optional timeout in seconds (default: 15)"}}}"#.to_string()),
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
                let package_id = parameters
                    .get("package_id")
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
                    package_id,
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

    //let body = String::from_utf8(response.body().to_vec())
    //    .map_err(|e| format!("Failed to parse response body: {:?}", e))?;

    //let apis_response: Value = serde_json::from_str(&body)
    //    .map_err(|e| format!("Failed to parse JSON response: {:?}", e))?;

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
        match get_api_documentation(&package_id).await {
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

    let _response = send(request)
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

    // Extract type information with documentation
    let mut types_with_docs: Vec<(String, Option<String>)> = Vec::new();

    // Iterate through all packages in the resolve
    for (_, package) in resolve.packages.iter() {
        // Add interfaces
        for (_, iface_id) in &package.interfaces {
            let iface = &resolve.interfaces[*iface_id];
            let type_name = iface.name.as_deref().unwrap_or("unnamed_interface");
            let docs = extract_docs(&iface.docs);
            types_with_docs.push((type_name.to_string(), docs));

            // Add functions within the interface
            for (func_name, _) in &iface.functions {
                let full_name = format!("{}.{}", type_name, func_name);
                types_with_docs.push((full_name, None)); // Function-level docs if available
            }
        }

        // Add worlds
        for (_, world_id) in &package.worlds {
            let world = &resolve.worlds[*world_id];
            let type_name = world.name.to_string();
            //let type_name = world.name.as_ref().map(|s| s.as_str()).unwrap_or("unnamed_world");
            let docs = extract_docs(&world.docs);
            types_with_docs.push((type_name.to_string(), docs));
        }
    }

    Ok(json!(types_with_docs))
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
    //let body = String::from_utf8(response.body().to_vec())
    //    .map_err(|e| format!("Failed to parse response body: {:?}", e))?;

    //// Try to parse as JSON, otherwise return as string
    //let result = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!(body));

    //Ok(result)
}

async fn get_api_documentation(package_id: &str) -> Result<String, String> {
    // This is a simplified version that just returns the package_id
    // In a full implementation, we would fetch and parse the actual documentation
    let parts: Vec<&str> = package_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err("Invalid package_id format".to_string());
    }

    // Try to get the API and extract package-level documentation
    match get_api(package_id).await {
        Ok(_api_data) => {
            // For now, just return a basic description
            Ok(format!("API package: {}", parts[0]))
        }
        Err(_) => Ok(format!("Package: {}", parts[0])),
    }
}

fn extract_docs(docs: &Docs) -> Option<String> {
    docs.contents.clone()
}
