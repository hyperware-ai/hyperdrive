use crate::tool_providers::{ToolExecutionCommand, ToolProvider};
use crate::types::{
    HypergridConnection, HypergridMessage, HypergridMessageType, SpiderState, Tool,
    ToolResponseContent, ToolResponseContentItem,
};
use serde_json::Value;
use std::time::Instant;

pub struct HypergridToolProvider {
    server_id: String,
}

impl HypergridToolProvider {
    pub fn new(server_id: String) -> Self {
        Self { server_id }
    }

    // No longer needed since we're showing all tools unconditionally
    // fn is_authorized(&self, state: &SpiderState) -> bool {
    //     state.hypergrid_connections.contains_key(&self.server_id)
    // }

    fn create_authorize_tool(&self) -> Tool {
        Tool {
            name: "hypergrid_authorize".to_string(),
            description: "Configure Hypergrid connection credentials. Use this when you receive hypergrid auth strings.".to_string(),
            parameters: r#"{"type":"object","required":["url","token","client_id","node"],"properties":{"url":{"type":"string","description":"The base URL for the Hypergrid API (e.g., http://localhost:8080/operator:hypergrid:ware.hypr/shim/mcp)"},"token":{"type":"string","description":"The authentication token"},"client_id":{"type":"string","description":"The unique client ID"},"node":{"type":"string","description":"The Hyperware node name"},"name":{"type":"string","description":"Your identity (e.g., 'Claude', 'GPT-4', 'Gemini Pro')"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["url","token","client_id","node"],"properties":{"url":{"type":"string","description":"The base URL for the Hypergrid API (e.g., http://localhost:8080/operator:hypergrid:ware.hypr/shim/mcp)"},"token":{"type":"string","description":"The authentication token"},"client_id":{"type":"string","description":"The unique client ID"},"node":{"type":"string","description":"The Hyperware node name"},"name":{"type":"string","description":"Your identity (e.g., 'Claude', 'GPT-4', 'Gemini Pro')"}}}"#.to_string()),
        }
    }

    fn create_search_tool(&self) -> Tool {
        Tool {
            name: "hypergrid_search".to_string(),
            description: "Search the Hypergrid provider registry for available data providers.".to_string(),
            parameters: r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string","description":"Search query to find providers in the registry"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string","description":"Search query to find providers in the registry"}}}"#.to_string()),
        }
    }

    fn create_call_tool(&self) -> Tool {
        Tool {
            name: "hypergrid_call".to_string(),
            description: "Call a Hypergrid provider with arguments to retrieve data.".to_string(),
            parameters: r#"{"type":"object","required":["providerId","providerName","callArgs"],"properties":{"providerId":{"type":"string","description":"The ID of the provider to call"},"providerName":{"type":"string","description":"The name of the provider to call"},"callArgs":{"type":"array","items":{"type":"array","items":{"type":"string"}},"description":"Arguments to pass to the provider as an array of [key, value] pairs"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["providerId","providerName","callArgs"],"properties":{"providerId":{"type":"string","description":"The ID of the provider to call"},"providerName":{"type":"string","description":"The name of the provider to call"},"callArgs":{"type":"array","items":{"type":"array","items":{"type":"string"}},"description":"Arguments to pass to the provider as an array of [key, value] pairs"}}}"#.to_string()),
        }
    }

    fn create_parse_curl_tool(&self) -> Tool {
        let schema = r#"{"type":"object","required":["curlCommand"],"properties":{"curlCommand":{"type":"string","description":"A complete cURL command to parse. Include the full command with method, URL, headers, and body. Example: curl -X POST 'https://api.example.com/users' -H 'Authorization: Bearer token' -H 'Content-Type: application/json' -d '{\"name\": \"John\", \"age\": 30}'"},"suggestedParameters":{"type":"array","items":{"type":"string"},"description":"Optional: List of field names that should be made modifiable (e.g., ['name', 'age', 'id']). If omitted, intelligent defaults will be suggested based on common patterns."}}}"#;
        Tool {
            name: "hypergrid_parse_curl".to_string(),
            description: "Parse a cURL command into a structured endpoint configuration that can be used with hypergrid_register. This tool extracts the HTTP method, URL structure, headers, body, and identifies potential dynamic parameters. Use this BEFORE calling hypergrid_register to prepare the endpoint object.".to_string(),
            parameters: schema.to_string(),
            input_schema_json: Some(schema.to_string()),
        }
    }

    fn create_register_tool(&self) -> Tool {
        let schema = r#"{"type":"object","required":["providerName","providerId","description","instructions","registeredProviderWallet","price","endpoint"],"properties":{"providerName":{"type":"string","description":"The name of the provider"},"providerId":{"type":"string","description":"HNS entry (Node Identity) of the process serving as the provider"},"description":{"type":"string","description":"Description of the provider"},"instructions":{"type":"string","description":"Instructions for using the provider"},"registeredProviderWallet":{"type":"string","description":"Ethereum wallet address for payment"},"price":{"type":"number","description":"Price per call in USDC"},"endpoint":{"type":"object","required":["originalCurl","method","baseUrl","urlTemplate","originalHeaders","parameters","parameterNames"],"properties":{"originalCurl":{"type":"string","description":"Original curl command template"},"method":{"type":"string","description":"HTTP method (GET, POST, etc.)"},"baseUrl":{"type":"string","description":"Base URL of the endpoint"},"urlTemplate":{"type":"string","description":"URL template with parameter placeholders"},"originalHeaders":{"type":"array","items":{"type":"array","items":{"type":"string"},"minItems":2,"maxItems":2},"description":"Original headers as key-value pairs"},"originalBody":{"type":"string","description":"Original body template (optional)"},"parameters":{"type":"array","items":{"type":"object","required":["parameterName","jsonPointer","location","exampleValue","valueType"],"properties":{"parameterName":{"type":"string","description":"Name of the parameter"},"jsonPointer":{"type":"string","description":"JSON pointer to the parameter location (e.g., /body/user_id)"},"location":{"type":"string","description":"Parameter location: body, query, path, or header"},"exampleValue":{"type":"string","description":"Example value for the parameter"},"valueType":{"type":"string","description":"Data type: string, number, etc."}}},"description":"Parameter definitions for substitution"},"parameterNames":{"type":"array","items":{"type":"string"},"description":"List of parameter names"}}},"validationArguments":{"type":"array","items":{"type":"array","items":{"type":"string"},"minItems":2,"maxItems":2},"description":"Test parameter values to validate the endpoint before on-chain registration. Format: [[\"param1\", \"value1\"], [\"param2\", \"value2\"]]. Use the exampleValue from endpoint.parameters or provide realistic test values."}}}"#;
        Tool {
            name: "hypergrid_register".to_string(),
            description: "Register a new Hypergrid provider with its endpoint details, including curl template, parameters, and pricing. The endpoint will be validated before on-chain registration. Use hypergrid_parse_curl first to prepare the endpoint object from a cURL command.".to_string(),
            parameters: schema.to_string(),
            input_schema_json: Some(schema.to_string()),
        }
    }
}

impl ToolProvider for HypergridToolProvider {
    fn get_tools(&self, _state: &SpiderState) -> Vec<Tool> {
        vec![
            self.create_authorize_tool(),
            self.create_search_tool(),
            self.create_call_tool(),
            self.create_parse_curl_tool(),
            self.create_register_tool(),
        ]
    }

    fn should_include_tool(&self, tool_name: &str, _state: &SpiderState) -> bool {
        // Original conditional logic - commented out to always show all tools
        // match tool_name {
        //     "hypergrid_authorize" => !self.is_authorized(state),
        //     "hypergrid_search" | "hypergrid_call" => self.is_authorized(state),
        //     _ => false,
        // }

        // Always show all hypergrid tools
        match tool_name {
            "hypergrid_authorize" | "hypergrid_search" | "hypergrid_call" | "hypergrid_register" => true,
            _ => false,
        }
    }

    fn prepare_execution(
        &self,
        tool_name: &str,
        parameters: &Value,
        state: &SpiderState,
    ) -> Result<ToolExecutionCommand, String> {
        match tool_name {
            "hypergrid_authorize" => {
                let url = parameters
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing url parameter".to_string())?
                    .to_string();

                let token = parameters
                    .get("token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing token parameter".to_string())?
                    .to_string();

                let client_id = parameters
                    .get("client_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing client_id parameter".to_string())?
                    .to_string();

                let node = parameters
                    .get("node")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing node parameter".to_string())?
                    .to_string();

                let name = parameters
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                Ok(ToolExecutionCommand::HypergridAuthorize {
                    server_id: self.server_id.clone(),
                    url,
                    token,
                    client_id,
                    node,
                    name,
                })
            }
            "hypergrid_search" => {
                // Check if configured
                if !state.hypergrid_connections.contains_key(&self.server_id) {
                    return Err("Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string());
                }

                let query = parameters
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing query parameter".to_string())?
                    .to_string();

                Ok(ToolExecutionCommand::HypergridSearch {
                    server_id: self.server_id.clone(),
                    query,
                })
            }
            "hypergrid_call" => {
                // Check if configured
                if !state.hypergrid_connections.contains_key(&self.server_id) {
                    return Err("Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string());
                }

                let provider_id = parameters
                    .get("providerId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing providerId parameter".to_string())?
                    .to_string();

                let provider_name = parameters
                    .get("providerName")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing providerName parameter".to_string())?
                    .to_string();

                // Support both "callArgs" (old) and "arguments" (new) parameter names
                let call_args = parameters
                    .get("callArgs")
                    .or_else(|| parameters.get("arguments"))
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| "Missing callArgs or arguments parameter".to_string())?;

                let args: Vec<(String, String)> = call_args
                    .iter()
                    .filter_map(|arg| {
                        arg.as_array().and_then(|pair| {
                            if pair.len() == 2 {
                                let key = pair[0].as_str()?.to_string();
                                let value = pair[1].as_str()?.to_string();
                                Some((key, value))
                            } else {
                                None
                            }
                        })
                    })
                    .collect();

                Ok(ToolExecutionCommand::HypergridCall {
                    server_id: self.server_id.clone(),
                    provider_id,
                    provider_name,
                    call_args: args,
                })
            }
            "hypergrid_parse_curl" => {
                let curl_command = parameters
                    .get("curlCommand")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing curlCommand parameter".to_string())?
                    .to_string();

                let suggested_parameters = parameters
                    .get("suggestedParameters")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<String>>()
                    });

                Ok(ToolExecutionCommand::HypergridParseCurl {
                    server_id: self.server_id.clone(),
                    curl_command,
                    suggested_parameters,
                })
            }
            "hypergrid_register" => {
                let provider_name = parameters
                    .get("providerName")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing providerName parameter".to_string())?
                    .to_string();

                let provider_id = parameters
                    .get("providerId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing providerId parameter".to_string())?
                    .to_string();

                let description = parameters
                    .get("description")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing description parameter".to_string())?
                    .to_string();
                
                let endpoint = parameters
                    .get("endpoint")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| "Missing endpoint parameter".to_string())?;

                let instructions = parameters
                    .get("instructions")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing instructions parameter".to_string())?
                    .to_string();

                let registered_provider_wallet = parameters
                    .get("registeredProviderWallet")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing registeredProviderWallet parameter".to_string())?
                    .to_string();

                let price = parameters
                    .get("price")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "Missing price parameter".to_string())?;

                let endpoint = parameters
                    .get("endpoint")
                    .ok_or_else(|| "Missing endpoint parameter".to_string())?
                    .clone();

                Ok(ToolExecutionCommand::HypergridRegister {
                    server_id: self.server_id.clone(),
                    provider_name,
                    provider_id,
                    description,
                    instructions,
                    registered_provider_wallet,
                    price,
                    endpoint,
                })
            }
            _ => Err(format!("Unknown hypergrid tool: {}", tool_name)),
        }
    }

    fn get_provider_id(&self) -> &str {
        &self.server_id
    }
}

// Extension trait for hypergrid operations
pub trait HypergridExt {
    async fn execute_hypergrid_authorize_impl(
        &mut self,
        server_id: String,
        url: String,
        token: String,
        client_id: String,
        node: String,
        name: Option<String>,
    ) -> Result<Value, String>;
    async fn execute_hypergrid_search_impl(
        &mut self,
        server_id: String,
        query: String,
    ) -> Result<Value, String>;
    async fn execute_hypergrid_call_impl(
        &mut self,
        server_id: String,
        provider_id: String,
        provider_name: String,
        call_args: Vec<(String, String)>,
    ) -> Result<Value, String>;
    async fn execute_hypergrid_parse_curl_impl(
        &mut self,
        server_id: String,
        curl_command: String,
        suggested_parameters: Option<Vec<String>>,
    ) -> Result<Value, String>;
    async fn execute_hypergrid_register_impl(
        &mut self,
        server_id: String,
        provider_name: String,
        provider_id: String,
        description: String,
        instructions: String,
        registered_provider_wallet: String,
        price: f64,
        endpoint: Value,
    ) -> Result<Value, String>;
    async fn test_hypergrid_connection(
        &self,
        url: &str,
        token: &str,
        client_id: &str,
    ) -> Result<(), String>;
    async fn call_hypergrid_api(
        &self,
        url: &str,
        token: &str,
        client_id: &str,
        message: &HypergridMessage,
    ) -> Result<String, String>;
}

// === cURL Parsing Helper Functions ===

#[derive(Debug, Clone)]
struct ParsedCurlRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<serde_json::Value>,
    base_url: String,
    path_segments: Vec<String>,
    query_params: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct ModifiableField {
    parameter_name: String,
    json_pointer: String,
    location: String, // "path", "query", "header", "body"
    example_value: String,
    value_type: String,
}

/// Simple cURL parser - extracts method, URL, headers, and body
fn parse_curl_command(curl: &str) -> Result<ParsedCurlRequest, String> {
    let curl = curl.trim();
    
    // Extract method (default to GET if not specified)
    let method = if curl.contains("-X POST") || curl.contains("--request POST") {
        "POST".to_string()
    } else if curl.contains("-X PUT") || curl.contains("--request PUT") {
        "PUT".to_string()
    } else if curl.contains("-X DELETE") || curl.contains("--request DELETE") {
        "DELETE".to_string()
    } else if curl.contains("-X PATCH") || curl.contains("--request PATCH") {
        "PATCH".to_string()
    } else {
        "GET".to_string()
    };
    
    // Extract URL - find first occurrence of http:// or https://
    let url_start = curl.find("http://").or_else(|| curl.find("https://"))
        .ok_or_else(|| "No URL found in cURL command".to_string())?;
    
    let url_part = &curl[url_start..];
    let url_end = url_part.find(|c: char| c == '\'' || c == '"' || c == ' ')
        .unwrap_or(url_part.len());
    let url = url_part[..url_end].trim_matches(|c| c == '\'' || c == '"').to_string();
    
    // Parse URL components
    let parsed_url = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {}", e))?;
    let base_url = format!("{}://{}", parsed_url.scheme(), parsed_url.host_str().unwrap_or(""));
    let path_segments: Vec<String> = parsed_url.path()
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let query_params: Vec<(String, String)> = parsed_url.query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    
    // Extract headers - simple pattern matching
    let mut headers = Vec::new();
    for line in curl.lines() {
        let line = line.trim();
        if line.starts_with("-H ") || line.starts_with("--header ") {
            // Extract header from patterns like: -H 'Key: Value' or -H "Key: Value"
            if let Some(header_start) = line.find('\'').or_else(|| line.find('"')) {
                let quote_char = line.chars().nth(header_start).unwrap();
                if let Some(header_end) = line[header_start + 1..].find(quote_char) {
                    let header_content = &line[header_start + 1..header_start + 1 + header_end];
                    if let Some(colon_pos) = header_content.find(':') {
                        let key = header_content[..colon_pos].trim().to_string();
                        let value = header_content[colon_pos + 1..].trim().to_string();
                        headers.push((key, value));
                    }
                }
            }
        }
    }
    
    // Extract body (look for -d or --data)
    let body = if let Some(data_start) = curl.find("-d ").or_else(|| curl.find("--data ")) {
        let body_part = &curl[data_start + 3..];
        let body_str = if body_part.starts_with('\'') {
            // Single-quoted body
            let end = body_part[1..].find('\'').unwrap_or(body_part.len() - 1);
            &body_part[1..end + 1]
        } else if body_part.starts_with('"') {
            // Double-quoted body
            let end = body_part[1..].find('"').unwrap_or(body_part.len() - 1);
            &body_part[1..end + 1]
        } else {
            // Unquoted - take until next space or flag
            let end = body_part.find(' ').unwrap_or(body_part.len());
            &body_part[..end]
        };
        
        // Try to parse as JSON
        serde_json::from_str(body_str).ok()
    } else {
        None
    };
    
    Ok(ParsedCurlRequest {
        method,
        url: url.clone(),
        headers,
        body,
        base_url,
        path_segments,
        query_params,
    })
}

/// Identify which fields should be modifiable based on the parsed request
fn identify_modifiable_fields(
    parsed: &ParsedCurlRequest,
    suggested_params: Option<&Vec<String>>,
) -> Vec<ModifiableField> {
    let mut fields = Vec::new();
    
    // Add path segments that look like parameters
    for (index, segment) in parsed.path_segments.iter().enumerate() {
        if is_likely_parameter(segment) {
            fields.push(ModifiableField {
                parameter_name: format!("path{}", index),
                json_pointer: format!("/path/{}", index),
                location: "path".to_string(),
                example_value: segment.clone(),
                value_type: infer_type(segment),
            });
        }
    }
    
    // Add all query parameters
    for (key, value) in &parsed.query_params {
        fields.push(ModifiableField {
            parameter_name: key.clone(),
            json_pointer: format!("/query/{}", key),
            location: "query".to_string(),
            example_value: value.clone(),
            value_type: infer_type(value),
        });
    }
    
    // Add non-standard headers (skip common ones)
    for (key, value) in &parsed.headers {
        if !is_standard_header(key) {
            fields.push(ModifiableField {
                parameter_name: key.clone(),
                json_pointer: format!("/headers/{}", key),
                location: "header".to_string(),
                example_value: value.clone(),
                value_type: "string".to_string(),
            });
        }
    }
    
    // Add body fields if body exists
    if let Some(body) = &parsed.body {
        add_body_fields(body, "/body", &mut fields, suggested_params);
    }
    
    fields
}

/// Recursively add body fields
fn add_body_fields(
    value: &serde_json::Value,
    path: &str,
    fields: &mut Vec<ModifiableField>,
    suggested_params: Option<&Vec<String>>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let pointer = format!("{}/{}", path, key);
                
                // Check if this field is in suggested parameters
                let is_suggested = suggested_params
                    .map(|params| params.contains(key))
                    .unwrap_or(true); // If no suggestions, include all
                
                if is_suggested {
                    match val {
                        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                            // For complex types, add the whole object/array
                            fields.push(ModifiableField {
                                parameter_name: key.clone(),
                                json_pointer: pointer.clone(),
                                location: "body".to_string(),
                                example_value: val.to_string(),
                                value_type: if val.is_array() { "array" } else { "object" }.to_string(),
                            });
                        }
                        _ => {
                            // For primitives, add directly
                            fields.push(ModifiableField {
                                parameter_name: key.clone(),
                                json_pointer: pointer,
                                location: "body".to_string(),
                                example_value: val.as_str().unwrap_or(&val.to_string()).to_string(),
                                value_type: infer_type_from_json(val),
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Build the endpoint object from parsed cURL and identified fields
fn build_endpoint_from_parsed(
    parsed: &ParsedCurlRequest,
    fields: &[ModifiableField],
) -> Result<serde_json::Value, String> {
    // Build URL template with parameter placeholders
    let mut url_template = parsed.base_url.clone();
    if !parsed.path_segments.is_empty() {
        url_template.push('/');
        for (index, segment) in parsed.path_segments.iter().enumerate() {
            if index > 0 {
                url_template.push('/');
            }
            // Check if this segment is a modifiable field
            let json_ptr = format!("/path/{}", index);
            if let Some(field) = fields.iter().find(|f| f.json_pointer == json_ptr) {
                url_template.push_str(&format!("{{{}}}", field.parameter_name));
            } else {
                url_template.push_str(segment);
            }
        }
    }
    
    // Convert fields to the backend parameter format
    let parameters: Vec<serde_json::Value> = fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "parameter_name": field.parameter_name,
                "json_pointer": field.json_pointer,
                "location": field.location,
                "example_value": field.example_value,
                "value_type": field.value_type,
            })
        })
        .collect();
    
    let parameter_names: Vec<String> = fields.iter()
        .map(|f| f.parameter_name.clone())
        .collect();
    
    Ok(serde_json::json!({
        "original_curl": parsed.url,
        "method": parsed.method,
        "base_url": parsed.base_url,
        "url_template": url_template,
        "original_headers": parsed.headers,
        "original_body": parsed.body.as_ref().map(|b| b.to_string()),
        "parameters": parameters,
        "parameter_names": parameter_names,
    }))
}

fn is_likely_parameter(segment: &str) -> bool {
    // Numeric ID
    if segment.chars().all(|c| c.is_numeric()) && !segment.is_empty() {
        return true;
    }
    
    // UUID pattern (simple check)
    if segment.len() == 36 && segment.chars().filter(|c| *c == '-').count() == 4 {
        return true;
    }
    
    // Long alphanumeric strings (likely IDs)
    if segment.len() >= 8 && segment.chars().all(|c| c.is_alphanumeric()) {
        return true;
    }
    
    false
}

fn is_standard_header(key: &str) -> bool {
    let lower = key.to_lowercase();
    matches!(
        lower.as_str(),
        "content-type" | "accept" | "user-agent" | "host" | "connection" | 
        "cache-control" | "accept-encoding" | "accept-language" | "content-length"
    )
}

fn infer_type(value: &str) -> String {
    if value.parse::<i64>().is_ok() {
        "number".to_string()
    } else if value == "true" || value == "false" {
        "boolean".to_string()
    } else {
        "string".to_string()
    }
}

fn infer_type_from_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
        _ => "string".to_string(),
    }
}

impl HypergridExt for SpiderState {
    async fn execute_hypergrid_authorize_impl(
        &mut self,
        server_id: String,
        url: String,
        token: String,
        client_id: String,
        node: String,
        name: Option<String>,
    ) -> Result<Value, String> {
        println!(
            "Spider: hypergrid_authorize called for server_id: {}",
            server_id
        );
        println!("Spider: Authorizing hypergrid with:");
        println!("  - URL: {}", url);
        println!("  - Token: {}...", &token[..token.len().min(20)]);
        println!("  - Client ID: {}", client_id);
        println!("  - Node: {}", node);
        if let Some(ref n) = name {
            println!("  - Name: {}", n);
        }

        // Test new connection
        println!("Spider: Testing hypergrid connection...");
        self.test_hypergrid_connection(&url, &token, &client_id)
            .await?;
        println!("Spider: Connection test successful!");

        // Create or update the hypergrid connection
        let hypergrid_conn = HypergridConnection {
            server_id: server_id.clone(),
            url: url.clone(),
            token: token.clone(),
            client_id: client_id.clone(),
            node: node.clone(),
            last_retry: Instant::now(),
            retry_count: 0,
            connected: true,
        };

        self.hypergrid_connections
            .insert(server_id.clone(), hypergrid_conn);
        println!("Spider: Stored hypergrid connection in memory");

        // Update transport config
        if let Some(server) = self.mcp_servers.iter_mut().find(|s| s.id == server_id) {
            println!("Spider: Updating server '{}' transport config", server.name);
            server.transport.url = Some(url.clone());
            server.transport.hypergrid_token = Some(token.clone());
            server.transport.hypergrid_client_id = Some(client_id.clone());
            server.transport.hypergrid_node = Some(node.clone());
            println!("Spider: Server transport config updated successfully");
            println!("Spider: State should auto-save due to SaveOptions::OnDiff");
        } else {
            println!(
                "Spider: WARNING - Could not find server with id: {}",
                server_id
            );
        }

        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "text".to_string(),
                text: format!("✅ Successfully authorized! Hypergrid is now configured with:\n- Node: {}\n- Client ID: {}\n- URL: {}", node, client_id, url),
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn execute_hypergrid_search_impl(
        &mut self,
        server_id: String,
        query: String,
    ) -> Result<Value, String> {
        let hypergrid_conn = self.hypergrid_connections.get(&server_id)
            .ok_or_else(|| "Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string())?;

        let response = self
            .call_hypergrid_api(
                &hypergrid_conn.url,
                &hypergrid_conn.token,
                &hypergrid_conn.client_id,
                &HypergridMessage {
                    request: HypergridMessageType::SearchRegistry(query),
                },
            )
            .await?;

        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "text".to_string(),
                text: response,
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn execute_hypergrid_call_impl(
        &mut self,
        server_id: String,
        provider_id: String,
        provider_name: String,
        call_args: Vec<(String, String)>,
    ) -> Result<Value, String> {
        let hypergrid_conn = self.hypergrid_connections.get(&server_id)
            .ok_or_else(|| "Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string())?;

        let response = self
            .call_hypergrid_api(
                &hypergrid_conn.url,
                &hypergrid_conn.token,
                &hypergrid_conn.client_id,
                &HypergridMessage {
                    request: HypergridMessageType::CallProvider {
                        provider_id,
                        provider_name,
                        arguments: call_args,
                    },
                },
            )
            .await?;

        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "text".to_string(),
                text: response,
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn execute_hypergrid_parse_curl_impl(
        &mut self,
        _server_id: String,
        curl_command: String,
        suggested_parameters: Option<Vec<String>>,
    ) -> Result<Value, String> {
        println!("Spider: Parsing cURL command...");
        
        // Parse the cURL command using a simple parser
        let parsed = parse_curl_command(&curl_command)?;
        
        // Identify potential modifiable fields
        let potential_fields = identify_modifiable_fields(&parsed, suggested_parameters.as_ref());
        
        // Build the endpoint object in the format expected by the backend
        let endpoint = build_endpoint_from_parsed(&parsed, &potential_fields)?;
        
        println!("Spider: cURL parsing successful");
        println!("  - Method: {}", endpoint.get("method").and_then(|v| v.as_str()).unwrap_or("unknown"));
        println!("  - URL Template: {}", endpoint.get("urlTemplate").and_then(|v| v.as_str()).unwrap_or("unknown"));
        println!("  - Parameters: {}", endpoint.get("parameterNames").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));
        
        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "application/json".to_string(),
                text: serde_json::to_string_pretty(&endpoint).unwrap_or_else(|_| "{}".to_string()),
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn execute_hypergrid_register_impl(
        &mut self,
        server_id: String,
        provider_name: String,
        provider_id: String,
        description: String,
        instructions: String,
        registered_provider_wallet: String,
        price: f64,
        endpoint: Value,
    ) -> Result<Value, String> {
        // Check if configured
        let hypergrid_conn = self.hypergrid_connections.get(&server_id)
            .ok_or_else(|| "Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string())?;
        
        let response = self
            .call_hypergrid_api(
                &hypergrid_conn.url,
                &hypergrid_conn.token,
                &hypergrid_conn.client_id,
                &HypergridMessage {
                    request: HypergridMessageType::RegisterProvider {
                        provider_name: provider_name.clone(),
                        provider_id: provider_id.clone(),
                        description,
                        instructions,
                        registered_provider_wallet,
                        price,
                        endpoint,
                    },
                },
            )
            .await?;

        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "text".to_string(),
                text: response,
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn test_hypergrid_connection(
        &self,
        url: &str,
        token: &str,
        client_id: &str,
    ) -> Result<(), String> {
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;
        use std::collections::HashMap;

        println!(
            "Spider: test_hypergrid_connection - Testing connection to {}",
            url
        );

        let test_message = HypergridMessage {
            request: HypergridMessageType::SearchRegistry("test".to_string()),
        };

        let body = serde_json::to_string(&test_message)
            .map_err(|e| format!("Failed to serialize test message: {}", e))?;

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Auth-Token".to_string(), token.to_string());
        headers.insert("X-Client-Id".to_string(), client_id.to_string());

        let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

        println!("Spider: test_hypergrid_connection - Sending test request...");
        let response = send_request_await_response(
            Method::POST,
            parsed_url,
            Some(headers),
            30000,
            body.into_bytes(),
        )
        .await
        .map_err(|e| {
            println!(
                "Spider: test_hypergrid_connection - Request failed: {:?}",
                e
            );
            format!("Connection test failed: {:?}", e)
        })?;

        if !response.status().is_success() {
            let error_text = String::from_utf8_lossy(response.body());
            println!(
                "Spider: test_hypergrid_connection - Server returned error: {}",
                error_text
            );
            return Err(format!(
                "Hypergrid server error (status {}): {}",
                response.status(),
                error_text
            ));
        }

        println!("Spider: test_hypergrid_connection - Connection test successful!");
        Ok(())
    }

    async fn call_hypergrid_api(
        &self,
        url: &str,
        token: &str,
        client_id: &str,
        message: &HypergridMessage,
    ) -> Result<String, String> {
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;
        use std::collections::HashMap;

        let body = serde_json::to_string(message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Auth-Token".to_string(), token.to_string());
        headers.insert("X-Client-Id".to_string(), client_id.to_string());

        let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

        let response = send_request_await_response(
            Method::POST,
            parsed_url,
            Some(headers),
            30000,
            body.into_bytes(),
        )
        .await
        .map_err(|e| format!("API call failed: {:?}", e))?;

        if !response.status().is_success() {
            let error_text = String::from_utf8_lossy(response.body());
            return Err(format!(
                "Hypergrid API error (status {}): {}",
                response.status(),
                error_text
            ));
        }

        let response_text = String::from_utf8_lossy(response.body()).to_string();
        Ok(response_text)
    }
}
