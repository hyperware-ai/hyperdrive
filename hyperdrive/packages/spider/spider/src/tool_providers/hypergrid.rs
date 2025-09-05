use crate::tool_providers::ToolProvider;
use crate::types::{SpiderState, Tool};
use serde_json::Value;

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

    fn execute_tool(
        &self,
        _tool_name: &str,
        _parameters: &Value,
        _state: &mut SpiderState,
    ) -> Result<Value, String> {
        // This is a placeholder - the actual execution still happens in lib.rs
        // The provider is responsible for tool registration and visibility logic only
        Err("Tool execution should be handled by the main Spider implementation".to_string())
    }

    fn get_provider_id(&self) -> &str {
        &self.server_id
    }
}
