pub mod build_container;
pub mod hypergrid;
pub mod hyperware;

use crate::types::{SpiderState, Tool};
use serde_json::Value;

pub enum ToolExecutionCommand {
    // Build container commands
    InitBuildContainer {
        metadata: Option<Value>,
    },
    LoadProject {
        project_uuid: Option<String>,
        name: String, // Now required
        initial_zip: Option<String>,
        channel_id: Option<u32>,
    },
    StartPackage {
        channel_id: u32,
        package_dir: String,
    },
    Persist {
        channel_id: u32,
        directories: Vec<String>,
    },
    DoneBuildContainer {
        metadata: Option<Value>,
        channel_id: Option<u32>,
    },
    GetProjects,
    // Hypergrid commands
    HypergridAuthorize {
        server_id: String,
        url: String,
        token: String,
        client_id: String,
        node: String,
        name: Option<String>,
    },
    HypergridSearch {
        server_id: String,
        query: String,
    },
    HypergridCall {
        server_id: String,
        provider_id: String,
        provider_name: String,
        call_args: Vec<(String, String)>,
    },
    // Hyperware commands
    HyperwareSearchApis {
        query: String,
    },
    HyperwareGetApi {
        package_id: String,
    },
    HyperwareCallApi {
        process_id: String,
        signature: String,
        timeout: u64,
    },
    // Direct result (for synchronous operations)
    DirectResult(Result<Value, String>),
}

pub trait ToolProvider: Send + Sync {
    fn get_tools(&self, state: &SpiderState) -> Vec<Tool>;

    fn should_include_tool(&self, tool_name: &str, state: &SpiderState) -> bool;

    fn prepare_execution(
        &self,
        tool_name: &str,
        parameters: &Value,
        state: &SpiderState,
    ) -> Result<ToolExecutionCommand, String>;

    fn get_provider_id(&self) -> &str;
}

pub struct ToolProviderRegistry {
    providers: Vec<Box<dyn ToolProvider>>,
}

impl Default for ToolProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn ToolProvider>) {
        self.providers.push(provider);
    }

    pub fn get_available_tools(&self, state: &SpiderState) -> Vec<Tool> {
        let mut tools = Vec::new();
        for provider in &self.providers {
            let provider_tools = provider.get_tools(state);
            for tool in provider_tools {
                if provider.should_include_tool(&tool.name, state) {
                    tools.push(tool);
                }
            }
        }
        tools
    }

    pub fn find_provider_for_tool(
        &self,
        tool_name: &str,
        state: &SpiderState,
    ) -> Option<&dyn ToolProvider> {
        for provider in &self.providers {
            let tools = provider.get_tools(state);
            if tools.iter().any(|t| t.name == tool_name) {
                return Some(provider.as_ref());
            }
        }
        None
    }

    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.providers
            .iter()
            .any(|p| p.get_provider_id() == provider_id)
    }
}
