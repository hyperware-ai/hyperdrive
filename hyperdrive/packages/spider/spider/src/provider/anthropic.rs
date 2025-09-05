use std::future::Future;
use std::pin::Pin;

use chrono::Utc;
use serde_json::Value;

use hyperware_anthropic_sdk::{
    AnthropicClient, Content, CreateMessageRequest, Message as SdkMessage, ResponseContentBlock,
    Role, Tool as SdkTool, ToolChoice,
};

use crate::provider::LlmProvider;
use crate::types::{Message, Tool, ToolCall, ToolResult};

pub(crate) struct AnthropicProvider {
    api_key: String,
    is_oauth: bool,
}

impl AnthropicProvider {
    pub(crate) fn new(api_key: String, is_oauth: bool) -> Self {
        Self { api_key, is_oauth }
    }
}

impl LlmProvider for AnthropicProvider {
    fn complete<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [Tool],
        model: Option<&'a str>,
        max_tokens: u32,
        temperature: f32,
    ) -> Pin<Box<dyn Future<Output = Result<Message, String>> + 'a>> {
        Box::pin(async move {
            // For simplicity in WASM, skip retry logic for now
            self.complete_with_retry(messages, tools, model, max_tokens, temperature)
                .await
        })
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

impl AnthropicProvider {
    // Transform MCP JSON Schema to Anthropic-compatible format
    fn transform_mcp_to_anthropic_schema(&self, mcp_schema: &Value) -> Value {
        // Start with basic structure
        let mut anthropic_schema = serde_json::json!({
            "type": "object"
        });

        if let Some(t) = mcp_schema.get("type") {
            anthropic_schema["type"] = t.clone();
        }

        // Handle $defs and resolve references if present
        let resolved_schema = if mcp_schema.get("$defs").is_some()
            || mcp_schema
                .as_object()
                .map(|o| o.keys().any(|k| k.contains("$ref")))
                .unwrap_or(false)
        {
            self.resolve_schema_refs(mcp_schema, mcp_schema.get("$defs"))
        } else {
            mcp_schema.clone()
        };

        // Extract and clean properties
        if let Some(properties) = resolved_schema.get("properties") {
            anthropic_schema["properties"] = self.clean_properties_for_anthropic(properties);
        }

        // Extract required fields
        if let Some(required) = resolved_schema.get("required") {
            anthropic_schema["required"] = required.clone();
        }

        anthropic_schema
    }

    // Resolve JSON Schema $ref references
    fn resolve_schema_refs(&self, schema: &Value, defs: Option<&Value>) -> Value {
        match schema {
            Value::Object(map) => {
                let mut resolved = serde_json::Map::new();

                for (key, value) in map {
                    if key == "$ref" {
                        // Resolve the reference
                        if let Some(ref_path) = value.as_str() {
                            if let Some(resolved_def) = self.resolve_ref_path(ref_path, defs) {
                                // Merge the resolved definition into current level
                                if let Value::Object(def_map) = resolved_def {
                                    for (def_key, def_value) in def_map {
                                        if def_key != "$ref" {
                                            resolved.insert(
                                                def_key,
                                                self.resolve_schema_refs(&def_value, defs),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    } else if key != "$defs" && key != "$schema" {
                        // Recursively resolve nested schemas, skip $defs and $schema
                        resolved.insert(key.clone(), self.resolve_schema_refs(value, defs));
                    }
                }

                Value::Object(resolved)
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|v| self.resolve_schema_refs(v, defs))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    // Helper to resolve a $ref path
    fn resolve_ref_path(&self, ref_path: &str, defs: Option<&Value>) -> Option<Value> {
        // Handle references like "#/$defs/TextOrSearchReplaceBlock"
        if ref_path.starts_with("#/$defs/") {
            if let Some(defs) = defs {
                let def_name = &ref_path[8..]; // Skip "#/$defs/"
                return defs.get(def_name).cloned();
            }
        }
        None
    }

    // Clean properties to ensure they match Anthropic's requirements
    fn clean_properties_for_anthropic(&self, properties: &Value) -> Value {
        match properties {
            Value::Object(map) => {
                let mut cleaned = serde_json::Map::new();
                for (key, value) in map {
                    // Ensure property names match Anthropic's pattern
                    if self.is_valid_anthropic_property_name(key) {
                        // Recursively clean the property value
                        cleaned.insert(key.clone(), self.clean_schema_value_for_anthropic(value));
                    }
                }
                Value::Object(cleaned)
            }
            other => other.clone(),
        }
    }

    // Clean individual schema values
    fn clean_schema_value_for_anthropic(&self, value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut cleaned = serde_json::Map::new();

                // Check if this object has a default but no type
                let has_default = map.contains_key("default");
                let has_type = map.contains_key("type");

                for (key, val) in map {
                    // Only keep standard JSON Schema properties for Anthropic
                    if matches!(
                        key.as_str(),
                        "type"
                            | "description"
                            | "properties"
                            | "required"
                            | "items"
                            | "enum"
                            | "const"
                            | "minimum"
                            | "maximum"
                            | "minLength"
                            | "maxLength"
                            | "pattern"
                            | "format"
                    ) {
                        cleaned.insert(key.clone(), self.clean_schema_value_for_anthropic(val));
                    }
                    // Special handling for default - only include if there's also a type
                    else if key == "default" && has_type {
                        cleaned.insert(key.clone(), val.clone());
                    }
                }

                // If we have a default but no type, infer the type from the default value
                if has_default && !has_type {
                    if let Some(default_val) = map.get("default") {
                        let inferred_type = match default_val {
                            Value::String(_) => "string",
                            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
                            Value::Number(_) => "number",
                            Value::Bool(_) => "boolean",
                            Value::Array(_) => "array",
                            Value::Object(_) => "object",
                            Value::Null => "null",
                        };
                        cleaned
                            .insert("type".to_string(), Value::String(inferred_type.to_string()));
                    }
                }

                Value::Object(cleaned)
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|v| self.clean_schema_value_for_anthropic(v))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    // Validate property names against Anthropic's pattern
    fn is_valid_anthropic_property_name(&self, name: &str) -> bool {
        // Pattern: ^[a-zA-Z0-9_.-]{1,64}$
        name.len() <= 64
            && name.len() >= 1
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
    }

    async fn complete_with_retry(
        &self,
        messages: &[Message],
        tools: &[Tool],
        model: Option<&str>,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<Message, String> {
        // Initialize the Anthropic SDK client
        let client = if self.is_oauth {
            // Use the with_oauth() method for OAuth tokens
            AnthropicClient::new(&self.api_key).with_oauth()
        } else {
            AnthropicClient::new(&self.api_key)
        };

        // Convert our Message format to SDK Message format
        let mut sdk_messages = Vec::new();

        for msg in messages {
            let role = match msg.role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "tool" => Role::User, // Tool results are sent as user messages in Claude API
                _ => Role::User,
            };

            // Handle different message types
            let content = if let Some(tool_results_json) = &msg.tool_results_json {
                // Parse tool results and format them for the SDK
                let tool_results: Vec<ToolResult> =
                    serde_json::from_str(tool_results_json).unwrap_or_else(|_| Vec::new());

                // Format tool results as text content
                let mut result_text = String::from("Tool execution results:\n");
                for result in tool_results {
                    result_text.push_str(&format!(
                        "- Tool call {}: {}\n",
                        result.tool_call_id, result.result
                    ));
                }
                Content::Text(result_text)
            } else if let Some(_tool_calls_json) = &msg.tool_calls_json {
                // For now, include tool calls as text in the message
                // The SDK will handle tool use blocks separately
                Content::Text(format!("{}\n[Tool calls pending]", msg.content))
            } else {
                Content::Text(msg.content.clone())
            };

            sdk_messages.push(SdkMessage { role, content });
        }

        // Convert our Tool format to SDK Tool format
        let sdk_tools: Vec<SdkTool> = tools
            .iter()
            .map(|tool| {
                // Parse the MCP schema from either inputSchema or parameters
                let mcp_schema = if let Some(ref input_schema_json) = tool.input_schema_json {
                    serde_json::from_str::<Value>(input_schema_json)
                        .unwrap_or_else(|_| serde_json::json!({}))
                } else {
                    serde_json::from_str::<Value>(&tool.parameters)
                        .unwrap_or_else(|_| serde_json::json!({}))
                };

                // Transform MCP schema to Anthropic-compatible format
                let anthropic_schema = self.transform_mcp_to_anthropic_schema(&mcp_schema);

                // Debug: Log the transformed schema
                println!(
                    "Spider: Tool {} transformed schema: {}",
                    tool.name,
                    serde_json::to_string_pretty(&anthropic_schema)
                        .unwrap_or_else(|_| "error".to_string())
                );

                // Extract required fields from the transformed schema
                let required = anthropic_schema
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_else(Vec::new);

                SdkTool::new(
                    tool.name.clone(),
                    tool.description.clone(),
                    anthropic_schema["properties"].clone(),
                    required,
                    None,
                    //anthropic_schema.get("type").and_then(|v| v.as_str()).map(|s| s.to_string()),
                )
            })
            .collect();

        // Create the request with the specified model or default
        let model_id = model.unwrap_or("claude-sonnet-4-20250514");
        let mut request = CreateMessageRequest::new(model_id, sdk_messages, max_tokens)
            .with_temperature(temperature);

        // Add system prompt for OAuth tokens
        if self.is_oauth {
            request =
                request.with_system("You are Claude Code, Anthropic's official CLI for Claude.");
        }

        println!("Tools: {sdk_tools:?}");

        // Add tools if any
        if !sdk_tools.is_empty() {
            request = request
                .with_tools(sdk_tools)
                .with_tool_choice(ToolChoice::Auto {
                    disable_parallel_tool_use: Some(false),
                });
        }

        // Send the message using the SDK
        let response = client
            .send_message(request)
            .await
            .map_err(|e| format!("Failed to send message via SDK: {:?}", e))?;

        // Convert SDK response back to our Message format
        let mut content_text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for block in &response.content {
            match block {
                ResponseContentBlock::Text { text, .. } => {
                    if !content_text.is_empty() {
                        content_text.push(' ');
                    }
                    content_text.push_str(text);
                }
                ResponseContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        tool_name: name.clone(),
                        parameters: serde_json::to_string(input)
                            .unwrap_or_else(|_| "{}".to_string()),
                    });
                }
            }
        }

        Ok(Message {
            role: "assistant".to_string(),
            content: content_text,
            tool_calls_json: if tool_calls.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&tool_calls).unwrap())
            },
            tool_results_json: None,
            timestamp: Utc::now().timestamp() as u64,
        })
    }
}
