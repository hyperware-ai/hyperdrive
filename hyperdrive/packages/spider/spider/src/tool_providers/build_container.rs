use crate::tool_providers::{ToolExecutionCommand, ToolProvider};
use crate::types::{
    BuildContainerReq, InitializeParams, JsonRpcReq, LoadProjectParams, McpCapabilities,
    McpClientInfo, McpRequestType, PendingMcpReq, PersistParams, SpiderAuthParams, SpiderAuthReq,
    SpiderState, StartPackageParams, Tool, ToolResponseContent, ToolResponseContentItem,
    WsConnection,
};
use hyperware_process_lib::{
    http::{
        client::{open_ws_connection, send_ws_client_push},
        server::WsMessageType,
    },
    hyperapp::sleep,
    vfs::open_file,
    LazyLoadBlob, Request,
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub struct BuildContainerToolProvider {
    provider_id: String,
}

const CONSTRUCTOR_SERVER_URL: &str = "http://localhost:8090";

impl BuildContainerToolProvider {
    pub fn new() -> Self {
        Self {
            provider_id: "build_container".to_string(),
        }
    }

    fn create_init_build_container_tool(&self) -> Tool {
        Tool {
            name: "init-build-container".to_string(),
            description: "Initialize a new build container for remote compilation and development (hosted mode only)".to_string(),
            parameters: r#"{"type":"object","properties":{"metadata":{"type":"object","description":"Optional metadata about the project (type, estimated duration, etc.)"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","properties":{"metadata":{"type":"object","description":"Optional metadata about the project (type, estimated duration, etc.)"}}}"#.to_string()),
        }
    }

    fn create_load_project_tool(&self) -> Tool {
        Tool {
            name: "load-project".to_string(),
            description: "Load a project into the build container. Creates a directory at `~/<uuid>` which should be used as the working directory for all subsequent file operations and development work. A project name is required - if the user doesn't specify one explicitly, create a descriptive name based on their input or the project context.".to_string(),
            parameters: r#"{"type":"object","required":["name"],"properties":{"project_uuid":{"type":"string","description":"Optional unique identifier for the project"},"name":{"type":"string","description":"Required project name. If user doesn't specify, create a descriptive name based on their input or project context"},"initial_zip":{"type":"string","description":"Optional VFS path to a zip file to extract in container's $HOME/<uuid> directory (e.g., /spider:dev.hypr/projects/<uuid>/backup.zip)"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["name"],"properties":{"project_uuid":{"type":"string","description":"Optional unique identifier for the project"},"name":{"type":"string","description":"Required project name. If user doesn't specify, create a descriptive name based on their input or project context"},"initial_zip":{"type":"string","description":"Optional VFS path to a zip file to extract in container's $HOME/<uuid> directory (e.g., /spider:dev.hypr/projects/<uuid>/backup.zip)"}}}"#.to_string()),
        }
    }

    fn create_start_package_tool(&self) -> Tool {
        Tool {
            name: "start-package".to_string(),
            description: "Deploy a built package from the build container to the Hyperware node. A package is distinguishable by a pkg/ directory inside of it. Do not use this tool on the pkg/ directory, but the directory that contains the pkg/".to_string(),
            parameters: r#"{"type":"object","required":["package_dir"],"properties":{"package_dir":{"type":"string","description":"Path to the package directory that was built with 'kit build'"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["package_dir"],"properties":{"package_dir":{"type":"string","description":"Path to the package directory that was built with 'kit build'"}}}"#.to_string()),
        }
    }

    fn create_persist_tool(&self) -> Tool {
        Tool {
            name: "persist".to_string(),
            description: "Persist directories from the build container by creating a zip file".to_string(),
            parameters: r#"{"type":"object","required":["directories"],"properties":{"directories":{"type":"array","items":{"type":"string"},"description":"List of directory paths to persist"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","required":["directories"],"properties":{"directories":{"type":"array","items":{"type":"string"},"description":"List of directory paths to persist"}}}"#.to_string()),
        }
    }

    fn create_done_build_container_tool(&self) -> Tool {
        Tool {
            name: "done-build-container".to_string(),
            description: "Notify that work with the build container is complete and it can be torn down (hosted mode only)".to_string(),
            parameters: r#"{"type":"object","properties":{"metadata":{"type":"object","description":"Optional metadata about completion status"}}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","properties":{"metadata":{"type":"object","description":"Optional metadata about completion status"}}}"#.to_string()),
        }
    }

    fn create_get_projects_tool(&self) -> Tool {
        Tool {
            name: "get-projects".to_string(),
            description: "Get a mapping of project names to their associated UUIDs".to_string(),
            parameters: r#"{"type":"object","properties":{}}"#.to_string(),
            input_schema_json: Some(r#"{"type":"object","properties":{}}"#.to_string()),
        }
    }
}

impl ToolProvider for BuildContainerToolProvider {
    fn get_tools(&self, state: &SpiderState) -> Vec<Tool> {
        let mut tools = Vec::new();

        // Always provide get-projects tool
        tools.push(self.create_get_projects_tool());

        // Check if we're in self-hosted mode
        let is_self_hosted =
            !state.build_container_ws_uri.is_empty() && !state.build_container_api_key.is_empty();

        if !is_self_hosted {
            // Hosted mode: show init_build_container
            tools.push(self.create_init_build_container_tool());
        }

        // Check if we have an active build container connection
        let has_connection = state.ws_connections.values().any(|conn| {
            conn.server_id.starts_with("build_container_")
                || conn.server_id == "build_container_self_hosted"
        });

        // Always show load_project in self-hosted mode, or if we have a connection in hosted mode
        if is_self_hosted {
            tools.push(self.create_load_project_tool());
        } else if has_connection {
            tools.push(self.create_load_project_tool());
        }

        // Show other tools if we have an active build container connection
        if has_connection {
            tools.push(self.create_start_package_tool());
            tools.push(self.create_persist_tool());

            if !is_self_hosted {
                // Only show done_build_container in hosted mode
                tools.push(self.create_done_build_container_tool());
            }
        }

        tools
    }

    fn should_include_tool(&self, tool_name: &str, state: &SpiderState) -> bool {
        let is_self_hosted =
            !state.build_container_ws_uri.is_empty() && !state.build_container_api_key.is_empty();
        let has_connection = state
            .ws_connections
            .values()
            .any(|conn| conn.server_id.starts_with("build_container_"));

        match tool_name {
            "get-projects" => true, // Always available
            "init-build-container" => !is_self_hosted,
            "load-project" => is_self_hosted || has_connection,
            "start-package" | "persist" => has_connection,
            "done-build-container" => !is_self_hosted && has_connection,
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
            "get-projects" => Ok(ToolExecutionCommand::GetProjects),
            "init-build-container" => {
                let metadata = parameters.get("metadata").cloned();

                Ok(ToolExecutionCommand::InitBuildContainer { metadata })
            }
            "load-project" => {
                let project_uuid = parameters
                    .get("project_uuid")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                // Name is now required
                let name = parameters
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| "Project name is required. Please provide a descriptive name for the project.".to_string())?;

                let initial_zip = parameters
                    .get("initial_zip")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                // Check if we need to establish connection for self-hosted mode
                let is_self_hosted = !state.build_container_ws_uri.is_empty()
                    && !state.build_container_api_key.is_empty();
                let channel_id = if is_self_hosted
                    && !state.ws_connections.values().any(|conn| {
                        conn.server_id.starts_with("build_container_")
                            || conn.server_id == "build_container_self_hosted"
                    }) {
                    // Need to establish connection first (this will be handled in execute)
                    None
                } else {
                    // Find existing build container connection
                    state
                        .ws_connections
                        .iter()
                        .find(|(_, conn)| {
                            conn.server_id.starts_with("build_container_")
                                || conn.server_id == "build_container_self_hosted"
                        })
                        .map(|(id, _)| *id)
                };

                Ok(ToolExecutionCommand::LoadProject {
                    project_uuid,
                    name,
                    initial_zip,
                    channel_id,
                })
            }
            "start-package" => {
                let package_dir = parameters
                    .get("package_dir")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing package_dir parameter".to_string())?
                    .to_string();

                // Find the build container connection
                let channel_id = state
                    .ws_connections
                    .iter()
                    .find(|(_, conn)| conn.server_id.starts_with("build_container_"))
                    .map(|(id, _)| *id)
                    .ok_or_else(|| {
                        "No build container connection found. Call init-build-container first."
                            .to_string()
                    })?;

                Ok(ToolExecutionCommand::StartPackage {
                    channel_id,
                    package_dir,
                })
            }
            "persist" => {
                let directories = parameters
                    .get("directories")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| "Missing directories parameter".to_string())?;

                let dir_strings: Vec<String> = directories
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();

                if dir_strings.is_empty() {
                    return Err("No valid directories provided".to_string());
                }

                // Find the build container connection
                let channel_id = state
                    .ws_connections
                    .iter()
                    .find(|(_, conn)| conn.server_id.starts_with("build_container_"))
                    .map(|(id, _)| *id)
                    .ok_or_else(|| {
                        "No build container connection found. Call init-build-container first."
                            .to_string()
                    })?;

                Ok(ToolExecutionCommand::Persist {
                    channel_id,
                    directories: dir_strings,
                })
            }
            "done-build-container" => {
                let metadata = parameters.get("metadata").cloned();

                // Find any active build container connection
                let channel_id = state
                    .ws_connections
                    .iter()
                    .find(|(_, conn)| conn.server_id.starts_with("build_container_"))
                    .map(|(id, _)| *id);

                Ok(ToolExecutionCommand::DoneBuildContainer {
                    metadata,
                    channel_id,
                })
            }
            _ => Err(format!("Unknown build container tool: {}", tool_name)),
        }
    }

    fn get_provider_id(&self) -> &str {
        &self.provider_id
    }
}

// Extension trait for build container operations
pub trait BuildContainerExt {
    async fn execute_init_build_container_impl(
        &mut self,
        metadata: Option<Value>,
    ) -> Result<Value, String>;
    async fn execute_load_project_impl(
        &mut self,
        project_uuid: Option<String>,
        name: String, // Now required
        initial_zip: Option<String>,
        channel_id: Option<u32>,
    ) -> Result<Value, String>;
    async fn execute_start_package_impl(
        &mut self,
        channel_id: u32,
        package_dir: String,
    ) -> Result<Value, String>;
    async fn execute_persist_impl(
        &mut self,
        channel_id: u32,
        directories: Vec<String>,
    ) -> Result<Value, String>;
    async fn execute_done_build_container_impl(
        &mut self,
        metadata: Option<Value>,
        channel_id: Option<u32>,
    ) -> Result<Value, String>;
    async fn connect_to_self_hosted_container(&mut self) -> Result<u32, String>;
    fn request_build_container_tools_list(&mut self, channel_id: u32);
    fn send_tools_list_request(&mut self, channel_id: u32);
    async fn deploy_package_to_app_store(
        &self,
        package_name: &str,
        publisher: &str,
        version_hash: &str,
        package_zip: &str,
        metadata: Value,
    ) -> Result<(), String>;
}

impl BuildContainerExt for SpiderState {
    async fn execute_init_build_container_impl(
        &mut self,
        metadata: Option<Value>,
    ) -> Result<Value, String> {
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;

        // Use hardcoded constructor URL
        let constructor_url = format!("{CONSTRUCTOR_SERVER_URL}/init-build-container");

        // Prepare request body
        let body = BuildContainerReq {
            metadata: metadata.clone(),
        };

        // Make HTTP request to constructor
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let url = url::Url::parse(&constructor_url)
            .map_err(|e| format!("Invalid constructor URL: {}", e))?;

        let response = send_request_await_response(
            Method::POST,
            url,
            Some(headers),
            30000,
            serde_json::to_string(&body)
                .map_err(|e| format!("Failed to serialize request: {}", e))?
                .into_bytes(),
        )
        .await
        .map_err(|e| format!("Failed to initialize build container: {:?}", e))?;

        if !response.status().is_success() {
            let error_text = String::from_utf8_lossy(response.body());
            return Err(format!(
                "Constructor error (status {}): {}",
                response.status(),
                error_text
            ));
        }

        // Parse response
        let response_data: Value = serde_json::from_slice(response.body())
            .map_err(|e| format!("Failed to parse constructor response: {}", e))?;

        let ws_uri = response_data
            .get("ws_uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing ws_uri in response".to_string())?;

        let api_key = response_data
            .get("api_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing api_key in response".to_string())?;

        // Generate a unique project UUID since we don't require it anymore
        let project_uuid = Uuid::new_v4().to_string();

        // Connect to the build container's ws-mcp server
        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;

        // Open WebSocket connection
        open_ws_connection(ws_uri.to_string(), None, channel_id)
            .await
            .map_err(|e| format!("Failed to open WS connection to {ws_uri}: {e}"))?;

        // Store connection info for the build container
        let server_id = format!("build_container_{}", project_uuid);
        self.ws_connections.insert(
            channel_id,
            WsConnection {
                server_id: server_id.clone(),
                server_name: format!("Build Container {}", project_uuid),
                channel_id,
                tools: Vec::new(),
                initialized: false,
            },
        );

        // Send authentication message
        let auth_request = SpiderAuthReq {
            jsonrpc: "2.0".to_string(),
            method: "spider/authorization".to_string(),
            params: SpiderAuthParams {
                api_key: api_key.to_string(),
            },
            id: format!("auth_{}", channel_id),
        };

        let blob = LazyLoadBlob::new(
            None::<String>,
            serde_json::to_string(&auth_request)
                .map_err(|e| format!("Failed to serialize auth request: {}", e))?
                .into_bytes(),
        );
        send_ws_client_push(channel_id, WsMessageType::Text, blob);

        // Update build container tools to show additional tools now that we're connected
        if let Some(provider) = self
            .tool_provider_registry
            .find_provider_for_tool("init-build-container", self)
        {
            let updated_tools = provider.get_tools(self);
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == "build_container")
            {
                server.tools = updated_tools;
            }
        }

        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "text".to_string(),
                text: format!(
                    "✅ Build container initialized successfully!\n- WebSocket: {}\n- Ready for remote compilation",
                    ws_uri
                ),
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn execute_load_project_impl(
        &mut self,
        project_uuid: Option<String>,
        name: String, // Now required
        initial_zip: Option<String>,
        mut channel_id: Option<u32>,
    ) -> Result<Value, String> {
        // Check if we need to connect to self-hosted container first
        let is_self_hosted =
            !self.build_container_ws_uri.is_empty() && !self.build_container_api_key.is_empty();

        if is_self_hosted && channel_id.is_none() {
            // Connect to self-hosted container
            channel_id = Some(self.connect_to_self_hosted_container().await?);
        }

        let channel_id =
            channel_id.ok_or_else(|| "No build container connection available".to_string())?;

        // Generate project UUID if not provided
        let project_uuid = project_uuid.unwrap_or_else(|| Uuid::new_v4().to_string());

        // Handle initial_zip - must be a VFS path if provided
        let initial_zip_content = if let Some(zip_path) = &initial_zip {
            // Validate it's a proper VFS path
            if !zip_path.starts_with('/') {
                return Err(format!(
                    "Invalid VFS path '{}'. VFS paths must start with '/' (e.g., /spider:dev.hypr/projects/<uuid>/backup.zip). \
                    To load a persisted project, first use 'get-projects' to find available projects, \
                    then provide the full VFS path to the backup zip file.",
                    zip_path
                ));
            }

            // Load the zip file from VFS
            match open_file(zip_path, false, None) {
                Ok(file) => {
                    match file.read() {
                        Ok(data) => {
                            if data.is_empty() {
                                return Err(format!(
                                    "The zip file at '{}' exists but is empty. \
                                    Please ensure the project was properly persisted with the 'persist' tool.",
                                    zip_path
                                ));
                            }
                            // Encode to base64 for transmission
                            use base64::{engine::general_purpose, Engine as _};
                            Some(general_purpose::STANDARD.encode(&data))
                        }
                        Err(e) => {
                            return Err(format!(
                                "Failed to read zip file at '{}': {:?}. \
                                Please verify the file exists and you have read permissions. \
                                Use 'get-projects' to see available projects.",
                                zip_path, e
                            ));
                        }
                    }
                }
                Err(e) => {
                    // Provide helpful suggestions based on the error
                    let suggestion = if zip_path.contains("/projects/") {
                        "Use 'get-projects' to list available projects and their UUIDs, \
                        then check the VFS for the correct backup.zip path."
                    } else {
                        "Make sure the path follows the format: /spider:dev.hypr/projects/<uuid>/backup.zip"
                    };

                    return Err(format!(
                        "Cannot open zip file at '{}': {:?}. \
                        {}. The file may not exist or the path may be incorrect.",
                        zip_path, e, suggestion
                    ));
                }
            }
        } else {
            None
        };

        // Update project name to UUID mapping (name is now always present)
        self.project_name_to_uuids
            .entry(name.clone())
            .or_insert_with(Vec::new)
            .push(project_uuid.clone());
        println!(
            "Spider: Added project '{}' with UUID {}",
            name, project_uuid
        );

        // Send spider/load-project request over WebSocket
        let request_id = format!("load-project-{}", Uuid::new_v4());
        println!(
            "Spider: Sending load-project request with id: {}",
            request_id
        );
        let request = JsonRpcReq {
            jsonrpc: "2.0".to_string(),
            method: "spider/load-project".to_string(),
            params: Some(
                serde_json::to_value(LoadProjectParams {
                    project_uuid: project_uuid.clone(),
                    name: Some(name.clone()),
                    initial_zip: initial_zip_content,
                })
                .map_err(|e| format!("Failed to serialize params: {}", e))?,
            ),
            id: request_id.clone(),
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        println!(
            "Spider: Sending request to channel {}: {}",
            channel_id, request_json
        );
        let blob = LazyLoadBlob::new(None::<String>, request_json.into_bytes());
        send_ws_client_push(channel_id, WsMessageType::Text, blob);

        // Wait for response (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(30);

        println!("Spider: Waiting for response with id: {}", request_id);
        loop {
            if start.elapsed() > timeout {
                println!(
                    "Spider: Timeout waiting for response, tool_responses keys: {:?}",
                    self.tool_responses.keys().collect::<Vec<_>>()
                );
                return Err("Timeout waiting for load-project response".to_string());
            }

            if let Some(response) = self.tool_responses.remove(&request_id) {
                println!(
                    "Spider: Found response for id {}: {:?}",
                    request_id, response
                );
                // Check if response contains an error
                if let Some(error) = response.get("error") {
                    return Err(format!("Failed to load project: {}", error));
                }

                // Extract project_uuid from response
                let returned_uuid = response
                    .get("project_uuid")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&project_uuid);

                // After successful load-project, ws-mcp may have new tools available
                // Send tools/list and wait for the response to ensure tools are updated
                println!("Spider: Requesting updated tools list after successful load-project");
                let tools_request_id = format!("tools_refresh_{}", channel_id);
                let tools_request = JsonRpcReq {
                    jsonrpc: "2.0".to_string(),
                    method: "tools/list".to_string(),
                    params: None,
                    id: tools_request_id.clone(),
                };

                // Store pending request
                if let Some(conn) = self.ws_connections.get(&channel_id) {
                    self.pending_mcp_requests.insert(
                        tools_request_id.clone(),
                        PendingMcpReq {
                            request_id: tools_request_id.clone(),
                            conversation_id: None,
                            server_id: conn.server_id.clone(),
                            request_type: McpRequestType::ToolsList,
                        },
                    );
                }

                println!(
                    "Spider: Sending tools/list request with id: {}",
                    tools_request_id
                );
                let blob = LazyLoadBlob::new(
                    None::<String>,
                    serde_json::to_string(&tools_request).unwrap().into_bytes(),
                );
                send_ws_client_push(channel_id, WsMessageType::Text, blob);

                // Wait for the tools/list response with a short timeout
                let tools_start = std::time::Instant::now();
                let tools_timeout = std::time::Duration::from_secs(5);

                println!("Spider: Waiting for tools/list response after load-project");
                loop {
                    if tools_start.elapsed() > tools_timeout {
                        println!(
                            "Spider: Timeout waiting for tools/list response, continuing anyway"
                        );
                        break; // Don't fail, just continue without updated tools
                    }

                    // Check if the tools have been updated (handle_tools_list_response will update them)
                    // We just need to wait a bit for the response to be processed
                    if !self.pending_mcp_requests.contains_key(&tools_request_id) {
                        println!("Spider: Tools list updated successfully");
                        break;
                    }

                    // Sleep briefly before checking again
                    let _ = sleep(100).await;
                }

                return Ok(serde_json::to_value(ToolResponseContent {
                    content: vec![ToolResponseContentItem {
                        content_type: "text".to_string(),
                        text: format!(
                            "✅ Project loaded successfully!\n- UUID: {}\n- Directory created in container",
                            returned_uuid
                        ),
                    }],
                })
                .map_err(|e| format!("Failed to serialize response: {}", e))?);
            }

            // Sleep briefly before checking again
            sleep(100).await;
        }
    }

    async fn execute_start_package_impl(
        &mut self,
        channel_id: u32,
        package_dir: String,
    ) -> Result<Value, String> {
        // Send spider/start-package request over WebSocket
        let request_id = format!("start-package-{}", Uuid::new_v4());
        let request = JsonRpcReq {
            jsonrpc: "2.0".to_string(),
            method: "spider/start-package".to_string(),
            params: Some(
                serde_json::to_value(StartPackageParams {
                    package_dir: package_dir.clone(),
                })
                .map_err(|e| format!("Failed to serialize params: {}", e))?,
            ),
            id: request_id.clone(),
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        let blob = LazyLoadBlob::new(None::<String>, request_json.into_bytes());
        send_ws_client_push(channel_id, WsMessageType::Text, blob);

        // Wait for response (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(30);

        loop {
            if start.elapsed() > timeout {
                return Err("Timeout waiting for start-package response".to_string());
            }

            if let Some(response) = self.tool_responses.remove(&request_id) {
                // Check if response contains an error
                if let Some(error) = response.get("error") {
                    return Err(format!("Failed to start package: {}", error));
                }

                // Extract package_zip from response
                let Some(package_zip) = response.get("package_zip").and_then(|v| v.as_str()) else {
                    return Err("No package_zip in response".to_string());
                };

                // Extract metadata fields from response
                let package_name = response
                    .get("package_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "No package_name in response".to_string())?;

                let our_node = hyperware_process_lib::our().node.clone();
                let publisher = response
                    .get("publisher")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&our_node);

                let version_hash = response
                    .get("version_hash")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "No version_hash in response".to_string())?;

                // Get the full metadata object from response
                let metadata = response
                    .get("metadata")
                    .ok_or_else(|| "No metadata in response".to_string())?;

                // Deploy the package to the Hyperware node using app-store
                match self
                    .deploy_package_to_app_store(
                        package_name,
                        publisher,
                        version_hash,
                        package_zip,
                        metadata.clone(),
                    )
                    .await
                {
                    Ok(_) => {
                        return Ok(serde_json::to_value(ToolResponseContent {
                            content: vec![ToolResponseContentItem {
                                content_type: "text".to_string(),
                                text: format!(
                                    "✅ Package '{}' from {} deployed and installed successfully!\n- Publisher: {}\n- Version hash: {}",
                                    package_name,
                                    package_dir,
                                    publisher,
                                    &version_hash[..8]  // Show first 8 chars of hash
                                ),
                            }],
                        })
                        .map_err(|e| format!("Failed to serialize response: {}", e))?);
                    }
                    Err(e) => {
                        return Err(format!("Failed to deploy package: {}", e));
                    }
                }
            }

            // Sleep briefly before checking again
            sleep(100).await;
        }
    }

    async fn execute_persist_impl(
        &mut self,
        channel_id: u32,
        directories: Vec<String>,
    ) -> Result<Value, String> {
        use hyperware_process_lib::{
            our,
            vfs::{create_drive, open_dir, open_file},
        };

        // Send spider/persist request over WebSocket
        let request_id = format!("persist_{}", Uuid::new_v4());
        let request = JsonRpcReq {
            jsonrpc: "2.0".to_string(),
            method: "spider/persist".to_string(),
            params: Some(
                serde_json::to_value(PersistParams {
                    directories: directories.clone(),
                })
                .map_err(|e| format!("Failed to serialize params: {}", e))?,
            ),
            id: request_id.clone(),
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        let blob = LazyLoadBlob::new(None::<String>, request_json.into_bytes());
        send_ws_client_push(channel_id, WsMessageType::Text, blob);

        // Wait for response (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(30);

        loop {
            if start.elapsed() > timeout {
                return Err("Timeout waiting for persist response".to_string());
            }

            if let Some(response) = self.tool_responses.remove(&request_id) {
                // Check if response contains persisted_zip
                if let Some(persisted_zip) = response.get("persisted_zip").and_then(|v| v.as_str())
                {
                    // Get project_uuid from response or generate one
                    let project_uuid = response
                        .get("project_uuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| {
                            // If no project_uuid in response, try to extract from the first directory path
                            // Assuming directories are like /home/user/<uuid>/...
                            directories
                                .first()
                                .and_then(|dir| {
                                    let parts: Vec<&str> = dir.split('/').collect();
                                    // Look for a UUID-like string in the path
                                    parts
                                        .iter()
                                        .find(|part| part.len() == 36 && part.contains('-'))
                                        .copied()
                                })
                                .unwrap_or("unknown")
                        })
                        .to_string();

                    // Create projects drive if it doesn't exist
                    let projects_drive = match create_drive(our().package_id(), "projects", None) {
                        Ok(drive_path) => drive_path,
                        Err(e) => {
                            println!("Warning: Failed to create projects drive: {:?}", e);
                            // Still return success but without saving to VFS
                            return Ok(serde_json::to_value(ToolResponseContent {
                                content: vec![ToolResponseContentItem {
                                    content_type: "text".to_string(),
                                    text: format!(
                                        "✅ Persisted {} directories successfully! (Note: Could not save backup to VFS)",
                                        directories.len()
                                    ),
                                }],
                            })
                            .map_err(|e| format!("Failed to serialize response: {}", e))?);
                        }
                    };

                    // Create project-specific directory path
                    let project_dir = format!("{}/{}", projects_drive, project_uuid);

                    // Create the project directory
                    match open_dir(&project_dir, true, None) {
                        Ok(_) => {
                            println!("Spider: Created/opened project directory: {}", project_dir);
                        }
                        Err(e) => {
                            println!(
                                "Warning: Failed to create project directory {}: {:?}",
                                project_dir, e
                            );
                            // Still try to continue - maybe we can write files directly
                        }
                    }

                    // Generate timestamp for the zip file
                    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
                    let zip_filename = format!("{}/backup-{}.zip", project_dir, timestamp);
                    let manifest_filename = format!("{}/manifest-{}.json", project_dir, timestamp);

                    // Decode the base64 zip data
                    use base64::{engine::general_purpose, Engine as _};
                    let zip_bytes = general_purpose::STANDARD
                        .decode(persisted_zip)
                        .map_err(|e| format!("Failed to decode base64 zip: {}", e))?;

                    // Save the zip file
                    match open_file(&zip_filename, true, None) {
                        Ok(file) => {
                            file.write(&zip_bytes)
                                .map_err(|e| format!("Failed to write zip file: {:?}", e))?;
                            println!("Saved project backup to: {}", zip_filename);
                        }
                        Err(e) => {
                            println!("Warning: Failed to save zip file: {:?}", e);
                        }
                    }

                    // Create and save manifest
                    let manifest = serde_json::json!({
                        "project_uuid": project_uuid,
                        "timestamp": timestamp,
                        "directories": directories,
                        "zip_file": zip_filename,
                        "size_bytes": zip_bytes.len(),
                    });

                    let manifest_json = serde_json::to_string_pretty(&manifest)
                        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;

                    match open_file(&manifest_filename, true, None) {
                        Ok(file) => {
                            file.write(manifest_json.as_bytes())
                                .map_err(|e| format!("Failed to write manifest: {:?}", e))?;
                            println!("Saved manifest to: {}", manifest_filename);
                        }
                        Err(e) => {
                            println!("Warning: Failed to save manifest: {:?}", e);
                        }
                    }

                    return Ok(serde_json::to_value(ToolResponseContent {
                        content: vec![ToolResponseContentItem {
                            content_type: "text".to_string(),
                            text: format!(
                                "✅ Persisted {} directories successfully!\n📁 Project: {}\n💾 Backup: {}\n📝 Manifest: {}",
                                directories.len(),
                                project_uuid,
                                zip_filename,
                                manifest_filename
                            ),
                        }],
                    })
                    .map_err(|e| format!("Failed to serialize response: {}", e))?);
                } else if let Some(error) = response.get("error") {
                    return Err(format!("Failed to persist directories: {}", error));
                } else {
                    return Err("Invalid response from persist operation".to_string());
                }
            }

            // Sleep briefly before checking again
            let _ = sleep(100).await;
        }
    }

    async fn execute_done_build_container_impl(
        &mut self,
        metadata: Option<Value>,
        channel_id: Option<u32>,
    ) -> Result<Value, String> {
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;

        if let Some(channel_id) = channel_id {
            // Get server_id before removing the connection
            let server_id = self
                .ws_connections
                .get(&channel_id)
                .map(|conn| conn.server_id.clone());

            // Send close message
            send_ws_client_push(channel_id, WsMessageType::Close, LazyLoadBlob::default());

            // Remove the connection
            self.ws_connections.remove(&channel_id);

            // Clean up any pending requests for this connection
            if let Some(sid) = server_id {
                self.pending_mcp_requests
                    .retain(|_, req| req.server_id != sid);
            }
        }

        // Use hardcoded constructor URL
        let constructor_url = format!("{CONSTRUCTOR_SERVER_URL}/done-build-container");

        // Prepare request body
        let body = BuildContainerReq {
            metadata: metadata.clone(),
        };

        // Make HTTP request to constructor
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let url = url::Url::parse(&constructor_url)
            .map_err(|e| format!("Invalid constructor URL: {}", e))?;

        let response = send_request_await_response(
            Method::POST,
            url,
            Some(headers),
            30000,
            serde_json::to_string(&body)
                .map_err(|e| format!("Failed to serialize request: {}", e))?
                .into_bytes(),
        )
        .await
        .map_err(|e| format!("Failed to tear down build container: {:?}", e))?;

        if !response.status().is_success() {
            let error_text = String::from_utf8_lossy(response.body());
            return Err(format!(
                "Constructor error (status {}): {}",
                response.status(),
                error_text
            ));
        }

        // Update build container tools to hide additional tools now that we're disconnected
        if let Some(provider) = self
            .tool_provider_registry
            .find_provider_for_tool("init-build-container", self)
        {
            let updated_tools = provider.get_tools(self);
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == "build_container")
            {
                server.tools = updated_tools;
            }
        }

        Ok(serde_json::to_value(ToolResponseContent {
            content: vec![ToolResponseContentItem {
                content_type: "text".to_string(),
                text: "✅ Build container has been torn down successfully!".to_string(),
            }],
        })
        .map_err(|e| format!("Failed to serialize response: {}", e))?)
    }

    async fn connect_to_self_hosted_container(&mut self) -> Result<u32, String> {
        // Check if we already have a connection to self-hosted container
        if let Some((channel_id, _)) = self
            .ws_connections
            .iter()
            .find(|(_, conn)| conn.server_id == "build_container_self_hosted")
        {
            println!(
                "Spider: Reusing existing self-hosted build container connection on channel {}",
                channel_id
            );
            return Ok(*channel_id);
        }

        // Connect to self-hosted container using configured WS URI and API key
        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;

        println!(
            "Spider: Opening new WebSocket connection to self-hosted build container on channel {}",
            channel_id
        );

        // Open WebSocket connection
        open_ws_connection(self.build_container_ws_uri.clone(), None, channel_id)
            .await
            .map_err(|e| {
                format!(
                    "Failed to open WS connection to {}: {e}",
                    self.build_container_ws_uri
                )
            })?;

        // Store connection info for the build container
        let server_id = "build_container_self_hosted".to_string();
        self.ws_connections.insert(
            channel_id,
            WsConnection {
                server_id: server_id.clone(),
                server_name: "Self-Hosted Build Container".to_string(),
                channel_id,
                tools: Vec::new(),
                initialized: false,
            },
        );

        // Send authentication message
        let auth_id = format!("auth_{}", channel_id);
        let auth_request = SpiderAuthReq {
            jsonrpc: "2.0".to_string(),
            method: "spider/authorization".to_string(),
            params: SpiderAuthParams {
                api_key: self.build_container_api_key.clone(),
            },
            id: auth_id.clone(),
        };

        println!("Spider: Sending authorization request with id: {}", auth_id);
        let blob = LazyLoadBlob::new(
            None::<String>,
            serde_json::to_string(&auth_request)
                .map_err(|e| format!("Failed to serialize auth request: {}", e))?
                .into_bytes(),
        );
        send_ws_client_push(channel_id, WsMessageType::Text, blob);

        // Wait for authentication response
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(10);

        println!("Spider: Waiting for authorization response...");
        loop {
            if start.elapsed() > timeout {
                println!("Spider: Authorization timeout after 10 seconds");
                return Err("Timeout waiting for authorization response".to_string());
            }

            if let Some(response) = self.tool_responses.remove(&auth_id) {
                println!("Spider: Got authorization response: {:?}", response);
                if response.get("status").and_then(|s| s.as_str()) == Some("authenticated") {
                    println!("Spider: Successfully authenticated with self-hosted build container");
                    break;
                } else if let Some(error) = response.get("error") {
                    return Err(format!("Authorization failed: {}", error));
                } else {
                    return Err("Invalid authorization response".to_string());
                }
            }

            // Sleep briefly before checking again
            let _ = sleep(100).await;
        }

        // Now send initialize request after successful authentication
        println!("Spider: Sending initialize request after successful authentication");
        self.request_build_container_tools_list(channel_id);

        // Update build container tools to show additional tools now that we're connected
        if let Some(provider) = self
            .tool_provider_registry
            .find_provider_for_tool("load-project", self)
        {
            let updated_tools = provider.get_tools(self);
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == "build_container")
            {
                server.tools = updated_tools;
            }
        }

        Ok(channel_id)
    }

    fn request_build_container_tools_list(&mut self, channel_id: u32) {
        use crate::types::{McpRequestType, PendingMcpReq};

        // First send initialize request
        let init_request_id = format!("init_build_container_{}", channel_id);
        let init_request = JsonRpcReq {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(
                serde_json::to_value(InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                    client_info: McpClientInfo {
                        name: "spider".to_string(),
                        version: "1.0.0".to_string(),
                    },
                    capabilities: McpCapabilities {},
                })
                .unwrap_or_else(|_| Value::Null),
            ),
            id: init_request_id.clone(),
        };

        // Store pending request for initialize
        if let Some(conn) = self.ws_connections.get(&channel_id) {
            self.pending_mcp_requests.insert(
                init_request_id.clone(),
                PendingMcpReq {
                    request_id: init_request_id.clone(),
                    conversation_id: None,
                    server_id: conn.server_id.clone(),
                    request_type: McpRequestType::Initialize,
                },
            );
        }

        println!(
            "Spider: Sending initialize request with id: {}",
            init_request_id
        );
        let init_blob = LazyLoadBlob::new(
            None::<String>,
            serde_json::to_string(&init_request).unwrap().into_bytes(),
        );
        send_ws_client_push(channel_id, WsMessageType::Text, init_blob);

        // Note: The actual tools/list request will be sent when we receive the initialize response
        // This is handled in handle_initialize_response in lib.rs which calls request_tools_list
    }

    fn send_tools_list_request(&mut self, channel_id: u32) {
        use crate::types::{McpRequestType, PendingMcpReq};

        let request_id = format!("tools_refresh_{}", channel_id);
        let tools_request = JsonRpcReq {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: request_id.clone(),
        };

        // Store pending request
        if let Some(conn) = self.ws_connections.get(&channel_id) {
            self.pending_mcp_requests.insert(
                request_id.clone(),
                PendingMcpReq {
                    request_id: request_id.clone(),
                    conversation_id: None,
                    server_id: conn.server_id.clone(),
                    request_type: McpRequestType::ToolsList,
                },
            );
        }

        println!("Spider: Sending tools/list request with id: {}", request_id);
        let blob = LazyLoadBlob::new(
            None::<String>,
            serde_json::to_string(&tools_request).unwrap().into_bytes(),
        );
        send_ws_client_push(channel_id, WsMessageType::Text, blob);
    }

    async fn deploy_package_to_app_store(
        &self,
        package_name: &str,
        publisher: &str,
        version_hash: &str,
        package_zip: &str,
        metadata: Value,
    ) -> Result<(), String> {
        use base64::Engine;

        println!("Spider: Deploying package {} to app-store", package_name);

        // Decode the base64 package zip
        let package_bytes = base64::engine::general_purpose::STANDARD
            .decode(package_zip)
            .map_err(|e| format!("Failed to decode package zip: {}", e))?;

        // Create NewPackage request
        let new_package_request = serde_json::json!({
            "NewPackage": {
                "package_id": {
                    "package_name": package_name,
                    "publisher_node": publisher,
                },
                "mirror": true
            }
        });

        // Send NewPackage request to app-store with the zip as blob
        let blob = LazyLoadBlob::new(None::<String>, package_bytes);
        let request = Request::to(("our", "main", "app-store", "sys"))
            .body(serde_json::to_vec(&new_package_request).map_err(|e| e.to_string())?)
            .blob(blob)
            .expects_response(15);

        let response = request
            .send_and_await_response(15)
            .map_err(|e| format!("Failed to send new-package request: {:?}", e))?
            .map_err(|e| format!("New-package request failed: {:?}", e))?;

        // Parse response
        let response_body = String::from_utf8(response.body().to_vec())
            .map_err(|e| format!("Failed to parse response body: {}", e))?;
        let response_json: Value = serde_json::from_str(&response_body)
            .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

        // Check if NewPackage was successful
        if let Some(new_package_response) = response_json.get("NewPackageResponse") {
            if new_package_response != &serde_json::Value::String("Success".to_string()) {
                return Err(format!("Failed to add package: {:?}", new_package_response));
            }
        } else {
            return Err(format!(
                "Unexpected response from app-store: {:?}",
                response_json
            ));
        }

        println!("Spider: Package added successfully, now installing...");

        // Parse metadata to create OnchainMetadata
        let onchain_metadata = serde_json::json!({
            "name": metadata.get("name").and_then(|v| v.as_str()).unwrap_or(package_name),
            "description": metadata.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "image": metadata.get("image").and_then(|v| v.as_str()).unwrap_or(""),
            "external_url": metadata.get("external_url").and_then(|v| v.as_str()).unwrap_or(""),
            "animation_url": metadata.get("animation_url").and_then(|v| v.as_str()),
            "properties": {
                "package_name": package_name,
                "publisher": publisher,
                "current_version": metadata.get("current_version").and_then(|v| v.as_str()).unwrap_or("1.0.0"),
                "mirrors": metadata.get("mirrors").and_then(|v| v.as_array()).unwrap_or(&vec![]).clone(),
                "code_hashes": metadata.get("code_hashes").and_then(|v| v.as_array()).unwrap_or(&vec![]).clone(),
                "license": metadata.get("license").and_then(|v| v.as_str()),
                "screenshots": metadata.get("screenshots").and_then(|v| v.as_array()).map(|v| v.clone()),
                "wit_version": metadata.get("wit_version").and_then(|v| v.as_u64()).map(|v| v as u32),
                "dependencies": metadata.get("dependencies").and_then(|v| v.as_array()).map(|v| v.clone()),
                "api_includes": metadata.get("api_includes").and_then(|v| v.as_array()).map(|v| v.clone()),
            }
        });

        // Create Install request
        let install_request = serde_json::json!({
            "Install": {
                "package_id": {
                    "package_name": package_name,
                    "publisher_node": publisher,
                },
                "version_hash": version_hash,
                "metadata": onchain_metadata
            }
        });

        // Send Install request to app-store
        let request = Request::to(("our", "main", "app-store", "sys"))
            .body(serde_json::to_vec(&install_request).map_err(|e| e.to_string())?)
            .expects_response(15);

        let response = request
            .send_and_await_response(15)
            .map_err(|e| format!("Failed to send install request: {:?}", e))?
            .map_err(|e| format!("Install request failed: {:?}", e))?;

        // Parse response
        let response_body = String::from_utf8(response.body().to_vec())
            .map_err(|e| format!("Failed to parse response body: {}", e))?;
        let response_json: Value = serde_json::from_str(&response_body)
            .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

        // Check if Install was successful
        if let Some(install_response) = response_json.get("InstallResponse") {
            if install_response == &serde_json::Value::String("Success".to_string()) {
                println!("Spider: Package {} installed successfully!", package_name);
                Ok(())
            } else {
                Err(format!("Failed to install package: {:?}", install_response))
            }
        } else {
            Err(format!(
                "Unexpected response from app-store: {:?}",
                response_json
            ))
        }
    }
}
