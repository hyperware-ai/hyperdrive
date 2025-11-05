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
}

impl ToolProvider for HypergridToolProvider {
    fn get_tools(&self, _state: &SpiderState) -> Vec<Tool> {
        vec![
            self.create_authorize_tool(),
            self.create_search_tool(),
            self.create_call_tool(),
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
            "hypergrid_authorize" | "hypergrid_search" | "hypergrid_call" => true,
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
