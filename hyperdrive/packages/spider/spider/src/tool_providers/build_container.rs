use crate::tool_providers::ToolProvider;
use crate::types::{SpiderState, Tool};
use serde_json::Value;

pub struct BuildContainerToolProvider {
    provider_id: String,
}

impl BuildContainerToolProvider {
    pub fn new(provider_id: String) -> Self {
        Self { provider_id }
    }

    fn create_init_build_container_tool(&self) -> Tool {
        Tool {
            name: "init_build_container".to_string(),
            description: "Initialize a build container for remote compilation. Returns WebSocket URI and API key for authentication.".to_string(),
            parameters: r#"{"type":"object","required":["project_uuid"],"properties":{"project_uuid":{"type":"string","description":"UUID of the project"},"project_name":{"type":"string","description":"Optional name of the project"},"initial_zip":{"type":"string","description":"Optional base64-encoded zipped directory to extract in $HOME"},"metadata":{"type":"object","description":"Additional metadata for the build container"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["project_uuid"],"properties":{"project_uuid":{"type":"string","description":"UUID of the project"},"project_name":{"type":"string","description":"Optional name of the project"},"initial_zip":{"type":"string","description":"Optional base64-encoded zipped directory to extract in $HOME"},"metadata":{"type":"object","description":"Additional metadata for the build container"}}}"#.to_string()),
        }
    }

    fn create_start_package_tool(&self) -> Tool {
        Tool {
            name: "start_package".to_string(),
            description: "Deploy a built package to the Hyperware node. Package must be previously built with 'kit build'.".to_string(),
            parameters: r#"{"type":"object","required":["package_dir"],"properties":{"package_dir":{"type":"string","description":"Path to the package directory containing the built pkg/ folder"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["package_dir"],"properties":{"package_dir":{"type":"string","description":"Path to the package directory containing the built pkg/ folder"}}}"#.to_string()),
        }
    }

    fn create_persist_tool(&self) -> Tool {
        Tool {
            name: "persist".to_string(),
            description: "Persist directories from the build container by zipping and saving them.".to_string(),
            parameters: r#"{"type":"object","required":["directories"],"properties":{"directories":{"type":"array","items":{"type":"string"},"description":"List of directory paths to persist"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["directories"],"properties":{"directories":{"type":"array","items":{"type":"string"},"description":"List of directory paths to persist"}}}"#.to_string()),
        }
    }

    fn create_done_build_container_tool(&self) -> Tool {
        Tool {
            name: "done_build_container".to_string(),
            description: "Notify that work with the build container is complete and it can be torn down.".to_string(),
            parameters: r#"{"type":"object","required":["project_uuid"],"properties":{"project_uuid":{"type":"string","description":"UUID of the project"},"metadata":{"type":"object","description":"Additional metadata about project completion"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["project_uuid"],"properties":{"project_uuid":{"type":"string","description":"UUID of the project"},"metadata":{"type":"object","description":"Additional metadata about project completion"}}}"#.to_string()),
        }
    }
}

impl ToolProvider for BuildContainerToolProvider {
    fn get_tools(&self, state: &SpiderState) -> Vec<Tool> {
        let mut tools = vec![self.create_init_build_container_tool()];

        // Only show other tools if we have an active build container connection
        if state.build_container_connection.is_some() {
            tools.push(self.create_start_package_tool());
            tools.push(self.create_persist_tool());
            tools.push(self.create_done_build_container_tool());
        }

        tools
    }

    fn should_include_tool(&self, tool_name: &str, state: &SpiderState) -> bool {
        match tool_name {
            "init_build_container" => true,
            "start_package" | "persist" | "done_build_container" => {
                state.build_container_connection.is_some()
            }
            _ => false,
        }
    }

    fn execute_tool(
        &self,
        _tool_name: &str,
        _parameters: &Value,
        _state: &mut SpiderState,
    ) -> Result<Value, String> {
        // Execution is handled by the main Spider implementation
        Err("Tool execution should be handled by the main Spider implementation".to_string())
    }

    fn get_provider_id(&self) -> &str {
        &self.provider_id
    }
}
