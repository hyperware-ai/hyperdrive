pub mod hypergrid;

use crate::types::{SpiderState, Tool};
use serde_json::Value;

pub trait ToolProvider: Send + Sync {
    fn get_tools(&self, state: &SpiderState) -> Vec<Tool>;

    fn should_include_tool(&self, tool_name: &str, state: &SpiderState) -> bool;

    fn execute_tool(
        &self,
        tool_name: &str,
        parameters: &Value,
        state: &mut SpiderState,
    ) -> Result<Value, String>;

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

    pub fn find_provider_for_tool(&self, tool_name: &str) -> Option<&dyn ToolProvider> {
        for provider in &self.providers {
            let tools = provider.get_tools(&SpiderState::default());
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
