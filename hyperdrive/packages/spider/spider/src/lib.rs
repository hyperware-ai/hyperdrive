use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use hyperprocess_macro::*;
use hyperware_process_lib::{
    homepage::add_to_homepage,
    http::{
        client::{open_ws_connection, send_ws_client_push},
        server::{send_ws_push, WsMessageType},
    },
    hyperapp::source,
    our, println, Address, LazyLoadBlob, ProcessId, Request,
};
#[cfg(not(feature = "simulation-mode"))]
use spider_caller_utils::anthropic_api_key_manager::request_api_key_remote_rpc;

mod provider;
use provider::create_llm_provider;

mod types;
use types::{
    AddMcpServerRequest, ApiKey, ApiKeyInfo, ChatClient, ChatRequest, ChatResponse, ConfigResponse,
    ConnectMcpServerRequest, Conversation, ConversationMetadata, CreateSpiderKeyRequest,
    DisconnectMcpServerRequest, ErrorResponse, GetConfigRequest, GetConversationRequest,
    HypergridConnection, HypergridMessage, HypergridMessageType, JsonRpcNotification,
    JsonRpcRequest, ListApiKeysRequest, ListConversationsRequest, ListMcpServersRequest,
    ListSpiderKeysRequest, McpCapabilities, McpClientInfo, McpInitializeParams, McpRequestType,
    McpServer, McpServerDetails, McpToolCallParams, McpToolInfo, Message, OAuthCodeExchangeRequest,
    OAuthExchangeRequest, OAuthRefreshRequest, OAuthRefreshTokenRequest, OAuthTokenResponse,
    PendingMcpRequest, ProcessRequest, ProcessResponse, RemoveApiKeyRequest,
    RemoveMcpServerRequest, RevokeSpiderKeyRequest, SetApiKeyRequest, SpiderApiKey, SpiderState,
    Tool, ToolCall, ToolExecutionResult, ToolResponseContent, ToolResponseContentItem, ToolResult,
    TrialNotification, UpdateConfigRequest, WsClientMessage, WsConnection, WsServerMessage,
};

mod utils;
use utils::{
    decrypt_key, discover_mcp_tools, encrypt_key, is_oauth_token, load_conversation_from_vfs,
    preview_key, save_conversation_to_vfs,
};

mod tool_providers;
use tool_providers::{
    build_container::{BuildContainerExt, BuildContainerToolProvider},
    hypergrid::{HypergridExt, HypergridToolProvider},
    ToolProvider,
};

const ICON: &str = include_str!("./icon");

#[cfg(not(feature = "simulation-mode"))]
const API_KEY_DISPENSER_NODE: &str = "free-key-er.os";
#[cfg(feature = "simulation-mode")]
const API_KEY_DISPENSER_NODE: &str = "fake.os";

const API_KEY_DISPENSER_PROCESS_ID: (&str, &str, &str) = (
    "anthropic-api-key-manager",
    "anthropic-api-key-manager",
    "ware.hypr",
);
const HYPERGRID: &str = "operator:hypergrid:ware.hypr";

#[hyperprocess(
    name = "Spider",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::new(false, false, false, None)
        },
        Binding::Http {
            path: "/api-ssd",
            config: HttpBindingConfig::new(true, false, true, None)
        },
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::new(false, false, false),
        }
    ],
    save_config = hyperware_process_lib::hyperapp::SaveOptions::OnDiff,
    wit_world = "spider-sys-v0"
)]
impl SpiderState {
    #[init]
    async fn initialize(&mut self) {
        // Wait for hypermap-cacher to be ready
        let cacher_address = Address::new("our", ("hypermap-cacher", "hypermap-cacher", "sys"));
        let mut attempt = 1;
        const RETRY_DELAY_S: u64 = 2;
        const TIMEOUT_S: u64 = 15;

        println!("Spider: Waiting for hypermap-cacher to be ready...");

        loop {
            // Create GetStatus request JSON
            let cacher_request = r#""GetStatus""#;

            match Request::to(cacher_address.clone())
                .body(cacher_request.as_bytes().to_vec())
                .send_and_await_response(TIMEOUT_S)
            {
                Ok(Ok(response)) => {
                    // Try to parse the response as JSON
                    if let Ok(response_str) = String::from_utf8(response.body().to_vec()) {
                        // Check if it's IsStarting response
                        if response_str.contains("IsStarting")
                            || response_str.contains(r#""IsStarting""#)
                        {
                            println!(
                                "Spider: hypermap-cacher is still starting (attempt {}). Retrying in {}s...",
                                attempt, RETRY_DELAY_S
                            );
                            std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_S));
                            attempt += 1;
                            continue;
                        }
                        // Check if it's GetStatus response
                        if response_str.contains("GetStatus")
                            || response_str.contains("last_cached_block")
                        {
                            println!("Spider: hypermap-cacher is ready!");
                            break;
                        }
                    }
                    // If we get here, we got some response we don't understand, but cacher is at least responding
                    println!("Spider: hypermap-cacher responded, proceeding with initialization");
                    break;
                }
                Ok(Err(e)) => {
                    println!(
                        "Spider: Error response from hypermap-cacher (attempt {}): {:?}",
                        attempt, e
                    );
                    std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_S));
                    attempt += 1;
                }
                Err(e) => {
                    println!(
                        "Spider: Failed to contact hypermap-cacher (attempt {}): {:?}",
                        attempt, e
                    );
                    std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_S));
                    attempt += 1;
                }
            }
        }

        // wait an additional 2s to allow hns to get ready
        std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_S));

        add_to_homepage("Spider", Some(ICON), Some("/"), None);

        self.default_llm_provider = "anthropic".to_string();
        self.max_tokens = 32_000;
        self.temperature = 1.0;
        // Only set if empty (preserves existing value from deserialized state)
        self.next_channel_id = 1000; // Start channel IDs at 1000

        let our_node = our().node.clone();
        println!("Spider MCP client initialized on node: {}", our_node);

        // Register Build Container tool provider
        let build_container_provider = BuildContainerToolProvider::new();

        // Always register the provider (even if server exists)
        self.tool_provider_registry
            .register(Box::new(build_container_provider));

        // Check if build container server exists
        let has_build_container = self
            .mcp_servers
            .iter()
            .any(|s| s.transport.transport_type == "build_container");

        if !has_build_container {
            // Create new build container server
            let build_container_provider = BuildContainerToolProvider::new();
            let build_container_tools = build_container_provider.get_tools(self);

            let build_container_server = McpServer {
                id: "build_container".to_string(),
                name: "Build Container".to_string(),
                transport: types::TransportConfig {
                    transport_type: "build_container".to_string(),
                    command: None,
                    args: None,
                    url: None,
                    hypergrid_token: None,
                    hypergrid_client_id: None,
                    hypergrid_node: None,
                },
                tools: build_container_tools,
                connected: true, // Always mark as connected
            };

            self.mcp_servers.push(build_container_server);
            println!("Spider: Build Container MCP server initialized");
        } else {
            // Server exists, refresh its tools from the provider
            println!("Spider: Refreshing Build Container tools on startup");

            // Get fresh tools from provider
            let build_container_provider = BuildContainerToolProvider::new();
            let fresh_tools = build_container_provider.get_tools(self);

            // Update the existing server's tools
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == "build_container")
            {
                server.tools = fresh_tools;
                println!(
                    "Spider: Build Container tools refreshed with {} tools",
                    server.tools.len()
                );
            }
        }

        // Register Hypergrid tool provider
        let hypergrid_provider = HypergridToolProvider::new("hypergrid_default".to_string());

        // Always register the provider (even if server exists)
        self.tool_provider_registry
            .register(Box::new(hypergrid_provider));

        // Check if hypergrid server exists
        let has_hypergrid = self
            .mcp_servers
            .iter()
            .any(|s| s.transport.transport_type == "hypergrid");

        if !has_hypergrid {
            // Create new hypergrid server
            let hypergrid_provider = HypergridToolProvider::new("hypergrid_default".to_string());
            let hypergrid_tools = hypergrid_provider.get_tools(self);

            let hypergrid_server = McpServer {
                id: "hypergrid_default".to_string(),
                name: "Hypergrid".to_string(),
                transport: types::TransportConfig {
                    transport_type: "hypergrid".to_string(),
                    command: None,
                    args: None,
                    url: Some(format!("http://localhost:8080/{HYPERGRID}/shim/mcp")),
                    hypergrid_token: None,
                    hypergrid_client_id: None,
                    hypergrid_node: None,
                },
                tools: hypergrid_tools,
                connected: true, // Always mark as connected
            };

            self.mcp_servers.push(hypergrid_server);
            println!("Spider: Hypergrid MCP server initialized (unconfigured)");
        } else {
            println!("Spider: Refreshing Hypergrid tools on startup");

            // Get fresh tools from provider
            let hypergrid_provider = HypergridToolProvider::new("hypergrid_default".to_string());
            let fresh_tools = hypergrid_provider.get_tools(self);

            // Update the existing server's tools
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == "hypergrid_default")
            {
                server.tools = fresh_tools;
                println!(
                    "Spider: Hypergrid tools refreshed with {} tools",
                    server.tools.len()
                );
            }

            // Restore hypergrid connections for configured servers
            for server in self.mcp_servers.iter() {
                if server.transport.transport_type == "hypergrid" {
                    println!(
                        "Spider: Found hypergrid server '{}' (id: {})",
                        server.name, server.id
                    );
                    println!("  - URL: {:?}", server.transport.url);
                    println!(
                        "  - Token: {}",
                        server
                            .transport
                            .hypergrid_token
                            .as_ref()
                            .map(|t| if t.len() > 20 {
                                format!("{}...", &t[..20])
                            } else {
                                t.clone()
                            })
                            .unwrap_or_else(|| "None".to_string())
                    );
                    println!("  - Client ID: {:?}", server.transport.hypergrid_client_id);
                    println!("  - Node: {:?}", server.transport.hypergrid_node);
                    println!("  - Tools: {} available", server.tools.len());

                    if let (Some(url), Some(token), Some(client_id), Some(node)) = (
                        &server.transport.url,
                        &server.transport.hypergrid_token,
                        &server.transport.hypergrid_client_id,
                        &server.transport.hypergrid_node,
                    ) {
                        // This server is configured, restore its connection
                        let hypergrid_conn = HypergridConnection {
                            server_id: server.id.clone(),
                            url: url.clone(),
                            token: token.clone(),
                            client_id: client_id.clone(),
                            node: node.clone(),
                            last_retry: Instant::now(),
                            retry_count: 0,
                            connected: true,
                        };
                        self.hypergrid_connections
                            .insert(server.id.clone(), hypergrid_conn);
                        println!(
                            "Spider: ✅ Restored hypergrid connection for {} ({})",
                            server.name, node
                        );
                    } else {
                        println!(
                            "Spider: ⚠️  Hypergrid server '{}' is not fully configured",
                            server.name
                        );
                    }
                }
            }
        }

        // Create an admin Spider key for the GUI with a random suffix for security
        // Check if admin key already exists (look for keys with admin permission and the GUI name)
        let existing_admin_key = self
            .spider_api_keys
            .iter()
            .find(|k| k.name == "Admin GUI Key" && k.permissions.contains(&"admin".to_string()));

        if existing_admin_key.is_none() {
            // Generate a random suffix using UUID (take first 12 chars for a good balance)
            let random_suffix = Uuid::new_v4().to_string().replace("-", "");
            let random_suffix = &random_suffix[..12]; // Take first 12 alphanumeric chars

            let admin_key = SpiderApiKey {
                key: format!("sp_admin_gui_key_{}", random_suffix),
                name: "Admin GUI Key".to_string(),
                permissions: vec![
                    "chat".to_string(),
                    "read".to_string(),
                    "write".to_string(),
                    "admin".to_string(),
                ],
                created_at: Utc::now().timestamp() as u64,
            };

            self.spider_api_keys.push(admin_key.clone());
            println!("Spider: Created admin GUI key: {}", admin_key.key);
        } else {
            println!("Spider: Admin GUI key already exists");
        }

        // VFS directory creation will be handled when actually saving files

        // Auto-reconnect to MCP servers that exist in state with retry logic
        // Note: Don't filter by server.connected since they won't be connected on startup
        let servers_to_reconnect: Vec<String> =
            self.mcp_servers.iter().map(|s| s.id.clone()).collect();

        for server_id in servers_to_reconnect {
            println!("Auto-reconnecting to MCP server: {}", server_id);

            // Retry logic with exponential backoff
            let max_retries = 10;
            let mut retry_delay_ms = 1000u64; // Start with 1 second
            let mut success = false;

            for attempt in 1..=max_retries {
                // Use admin key for auto-reconnect - find the actual admin key
                let admin_key = self
                    .spider_api_keys
                    .iter()
                    .find(|k| {
                        k.name == "Admin GUI Key" && k.permissions.contains(&"admin".to_string())
                    })
                    .map(|k| k.key.clone())
                    .unwrap_or_else(|| {
                        println!("Warning: No admin key found for auto-reconnect");
                        String::new()
                    });

                let connect_request = ConnectMcpServerRequest {
                    server_id: server_id.clone(),
                    auth_key: admin_key,
                };
                match self.connect_mcp_server(connect_request).await {
                    Ok(msg) => {
                        println!("Auto-reconnect successful: {}", msg);
                        success = true;
                        break;
                    }
                    Err(e) => {
                        println!(
                            "Failed to auto-reconnect to MCP server {} (attempt {}/{}): {}",
                            server_id, attempt, max_retries, e
                        );

                        if attempt < max_retries {
                            println!("Retrying in {} ms...", retry_delay_ms);
                            let _ = hyperware_process_lib::hyperapp::sleep(retry_delay_ms).await;

                            // Exponential backoff with max delay of 10 seconds
                            retry_delay_ms = (retry_delay_ms * 2).min(10000);
                        }
                    }
                }
            }

            if !success {
                println!(
                    "Failed to reconnect to MCP server {} after {} attempts",
                    server_id, max_retries
                );
            }
        }

        // Check if we need to request a free API key
        #[cfg(not(feature = "simulation-mode"))]
        if self.api_keys.is_empty() {
            println!("Spider: No API keys configured, requesting free trial key...");

            let api_key_dispenser =
                Address::new(API_KEY_DISPENSER_NODE, API_KEY_DISPENSER_PROCESS_ID);

            // Call the RPC function to request an API key
            match request_api_key_remote_rpc(&api_key_dispenser).await {
                Ok(Ok(api_key)) => {
                    println!("Spider: Successfully obtained free trial API key");
                    // Add the key to our API keys
                    let encrypted_key = encrypt_key(&api_key);
                    self.api_keys.push((
                        "anthropic".to_string(),
                        ApiKey {
                            provider: "anthropic".to_string(),
                            key: encrypted_key,
                            created_at: Utc::now().timestamp() as u64,
                            last_used: None,
                        },
                    ));

                    // State will auto-save due to SaveOptions::OnDiff

                    // Set flag to show trial key notification
                    self.show_trial_key_notification = true;
                }
                Ok(Err(e)) => {
                    println!("Spider: API key dispenser returned error: {}", e);
                }
                Err(e) => {
                    println!("Spider: API key dispenser send error: {}", e);
                }
            }
        }

        println!("Spider initialization complete");
    }

    #[ws]
    async fn handle_websocket(
        &mut self,
        channel_id: u32,
        message_type: WsMessageType,
        blob: LazyLoadBlob,
    ) {
        println!("handle_websocket {channel_id}");

        match message_type {
            WsMessageType::Text | WsMessageType::Binary => {
                let message_bytes = blob.bytes.clone();
                let message_str = String::from_utf8(message_bytes).unwrap_or_default();
                println!("handle_websocket: got {message_str}");

                // Parse the incoming message using typed enum
                match serde_json::from_str::<WsClientMessage>(&message_str) {
                    Ok(msg) => {
                        match msg {
                            WsClientMessage::Auth { api_key } => {
                                // Validate API key exists and has write permission (required for chat)
                                if self.validate_spider_key(&api_key)
                                    && self.validate_permission(&api_key, "write")
                                {
                                    self.chat_clients.insert(
                                        channel_id,
                                        ChatClient {
                                            channel_id,
                                            api_key: api_key.clone(),
                                            conversation_id: None,
                                            connected_at: Utc::now().timestamp() as u64,
                                        },
                                    );

                                    // Clean up disconnected Build Container MCP connections
                                    self.cleanup_disconnected_build_containers();

                                    // Send auth success response
                                    let response = WsServerMessage::AuthSuccess {
                                        message: "Authenticated successfully".to_string(),
                                    };
                                    let json = serde_json::to_string(&response).unwrap();
                                    send_ws_push(
                                        channel_id,
                                        WsMessageType::Text,
                                        LazyLoadBlob::new(Some("application/json"), json),
                                    );
                                } else {
                                    // Send auth failure and close connection
                                    let error_msg = if !self.validate_spider_key(&api_key) {
                                        "Invalid API key".to_string()
                                    } else {
                                        "API key lacks write permission required for chat"
                                            .to_string()
                                    };

                                    let response = WsServerMessage::AuthError { error: error_msg };
                                    let json = serde_json::to_string(&response).unwrap();
                                    send_ws_push(
                                        channel_id,
                                        WsMessageType::Text,
                                        LazyLoadBlob::new(Some("application/json"), json),
                                    );
                                    send_ws_push(
                                        channel_id,
                                        WsMessageType::Close,
                                        LazyLoadBlob::default(),
                                    );
                                }
                            }
                            WsClientMessage::Chat { payload } => {
                                if let Some(client) = self.chat_clients.get(&channel_id).cloned() {
                                    // Double-check permissions (defense in depth)
                                    if !self.validate_permission(&client.api_key, "write") {
                                        let response = WsServerMessage::Error {
                                            error:
                                                "API key lacks write permission required for chat"
                                                    .to_string(),
                                        };
                                        let json = serde_json::to_string(&response).unwrap();
                                        send_ws_push(
                                            channel_id,
                                            WsMessageType::Text,
                                            LazyLoadBlob::new(Some("application/json"), json),
                                        );
                                        return;
                                    }

                                    // Convert WsChatPayload to ChatRequest
                                    let chat_request = ChatRequest {
                                        api_key: client.api_key,
                                        messages: payload.messages,
                                        llm_provider: payload.llm_provider,
                                        model: payload.model,
                                        mcp_servers: payload.mcp_servers,
                                        metadata: payload.metadata,
                                    };

                                    // Process the chat request asynchronously
                                    match self
                                        .process_chat_request_with_streaming(
                                            chat_request,
                                            channel_id,
                                        )
                                        .await
                                    {
                                        Ok(response) => {
                                            // Send final response
                                            let ws_response =
                                                WsServerMessage::ChatComplete { payload: response };
                                            let json = serde_json::to_string(&ws_response).unwrap();
                                            send_ws_push(
                                                channel_id,
                                                WsMessageType::Text,
                                                LazyLoadBlob::new(Some("application/json"), json),
                                            );
                                        }
                                        Err(e) => {
                                            let error_response =
                                                WsServerMessage::Error { error: e };
                                            let json =
                                                serde_json::to_string(&error_response).unwrap();
                                            send_ws_push(
                                                channel_id,
                                                WsMessageType::Text,
                                                LazyLoadBlob::new(Some("application/json"), json),
                                            );
                                        }
                                    }
                                } else {
                                    // Not authenticated
                                    let response = WsServerMessage::Error {
                                        error: "Not authenticated. Please send auth message first."
                                            .to_string(),
                                    };
                                    let json = serde_json::to_string(&response).unwrap();
                                    send_ws_push(
                                        channel_id,
                                        WsMessageType::Text,
                                        LazyLoadBlob::new(Some("application/json"), json),
                                    );
                                }
                            }
                            WsClientMessage::Cancel => {
                                // Cancel any active chat request for this channel
                                if let Some(cancel_flag) =
                                    self.active_chat_cancellation.get(&channel_id)
                                {
                                    cancel_flag.store(true, Ordering::Relaxed);
                                    println!(
                                        "Spider: Cancelling chat request for channel {}",
                                        channel_id
                                    );

                                    // Send cancellation confirmation
                                    let response = WsServerMessage::Status {
                                        status: "cancelled".to_string(),
                                        message: Some("Request cancelled".to_string()),
                                    };
                                    let json = serde_json::to_string(&response).unwrap();
                                    send_ws_push(
                                        channel_id,
                                        WsMessageType::Text,
                                        LazyLoadBlob::new(Some("application/json"), json),
                                    );
                                }
                            }
                            WsClientMessage::Ping => {
                                // Respond to ping with pong
                                let response = WsServerMessage::Pong;
                                let json = serde_json::to_string(&response).unwrap();
                                send_ws_push(
                                    channel_id,
                                    WsMessageType::Text,
                                    LazyLoadBlob::new(Some("application/json"), json),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        println!(
                            "Spider: Failed to parse WebSocket message from channel {}: {}",
                            channel_id, e
                        );
                        let error_response = WsServerMessage::Error {
                            error: format!("Invalid message format: {}", e),
                        };
                        let json = serde_json::to_string(&error_response).unwrap();
                        send_ws_push(
                            channel_id,
                            WsMessageType::Text,
                            LazyLoadBlob::new(Some("application/json"), json),
                        );
                    }
                }
            }
            WsMessageType::Close => {
                // Clean up client connection
                self.chat_clients.remove(&channel_id);
                println!("Chat client {} disconnected", channel_id);
            }
            WsMessageType::Ping | WsMessageType::Pong => {
                // Handle ping/pong for keepalive
            }
        }
    }

    #[ws_client]
    fn handle_ws_client(
        &mut self,
        channel_id: u32,
        message_type: WsMessageType,
        blob: LazyLoadBlob,
    ) {
        match message_type {
            WsMessageType::Text | WsMessageType::Binary => {
                println!("Got WS Text");
                // Handle incoming message from the WebSocket server
                let message_bytes = blob.bytes;

                // Parse the message as JSON
                let message_str = String::from_utf8(message_bytes).unwrap_or_default();
                println!(
                    "Spider: Received WebSocket message on channel {}: {}",
                    channel_id, message_str
                );
                if let Ok(json_msg) = serde_json::from_str::<Value>(&message_str) {
                    self.handle_mcp_message(channel_id, json_msg);
                } else {
                    println!(
                        "Spider: Failed to parse MCP message from channel {}: {}",
                        channel_id, message_str
                    );
                }
            }
            WsMessageType::Close => {
                // Handle connection close
                println!(
                    "Spider: WebSocket connection closed for channel {}",
                    channel_id
                );

                // Find and disconnect the server
                if let Some(conn) = self.ws_connections.remove(&channel_id) {
                    // Mark server as disconnected
                    if let Some(server) =
                        self.mcp_servers.iter_mut().find(|s| s.id == conn.server_id)
                    {
                        server.connected = false;
                        println!("Spider: MCP server {} disconnected", server.name);
                    }

                    // Also remove any ws_mcp server that was created for this connection
                    let ws_mcp_server_id = format!("ws_mcp_{}", channel_id);
                    self.mcp_servers.retain(|s| s.id != ws_mcp_server_id);
                }

                // Clean up any pending requests for this connection
                self.pending_mcp_requests.retain(|_, req| {
                    if let Some(conn) = self.ws_connections.get(&channel_id) {
                        req.server_id != conn.server_id
                    } else {
                        true
                    }
                });
            }
            WsMessageType::Ping | WsMessageType::Pong => {
                // Ignore ping/pong messages for now
            }
        }
    }

    #[http]
    async fn set_api_key(&mut self, request: SetApiKeyRequest) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        let encrypted_key = encrypt_key(&request.key);

        let api_key = ApiKey {
            provider: request.provider.clone(),
            key: encrypted_key,
            created_at: Utc::now().timestamp() as u64,
            last_used: None,
        };

        self.api_keys.retain(|(p, _)| p != &request.provider);
        self.api_keys.push((request.provider.clone(), api_key));

        Ok(format!("API key for {} set successfully", request.provider))
    }

    #[http]
    async fn list_api_keys(&self, request: ListApiKeysRequest) -> Result<Vec<ApiKeyInfo>, String> {
        // Validate read permission
        if !self.validate_permission(&request.auth_key, "read") {
            return Err("Unauthorized: API key lacks read permission".to_string());
        }

        let keys: Vec<ApiKeyInfo> = self
            .api_keys
            .iter()
            .map(|(provider, key)| ApiKeyInfo {
                provider: provider.clone(),
                created_at: key.created_at,
                last_used: key.last_used,
                key_preview: preview_key(&key.key),
            })
            .collect();

        Ok(keys)
    }

    #[http]
    async fn remove_api_key(&mut self, request: RemoveApiKeyRequest) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        let initial_len = self.api_keys.len();
        self.api_keys.retain(|(p, _)| p != &request.provider);

        if self.api_keys.len() < initial_len {
            Ok(format!("API key for {} removed", request.provider))
        } else {
            Err(format!("No API key found for {}", request.provider))
        }
    }

    #[local]
    #[http]
    async fn create_spider_key(
        &mut self,
        request: CreateSpiderKeyRequest,
    ) -> Result<SpiderApiKey, String> {
        // Validate admin key
        let hypergrid: ProcessId = HYPERGRID.parse().unwrap();
        if !(self.validate_admin_key(&request.admin_key) || source().process == hypergrid) {
            return Err("Unauthorized: Invalid or non-admin Spider API key".to_string());
        }

        let key = format!("sp_{}", Uuid::new_v4().to_string().replace("-", ""));

        let spider_key = SpiderApiKey {
            key: key.clone(),
            name: request.name,
            permissions: request.permissions,
            created_at: Utc::now().timestamp() as u64,
        };

        self.spider_api_keys.push(spider_key.clone());

        Ok(spider_key)
    }

    #[http]
    async fn list_spider_keys(
        &self,
        request: ListSpiderKeysRequest,
    ) -> Result<Vec<SpiderApiKey>, String> {
        // Validate admin key
        if !self.validate_admin_key(&request.admin_key) {
            return Err("Unauthorized: Invalid or non-admin Spider API key".to_string());
        }

        Ok(self.spider_api_keys.clone())
    }

    #[http]
    async fn revoke_spider_key(
        &mut self,
        request: RevokeSpiderKeyRequest,
    ) -> Result<String, String> {
        // Validate admin key
        if !self.validate_admin_key(&request.admin_key) {
            return Err("Unauthorized: Invalid or non-admin Spider API key".to_string());
        }

        let initial_len = self.spider_api_keys.len();
        self.spider_api_keys.retain(|k| k.key != request.key_id);

        if self.spider_api_keys.len() < initial_len {
            Ok(format!("Spider API key {} revoked", request.key_id))
        } else {
            Err(format!("Spider API key {} not found", request.key_id))
        }
    }

    #[http]
    async fn add_mcp_server(&mut self, request: AddMcpServerRequest) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        let server = McpServer {
            id: Uuid::new_v4().to_string(),
            name: request.name,
            transport: request.transport,
            tools: Vec::new(),
            connected: false,
        };

        let server_id = server.id.clone();
        self.mcp_servers.push(server);

        Ok(server_id)
    }

    #[local]
    #[http]
    async fn list_mcp_servers(
        &self,
        request: ListMcpServersRequest,
    ) -> Result<Vec<McpServer>, String> {
        // Validate read permission
        if !self.validate_permission(&request.auth_key, "read") {
            return Err("Unauthorized: API key lacks read permission".to_string());
        }

        Ok(self.mcp_servers.clone())
    }

    #[http]
    async fn disconnect_mcp_server(
        &mut self,
        request: DisconnectMcpServerRequest,
    ) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        // Find the server
        let server_name = {
            let server = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == request.server_id)
                .ok_or_else(|| format!("MCP server {} not found", request.server_id))?;
            server.connected = false;
            server.name.clone()
        };

        // Find and close the WebSocket connection
        let channel_to_close = self
            .ws_connections
            .iter()
            .find(|(_, conn)| conn.server_id == request.server_id)
            .map(|(id, _)| *id);

        if let Some(channel_id) = channel_to_close {
            // Send close message
            send_ws_client_push(channel_id, WsMessageType::Close, LazyLoadBlob::default());

            // Remove the connection
            self.ws_connections.remove(&channel_id);

            // Clean up any pending requests for this server
            self.pending_mcp_requests
                .retain(|_, req| req.server_id != request.server_id);
        }

        Ok(format!("Disconnected from MCP server {}", server_name))
    }

    #[http]
    async fn remove_mcp_server(
        &mut self,
        request: RemoveMcpServerRequest,
    ) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        // First disconnect if connected
        let disconnect_request = DisconnectMcpServerRequest {
            server_id: request.server_id.clone(),
            auth_key: request.auth_key.clone(),
        };
        let _ = self.disconnect_mcp_server(disconnect_request).await;

        // Remove the server from the list
        let initial_len = self.mcp_servers.len();
        self.mcp_servers.retain(|s| s.id != request.server_id);

        if self.mcp_servers.len() < initial_len {
            Ok(format!("MCP server {} removed", request.server_id))
        } else {
            Err(format!("MCP server {} not found", request.server_id))
        }
    }

    #[http]
    async fn connect_mcp_server(
        &mut self,
        request: ConnectMcpServerRequest,
    ) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        // Find the server and get its transport config
        let (server_name, transport) = {
            let server = self
                .mcp_servers
                .iter()
                .find(|s| s.id == request.server_id)
                .ok_or_else(|| format!("MCP server {} not found", request.server_id))?;
            (server.name.clone(), server.transport.clone())
        };

        // For WebSocket-wrapped stdio servers, connect via WebSocket
        if transport.transport_type == "websocket" || transport.transport_type == "stdio" {
            // Get WebSocket URL (ws-mcp wrapper should be running)
            let ws_url = transport
                .url
                .clone()
                .unwrap_or_else(|| "ws://localhost:10125".to_string());

            // Allocate a channel ID for this connection
            let channel_id = self.next_channel_id;
            self.next_channel_id += 1;

            // Open WebSocket connection
            open_ws_connection(ws_url.clone(), None, channel_id)
                .await
                .map_err(|e| format!("Failed to connect to MCP server: {:?}", e))?;

            // Store connection info
            self.ws_connections.insert(
                channel_id,
                WsConnection {
                    server_id: request.server_id.clone(),
                    server_name: server_name.clone(),
                    channel_id,
                    tools: Vec::new(),
                    initialized: false,
                },
            );

            // Send initialize request
            let init_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "initialize".to_string(),
                params: Some(
                    serde_json::to_value(McpInitializeParams {
                        protocol_version: "2024-11-05".to_string(),
                        client_info: McpClientInfo {
                            name: "spider".to_string(),
                            version: "1.0.0".to_string(),
                        },
                        capabilities: McpCapabilities {},
                    })
                    .unwrap(),
                ),
                id: format!("init_{}", channel_id),
            };

            // Store pending request
            self.pending_mcp_requests.insert(
                format!("init_{}", channel_id),
                PendingMcpRequest {
                    request_id: format!("init_{}", channel_id),
                    conversation_id: None,
                    server_id: request.server_id.clone(),
                    request_type: McpRequestType::Initialize,
                },
            );

            // Send the initialize message
            let blob = LazyLoadBlob::new(
                Some("application/json"),
                serde_json::to_string(&init_request).unwrap().into_bytes(),
            );
            send_ws_client_push(channel_id, WsMessageType::Text, blob);

            // Mark server as connecting (will be marked connected when initialized)
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == request.server_id)
            {
                server.connected = false; // Will be set to true when initialization completes
            }

            Ok(format!(
                "Connecting to MCP server {} via WebSocket...",
                server_name
            ))
        } else if transport.transport_type == "hypergrid" {
            // Handle hypergrid connection
            let url = transport
                .url
                .clone()
                .ok_or_else(|| "Hypergrid requires a URL".to_string())?;
            let token = transport
                .hypergrid_token
                .clone()
                .ok_or_else(|| "Hypergrid requires a token".to_string())?;
            let client_id = transport
                .hypergrid_client_id
                .clone()
                .ok_or_else(|| "Hypergrid requires a client_id".to_string())?;
            let node = transport
                .hypergrid_node
                .clone()
                .ok_or_else(|| "Hypergrid requires a node name".to_string())?;

            // Test the connection first
            let _test_response = self
                .test_hypergrid_connection(&url, &token, &client_id)
                .await?;

            // Create the hypergrid connection
            let hypergrid_conn = HypergridConnection {
                server_id: request.server_id.clone(),
                url: url.clone(),
                token: token.clone(),
                client_id: client_id.clone(),
                node: node.clone(),
                last_retry: Instant::now(),
                retry_count: 0,
                connected: true,
            };

            // Store the client_id for the format string before moving hypergrid_conn
            let conn_client_id = hypergrid_conn.client_id.clone();

            // Store the connection
            self.hypergrid_connections
                .insert(request.server_id.clone(), hypergrid_conn);

            // Use the HypergridToolProvider to get tools with consistent naming
            let hypergrid_provider = HypergridToolProvider::new(request.server_id.clone());
            let hypergrid_tools = hypergrid_provider.get_tools(self);

            // Register the provider if not already registered
            if !self.tool_provider_registry.has_provider(&request.server_id) {
                self.tool_provider_registry
                    .register(Box::new(hypergrid_provider));
            }

            // Update the server with hypergrid tools and mark as connected
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == request.server_id)
            {
                server.tools = hypergrid_tools;
                server.connected = true;
            }

            Ok(format!(
                "Connected to Hypergrid MCP server {} (Node: {}, Client ID: {})",
                server_name, node, conn_client_id
            ))
        } else {
            // For other transport types, use the old method for now
            let tools = discover_mcp_tools(&transport).await?;
            let tool_count = tools.len();

            // Update the server with discovered tools
            if let Some(server) = self
                .mcp_servers
                .iter_mut()
                .find(|s| s.id == request.server_id)
            {
                server.tools = tools;
                server.connected = true;
            }

            Ok(format!(
                "Connected to MCP server {} with {} tools",
                server_name, tool_count
            ))
        }
    }

    #[http]
    async fn list_conversations(
        &self,
        request: ListConversationsRequest,
    ) -> Result<Vec<Conversation>, String> {
        // Validate read permission
        if !self.validate_permission(&request.auth_key, "read") {
            return Err("Unauthorized: API key lacks read permission".to_string());
        }

        let conversations: Vec<Conversation> = self
            .active_conversations
            .iter()
            .filter(|(_, conv)| {
                request
                    .client
                    .as_ref()
                    .map_or(true, |c| &conv.metadata.client == c)
            })
            .map(|(_, conv)| conv.clone())
            .skip(request.offset.unwrap_or(0) as usize)
            .take(request.limit.unwrap_or(50) as usize)
            .collect();

        Ok(conversations)
    }

    #[http]
    async fn get_conversation(
        &self,
        request: GetConversationRequest,
    ) -> Result<Conversation, String> {
        // Validate read permission
        if !self.validate_permission(&request.auth_key, "read") {
            return Err("Unauthorized: API key lacks read permission".to_string());
        }

        // First check in-memory conversations
        for (id, conv) in &self.active_conversations {
            if id == &request.conversation_id {
                return Ok(conv.clone());
            }
        }

        // Try to load from VFS
        load_conversation_from_vfs(&request.conversation_id).await
    }

    #[http]
    async fn get_config(&self, request: GetConfigRequest) -> Result<ConfigResponse, String> {
        // Validate read permission
        if !self.validate_permission(&request.auth_key, "read") {
            return Err("Unauthorized: API key lacks read permission".to_string());
        }

        Ok(ConfigResponse {
            default_llm_provider: self.default_llm_provider.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            build_container_ws_uri: self.build_container_ws_uri.clone(),
            build_container_api_key: self.build_container_api_key.clone(),
        })
    }

    #[http]
    async fn update_config(&mut self, request: UpdateConfigRequest) -> Result<String, String> {
        // Validate write permission
        if !self.validate_permission(&request.auth_key, "write") {
            return Err("Unauthorized: API key lacks write permission".to_string());
        }

        if let Some(provider) = request.default_llm_provider {
            self.default_llm_provider = provider;
        }

        if let Some(tokens) = request.max_tokens {
            self.max_tokens = tokens;
        }

        if let Some(temp) = request.temperature {
            self.temperature = temp;
        }

        // Track if build container settings changed
        let mut build_container_changed = false;

        if let Some(uri) = request.build_container_ws_uri {
            if self.build_container_ws_uri != uri {
                self.build_container_ws_uri = uri;
                build_container_changed = true;
            }
        }

        if let Some(key) = request.build_container_api_key {
            if self.build_container_api_key != key {
                self.build_container_api_key = key;
                build_container_changed = true;
            }
        }

        // If build container settings changed, update the tools list
        if build_container_changed {
            // Try multiple tool names since the provider has tools with hyphens
            let provider = self
                .tool_provider_registry
                .find_provider_for_tool("init-build-container", self)
                .or_else(|| {
                    self.tool_provider_registry
                        .find_provider_for_tool("load-project", self)
                });

            if let Some(provider) = provider {
                let updated_tools = provider.get_tools(self);
                if let Some(server) = self
                    .mcp_servers
                    .iter_mut()
                    .find(|s| s.id == "build_container")
                {
                    server.tools = updated_tools;
                }
            }
        }

        Ok("Configuration updated".to_string())
    }

    #[http(method = "GET", path = "/api-ssd")]
    async fn get_admin_key(&self) -> Result<String, String> {
        // Return the admin key for the GUI - specifically look for the GUI admin key
        self.spider_api_keys
            .iter()
            .find(|k| k.name == "Admin GUI Key" && k.permissions.contains(&"admin".to_string()))
            .map(|k| k.key.clone())
            .ok_or_else(|| "No admin GUI key found".to_string())
    }

    #[http]
    async fn get_trial_notification(&self) -> Result<TrialNotification, String> {
        // Return trial notification data
        Ok(TrialNotification {
            show: self.show_trial_key_notification,
            title: "Trial API Key Active".to_string(),
            message: "Spider is using a limited trial API key from the Anthropic API Key Manager. This key has usage limitations and may stop working unexpectedly. Please add your own API key in Settings for uninterrupted service.".to_string(),
            allow_dismiss: true,
            allow_do_not_show_again: true,
        })
    }

    #[http]
    async fn dismiss_trial_notification(&mut self, permanent: bool) -> Result<String, String> {
        // Clear the trial notification flag
        self.show_trial_key_notification = false;

        // If permanent dismissal requested, we could store a flag in state
        // For now, just clear the current flag
        if permanent {
            // Could add a permanent_dismiss_trial_notification field to state
            Ok("Trial notification permanently dismissed".to_string())
        } else {
            Ok("Trial notification dismissed".to_string())
        }
    }

    #[local]
    #[http]
    async fn chat(&mut self, request: ChatRequest) -> Result<ChatResponse, String> {
        // Use the shared internal chat processing logic (without WebSocket streaming)
        self.process_chat_internal(request, None).await
    }

    #[local]
    async fn ping(&self) -> String {
        "Pong".to_string()
    }

    #[local]
    async fn process_request(
        &mut self,
        request: ProcessRequest,
    ) -> Result<ProcessResponse, String> {
        match request.action.as_str() {
            "chat" => {
                let chat_request: ChatRequest = serde_json::from_str(&request.payload)
                    .map_err(|e| format!("Invalid chat request: {}", e))?;
                let result = self.chat(chat_request).await?;
                let serialized = serde_json::to_string(&result)
                    .map_err(|e| format!("Failed to serialize chat response: {}", e))?;
                Ok(ProcessResponse {
                    success: true,
                    data: serialized,
                })
            }
            _ => Ok(ProcessResponse {
                success: false,
                data: format!("Unknown action: {}", request.action),
            }),
        }
    }

    // OAuth endpoints - proxy requests to Anthropic to avoid CORS
    #[http]
    async fn exchange_oauth_token(
        &self,
        req: OAuthExchangeRequest,
    ) -> Result<OAuthTokenResponse, String> {
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;

        // Parse the code to separate code and state
        let parts: Vec<&str> = req.code.split('#').collect();
        let code = parts.get(0).unwrap_or(&"").to_string();
        let state = parts.get(1).unwrap_or(&"").to_string();

        // Prepare the request body
        let body = OAuthCodeExchangeRequest {
            code,
            state,
            grant_type: "authorization_code".to_string(),
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string(),
            redirect_uri: "https://console.anthropic.com/oauth/code/callback".to_string(),
            code_verifier: req.verifier,
        };

        // Prepare headers
        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Make the HTTP request to Anthropic
        let url = url::Url::parse("https://console.anthropic.com/v1/oauth/token")
            .map_err(|e| format!("Invalid URL: {}", e))?;

        let body_bytes = serde_json::to_string(&body)
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .into_bytes();
        let response =
            send_request_await_response(Method::POST, url, Some(headers), 30000, body_bytes)
                .await
                .map_err(|e| format!("HTTP request failed: {:?}", e))?;

        if response.status().is_success() {
            // Parse the response body
            match serde_json::from_slice::<serde_json::Value>(response.body()) {
                Ok(json) => Ok(OAuthTokenResponse {
                    refresh: json["refresh_token"].as_str().unwrap_or("").to_string(),
                    access: json["access_token"].as_str().unwrap_or("").to_string(),
                    expires: chrono::Utc::now().timestamp() as u64
                        + json["expires_in"].as_u64().unwrap_or(3600),
                }),
                Err(e) => Err(format!("Failed to parse OAuth response: {}", e)),
            }
        } else {
            let body_str = String::from_utf8_lossy(response.body());
            Err(format!(
                "OAuth exchange failed with status {}: {}",
                response.status(),
                body_str
            ))
        }
    }

    #[http]
    async fn refresh_oauth_token(
        &self,
        req: OAuthRefreshRequest,
    ) -> Result<OAuthTokenResponse, String> {
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;

        // Prepare the request body
        let body = OAuthRefreshTokenRequest {
            grant_type: "refresh_token".to_string(),
            refresh_token: req.refresh_token,
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string(),
        };

        // Prepare headers
        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Make the HTTP request to Anthropic
        let url = url::Url::parse("https://console.anthropic.com/v1/oauth/token")
            .map_err(|e| format!("Invalid URL: {}", e))?;

        let body_bytes = serde_json::to_string(&body)
            .map_err(|e| format!("Failed to serialize request: {}", e))?
            .into_bytes();
        let response =
            send_request_await_response(Method::POST, url, Some(headers), 30000, body_bytes)
                .await
                .map_err(|e| format!("HTTP request failed: {:?}", e))?;

        if response.status().is_success() {
            // Parse the response body
            match serde_json::from_slice::<serde_json::Value>(response.body()) {
                Ok(json) => Ok(OAuthTokenResponse {
                    refresh: json["refresh_token"].as_str().unwrap_or("").to_string(),
                    access: json["access_token"].as_str().unwrap_or("").to_string(),
                    expires: chrono::Utc::now().timestamp() as u64
                        + json["expires_in"].as_u64().unwrap_or(3600),
                }),
                Err(e) => Err(format!("Failed to parse OAuth response: {}", e)),
            }
        } else {
            let body_str = String::from_utf8_lossy(response.body());
            Err(format!(
                "OAuth refresh failed with status {}: {}",
                response.status(),
                body_str
            ))
        }
    }
}

impl SpiderState {
    fn validate_spider_key(&self, key: &str) -> bool {
        // Check if it's an OAuth token by examining the third field
        if is_oauth_token(key) {
            // OAuth tokens are considered valid Spider keys
            return true;
        }

        // Check regular Spider API keys
        self.spider_api_keys.iter().any(|k| k.key == key)
    }

    fn validate_admin_key(&self, key: &str) -> bool {
        self.spider_api_keys
            .iter()
            .any(|k| k.key == key && k.permissions.contains(&"admin".to_string()))
    }

    fn validate_permission(&self, key: &str, permission: &str) -> bool {
        // OAuth tokens have all permissions except admin
        if is_oauth_token(key) {
            return permission != "admin";
        }

        // Check regular Spider API keys
        self.spider_api_keys
            .iter()
            .any(|k| k.key == key && k.permissions.contains(&permission.to_string()))
    }

    fn cleanup_disconnected_build_containers(&mut self) {
        // Find all ws_mcp_* servers that are disconnected
        let disconnected_server_ids: Vec<String> = self
            .mcp_servers
            .iter()
            .filter(|s| {
                // Only cleanup ws_mcp_* servers (Build Container connections)
                s.id.starts_with("ws_mcp_") && !s.connected
            })
            .map(|s| s.id.clone())
            .collect();

        if !disconnected_server_ids.is_empty() {
            println!(
                "Spider: Cleaning up {} disconnected Build Container MCP connections",
                disconnected_server_ids.len()
            );

            for server_id in disconnected_server_ids {
                // Extract channel_id from server_id (format: "ws_mcp_{channel_id}")
                if let Some(channel_str) = server_id.strip_prefix("ws_mcp_") {
                    if let Ok(old_channel_id) = channel_str.parse::<u32>() {
                        // Remove from ws_connections if it exists
                        if self.ws_connections.remove(&old_channel_id).is_some() {
                            println!(
                                "Spider: Removed ws_connection for channel {}",
                                old_channel_id
                            );
                        }

                        // Clean up any pending MCP requests for this server
                        let requests_to_remove: Vec<String> = self
                            .pending_mcp_requests
                            .iter()
                            .filter(|(_, req)| req.server_id == server_id)
                            .map(|(id, _)| id.clone())
                            .collect();

                        for req_id in requests_to_remove {
                            self.pending_mcp_requests.remove(&req_id);
                            self.tool_responses.remove(&req_id);
                        }
                    }
                }

                // Remove the server from mcp_servers list
                self.mcp_servers.retain(|s| s.id != server_id);
                println!("Spider: Removed Build Container MCP server {}", server_id);
            }

            println!("Spider: Build Container cleanup complete");
        } else {
            println!("Spider: No disconnected Build Container MCP connections to clean up");
        }
    }

    // Streaming version of chat for WebSocket clients
    async fn process_chat_request_with_streaming(
        &mut self,
        request: ChatRequest,
        channel_id: u32,
    ) -> Result<ChatResponse, String> {
        // Create a cancellation flag for this request
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.active_chat_cancellation
            .insert(channel_id, cancel_flag.clone());

        // Send initial status
        let status_msg = WsServerMessage::Status {
            status: "processing".to_string(),
            message: Some("Starting chat processing...".to_string()),
        };
        let json = serde_json::to_string(&status_msg).unwrap();
        send_ws_push(
            channel_id,
            WsMessageType::Text,
            LazyLoadBlob::new(Some("application/json"), json),
        );

        // Use the regular chat processing but send streaming updates
        let result = self.process_chat_internal(request, Some(channel_id)).await;

        // Clean up cancellation flag
        self.active_chat_cancellation.remove(&channel_id);

        // Send completion status
        let status_msg = WsServerMessage::Status {
            status: "complete".to_string(),
            message: None,
        };
        let json = serde_json::to_string(&status_msg).unwrap();
        send_ws_push(
            channel_id,
            WsMessageType::Text,
            LazyLoadBlob::new(Some("application/json"), json),
        );

        result
    }

    // Internal chat processing logic shared by HTTP and WebSocket
    async fn process_chat_internal(
        &mut self,
        request: ChatRequest,
        channel_id: Option<u32>,
    ) -> Result<ChatResponse, String> {
        // This is a refactored version of the chat logic that can send WebSocket updates
        // For now, just call the regular chat method
        // TODO: Refactor the chat method to use this shared logic

        // We can't easily call the #[http] method from here, so we'll need to duplicate the logic
        // or restructure the code. For now, let's just process it inline.

        // Validate API key (Spider key or OAuth token)
        if !self.validate_spider_key(&request.api_key) {
            return Err("Unauthorized: Invalid API key".to_string());
        }

        // Check permissions
        if !self.validate_permission(&request.api_key, "write") {
            return Err("Forbidden: API key lacks write permission".to_string());
        }

        let conversation_id = Uuid::new_v4().to_string();
        let llm_provider = request
            .llm_provider
            .unwrap_or(self.default_llm_provider.clone());

        // Determine key name for logging
        let key_name = if is_oauth_token(&request.api_key) {
            "OAuth Token".to_string()
        } else {
            self.spider_api_keys
                .iter()
                .find(|k| k.key == request.api_key)
                .map(|k| k.name.clone())
                .unwrap_or("Unknown Key".to_string())
        };

        println!(
            "Spider: Starting new conversation {} with provider {} (key: {})",
            conversation_id, llm_provider, key_name
        );

        // Get the API key for the selected provider
        let api_key = if is_oauth_token(&request.api_key) {
            // OAuth token - use it directly as the API key
            if llm_provider != "anthropic" && llm_provider != "anthropic-oauth" {
                return Err(format!(
                    "OAuth token can only be used with Anthropic provider, not {}",
                    llm_provider
                ));
            }
            request.api_key.clone()
        } else {
            // Regular Spider key - look up the provider's API key
            // For Anthropic, prefer OAuth token if available
            if llm_provider == "anthropic" {
                // First check for anthropic-oauth key (OAuth tokens stored as API keys)
                if let Some((_, oauth_key)) =
                    self.api_keys.iter().find(|(p, _)| p == "anthropic-oauth")
                {
                    let decrypted = decrypt_key(&oauth_key.key);
                    // If it's an OAuth token, use it
                    if is_oauth_token(&decrypted) {
                        decrypted
                    } else {
                        // Fall back to regular anthropic key if exists
                        self.api_keys
                            .iter()
                            .find(|(p, _)| p == "anthropic")
                            .map(|(_, k)| decrypt_key(&k.key))
                            .ok_or_else(|| {
                                format!("No API key found for provider: {}", llm_provider)
                            })?
                    }
                } else {
                    // No OAuth, try regular anthropic key
                    self.api_keys
                        .iter()
                        .find(|(p, _)| p == "anthropic")
                        .map(|(_, k)| decrypt_key(&k.key))
                        .ok_or_else(|| format!("No API key found for provider: {}", llm_provider))?
                }
            } else {
                // Non-Anthropic provider, use regular lookup
                let encrypted_key = self
                    .api_keys
                    .iter()
                    .find(|(p, _)| p == &llm_provider)
                    .map(|(_, k)| k.key.clone())
                    .ok_or_else(|| format!("No API key found for provider: {}", llm_provider))?;
                decrypt_key(&encrypted_key)
            }
        };

        // Start the agentic loop - runs indefinitely until the agent stops making tool calls
        let mut working_messages = request.messages.clone();
        let mut iteration_count = 0;

        let response = loop {
            iteration_count += 1;

            // Collect available tools from connected MCP servers - refreshed each iteration
            // This ensures newly available tools (e.g., after load-project) are immediately available
            let available_tools: Vec<Tool> = if let Some(ref mcp_server_ids) = request.mcp_servers {
                self.mcp_servers
                    .iter()
                    .filter(|s| {
                        s.connected && (
                            mcp_server_ids.contains(&s.id) ||
                            // If build_container is selected, also include ws_mcp_* servers
                            (mcp_server_ids.contains(&"build_container".to_string()) && s.id.starts_with("ws_mcp_"))
                        )
                    })
                    .flat_map(|s| s.tools.clone())
                    .collect()
            } else {
                // Use all connected servers if none specified
                self.mcp_servers
                    .iter()
                    .filter(|s| s.connected)
                    .flat_map(|s| s.tools.clone())
                    .collect()
            };

            // Check for cancellation
            if let Some(ch_id) = channel_id {
                if let Some(cancel_flag) = self.active_chat_cancellation.get(&ch_id) {
                    let is_cancelled = cancel_flag.load(Ordering::Relaxed);
                    if is_cancelled {
                        println!(
                            "Spider: Chat request cancelled at iteration {}",
                            iteration_count
                        );
                        return Err("Request cancelled by user".to_string());
                    }
                }

                // Send streaming update
                let stream_msg = WsServerMessage::Stream {
                    iteration: iteration_count,
                    message: format!("Processing iteration {}...", iteration_count),
                    tool_calls: None,
                };
                let json = serde_json::to_string(&stream_msg).unwrap();
                send_ws_push(
                    ch_id,
                    WsMessageType::Text,
                    LazyLoadBlob::new(Some("application/json"), json),
                );
            }

            // Call the LLM with available tools using the provider abstraction
            let provider = create_llm_provider(&llm_provider, &api_key);
            let llm_response = match provider
                .complete(
                    &working_messages,
                    &available_tools,
                    request.model.as_deref(),
                    self.max_tokens,
                    self.temperature,
                )
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    // Log the error for debugging
                    println!("Spider: Error calling LLM provider {}: {}", llm_provider, e);

                    // Check if it's an API key error
                    if e.contains("401") || e.contains("unauthorized") || e.contains("api key") {
                        return Err(format!(
                            "Authentication failed for {}: Please check your API key",
                            llm_provider
                        ));
                    }

                    // Check if it's a rate limit error
                    if e.contains("429") || e.contains("rate limit") {
                        return Err(format!(
                            "Rate limited by {}: Please try again later",
                            llm_provider
                        ));
                    }

                    // Return user-friendly error message
                    return Err(format!(
                        "Failed to get response from {}: {}",
                        llm_provider, e
                    ));
                }
            };

            // Check if the response contains tool calls
            println!("[DEBUG] LLM response received:");
            println!("[DEBUG]   - content: {}", llm_response.content);
            println!(
                "[DEBUG]   - has tool_calls_json: {}",
                llm_response.tool_calls_json.is_some()
            );

            if let Some(ref tool_calls_json) = llm_response.tool_calls_json {
                // The agent wants to use tools - execute them
                println!(
                    "Spider: Iteration {} - Agent requested tool calls",
                    iteration_count
                );
                println!("[DEBUG]   Tool calls JSON: {}", tool_calls_json);

                // Send streaming update for tool calls
                if let Some(ch_id) = channel_id {
                    let stream_msg = WsServerMessage::Stream {
                        iteration: iteration_count,
                        message: "Executing tool calls...".to_string(),
                        tool_calls: Some(tool_calls_json.clone()),
                    };
                    let json = serde_json::to_string(&stream_msg).unwrap();
                    send_ws_push(
                        ch_id,
                        WsMessageType::Text,
                        LazyLoadBlob::new(Some("application/json"), json),
                    );
                }

                let tool_results = self
                    .process_tool_calls(tool_calls_json, Some(conversation_id.clone()))
                    .await?;

                // Add the assistant's message with tool calls
                working_messages.push(llm_response.clone());

                // Send the assistant message with tool calls to the client
                if let Some(ch_id) = channel_id {
                    let msg_update = WsServerMessage::Message {
                        message: llm_response.clone(),
                    };
                    let json = serde_json::to_string(&msg_update).unwrap();
                    send_ws_push(
                        ch_id,
                        WsMessageType::Text,
                        LazyLoadBlob::new(Some("application/json"), json),
                    );
                }

                // Add tool results as a new message for the LLM to see
                let tool_message = Message {
                    role: "tool".to_string(),
                    content: "Tool execution results".to_string(),
                    tool_calls_json: None,
                    tool_results_json: Some(serde_json::to_string(&tool_results).unwrap()),
                    timestamp: Utc::now().timestamp() as u64,
                };
                working_messages.push(tool_message.clone());

                // Send the tool results message to the client
                if let Some(ch_id) = channel_id {
                    let msg_update = WsServerMessage::Message {
                        message: tool_message.clone(),
                    };
                    let json = serde_json::to_string(&msg_update).unwrap();
                    send_ws_push(
                        ch_id,
                        WsMessageType::Text,
                        LazyLoadBlob::new(Some("application/json"), json),
                    );
                }

                // Continue the loop - the agent will decide what to do next
                continue;
            } else {
                // No tool calls - check if the agent is actually done
                println!(
                    "Spider: Iteration {} - No tool calls, checking if agent is done",
                    iteration_count
                );

                // Check if response is just a "." - if so, continue immediately
                let completion_status = if llm_response.content.trim() == "." {
                    println!("[DEBUG] Response is just '.', treating as continue");
                    "continue".to_string()
                } else if llm_provider == "anthropic" {
                    // Use the same API key that was used for the main request
                    use crate::provider::AnthropicProvider;

                    // The api_key variable already contains the correct key for this conversation
                    let is_oauth = is_oauth_token(&api_key);
                    let anthropic_provider = AnthropicProvider::new(api_key.clone(), is_oauth);

                    anthropic_provider
                        .check_tool_loop_completion(&llm_response.content)
                        .await
                } else {
                    // For non-Anthropic providers, assume done
                    "done".to_string()
                };

                if completion_status == "continue" {
                    println!(
                        "[DEBUG] Agent indicated it wants to continue, sending continue message"
                    );

                    // Add the assistant's response to messages
                    working_messages.push(llm_response.clone());

                    // Send the assistant message to the client (but skip if it's just ".")
                    if let Some(ch_id) = channel_id {
                        if llm_response.content.trim() != "." {
                            let msg_update = WsServerMessage::Message {
                                message: llm_response.clone(),
                            };
                            let json = serde_json::to_string(&msg_update).unwrap();
                            send_ws_push(
                                ch_id,
                                WsMessageType::Text,
                                LazyLoadBlob::new(Some("application/json"), json),
                            );
                        }
                    }

                    // Add a continue message and loop
                    let continue_message = Message {
                        role: "user".to_string(),
                        content: "continue".to_string(),
                        tool_calls_json: None,
                        tool_results_json: None,
                        timestamp: Utc::now().timestamp() as u64,
                    };
                    working_messages.push(continue_message);

                    // Continue the loop
                    continue;
                } else {
                    // Agent is done (or error/failed to parse)
                    println!(
                        "Spider: Iteration {} - Agent provided final response (completion check: {})",
                        iteration_count, completion_status
                    );

                    // Send the final assistant message to the client (but skip if it's just ".")
                    if let Some(ch_id) = channel_id {
                        if llm_response.content.trim() != "." {
                            let msg_update = WsServerMessage::Message {
                                message: llm_response.clone(),
                            };
                            let json = serde_json::to_string(&msg_update).unwrap();
                            send_ws_push(
                                ch_id,
                                WsMessageType::Text,
                                LazyLoadBlob::new(Some("application/json"), json),
                            );
                        }
                    }

                    break llm_response;
                }
            }
        };

        // Add the final response to messages
        working_messages.push(response.clone());

        let metadata = request.metadata.unwrap_or(ConversationMetadata {
            start_time: Utc::now().to_rfc3339(),
            client: "unknown".to_string(),
            from_stt: false,
        });

        // Get only the new messages that were added during this chat session
        // (everything after the initial user messages)
        let initial_message_count = request.messages.len();
        let new_messages = working_messages[initial_message_count..].to_vec();

        // Gather MCP server details for the conversation
        let mcp_server_ids = request.mcp_servers.clone().unwrap_or_default();
        let mcp_servers_details: Vec<McpServerDetails> = mcp_server_ids
            .iter()
            .filter_map(|server_id| {
                self.mcp_servers
                    .iter()
                    .find(|s| &s.id == server_id)
                    .map(|server| McpServerDetails {
                        id: server.id.clone(),
                        name: server.name.clone(),
                        tools: server
                            .tools
                            .iter()
                            .map(|tool| McpToolInfo {
                                name: tool.name.clone(),
                                description: tool.description.clone(),
                            })
                            .collect(),
                    })
            })
            .collect();

        let conversation = Conversation {
            id: conversation_id.clone(),
            messages: working_messages,
            metadata,
            llm_provider,
            mcp_servers: mcp_server_ids,
            mcp_servers_details: if mcp_servers_details.is_empty() {
                None
            } else {
                Some(mcp_servers_details)
            },
        };

        // Save to VFS
        if let Err(e) = save_conversation_to_vfs(&conversation).await {
            println!("Warning: Failed to save conversation to VFS: {}", e);
        }

        // Keep in memory for quick access
        self.active_conversations
            .push((conversation_id.clone(), conversation));

        Ok(ChatResponse {
            conversation_id,
            response,
            all_messages: new_messages,
        })
    }

    fn handle_mcp_message(&mut self, channel_id: u32, message: Value) {
        println!(
            "Spider: handle_mcp_message received on channel {}: {:?}",
            channel_id, message
        );

        // Find the connection for this channel
        let conn = match self.ws_connections.get(&channel_id) {
            Some(c) => c.clone(),
            None => {
                println!(
                    "Spider: Received MCP message for unknown channel {}",
                    channel_id
                );
                return;
            }
        };

        // Check if this is a response to a pending request
        if let Some(id) = message.get("id").and_then(|v| v.as_str()) {
            println!("Spider: Message has id: {}", id);

            // Check if this is a spider/* method response (not in pending_mcp_requests)
            // These are direct responses to spider/* methods like load-project, auth, etc.
            if id.starts_with("load-project-")
                || id.starts_with("start-package-")
                || id.starts_with("persist")
                || id.starts_with("auth_")
            {
                println!("Spider: Handling spider/* method response with id: {}", id);
                // Store the response for the waiting execute_*_impl method
                let result = if let Some(result_value) = message.get("result") {
                    result_value.clone()
                } else if let Some(error) = message.get("error") {
                    serde_json::to_value(ErrorResponse {
                        error: error.clone(),
                    })
                    .unwrap_or_else(|_| Value::Null)
                } else {
                    serde_json::to_value(ErrorResponse {
                        error: Value::String("Invalid response format".to_string()),
                    })
                    .unwrap_or_else(|_| Value::Null)
                };
                self.tool_responses.insert(id.to_string(), result);
                println!("Spider: Stored response for id {} in tool_responses", id);
                return;
            }

            if let Some(pending) = self.pending_mcp_requests.remove(id) {
                println!("Spider: Found pending request for id: {}", id);
                match pending.request_type {
                    McpRequestType::Initialize => {
                        self.handle_initialize_response(channel_id, &conn, &message);
                    }
                    McpRequestType::ToolsList => {
                        self.handle_tools_list_response(channel_id, &conn, &message);
                    }
                    McpRequestType::ToolCall { tool_name: _ } => {
                        self.handle_tool_call_response(&pending, &message);
                    }
                }
            } else {
                println!("Spider: No pending request found for id: {}", id);
            }
        }

        // Handle notifications or other messages
        if let Some(method) = message.get("method").and_then(|v| v.as_str()) {
            match method {
                "tools/list_changed" => {
                    // Tools have changed, re-fetch them
                    self.request_tools_list(channel_id);
                }
                _ => {
                    println!("Spider: Received MCP notification: {}", method);
                }
            }
        }
    }

    fn handle_initialize_response(
        &mut self,
        channel_id: u32,
        conn: &WsConnection,
        message: &Value,
    ) {
        if let Some(_result) = message.get("result") {
            println!(
                "Spider: MCP server {} initialized successfully",
                conn.server_name
            );

            // Mark connection as initialized
            if let Some(ws_conn) = self.ws_connections.get_mut(&channel_id) {
                ws_conn.initialized = true;
            }

            // Send notifications/initialized
            let notif = JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "notifications/initialized".to_string(),
                params: None,
            };
            let blob = LazyLoadBlob::new(
                Some("application/json"),
                serde_json::to_string(&notif).unwrap().into_bytes(),
            );
            send_ws_client_push(channel_id, WsMessageType::Text, blob);

            // Request tools list
            self.request_tools_list(channel_id);
        } else if let Some(error) = message.get("error") {
            println!(
                "Spider: Failed to initialize MCP server {}: {:?}",
                conn.server_name, error
            );
        }
    }

    fn request_tools_list(&mut self, channel_id: u32) {
        let request_id = format!("tools_{}", channel_id);
        let tools_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: request_id.clone(),
        };

        // Store pending request
        if let Some(conn) = self.ws_connections.get(&channel_id) {
            self.pending_mcp_requests.insert(
                request_id.clone(),
                PendingMcpRequest {
                    request_id,
                    conversation_id: None,
                    server_id: conn.server_id.clone(),
                    request_type: McpRequestType::ToolsList,
                },
            );
        }

        let blob = LazyLoadBlob::new(
            Some("application/json"),
            serde_json::to_string(&tools_request).unwrap().into_bytes(),
        );
        send_ws_client_push(channel_id, WsMessageType::Text, blob);
    }

    fn handle_tools_list_response(
        &mut self,
        channel_id: u32,
        conn: &WsConnection,
        message: &Value,
    ) {
        if let Some(result) = message.get("result") {
            if let Some(tools_json) = result.get("tools").and_then(|v| v.as_array()) {
                let mut tools = Vec::new();

                for tool_json in tools_json {
                    if let (Some(name), Some(description)) = (
                        tool_json.get("name").and_then(|v| v.as_str()),
                        tool_json.get("description").and_then(|v| v.as_str()),
                    ) {
                        // Store both the old parameters format and the new inputSchema
                        let parameters = tool_json
                            .get("parameters")
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "{}".to_string());

                        // Store the complete inputSchema if available as a JSON string
                        let input_schema_json = tool_json
                            .get("inputSchema")
                            .map(|schema| schema.to_string());

                        tools.push(Tool {
                            name: name.to_string(),
                            description: description.to_string(),
                            parameters,
                            input_schema_json,
                        });
                    }
                }

                let tool_count = tools.len();
                println!(
                    "Spider: Received {} tools from MCP server {}",
                    tool_count, conn.server_name
                );

                // Update connection with tools
                if let Some(ws_conn) = self.ws_connections.get_mut(&channel_id) {
                    ws_conn.tools = tools.clone();
                }

                // For build container connections, we need special handling
                if conn.server_id == "build_container_self_hosted"
                    || conn.server_id.starts_with("build_container_")
                {
                    // Create or update a separate ws-mcp server entry for the remote tools
                    let ws_mcp_server_id = format!("ws_mcp_{}", channel_id);

                    // Check if this ws-mcp server already exists
                    if let Some(server) = self
                        .mcp_servers
                        .iter_mut()
                        .find(|s| s.id == ws_mcp_server_id)
                    {
                        server.tools = tools;
                        server.connected = true;
                        println!(
                            "Spider: Updated ws-mcp server {} with {} tools",
                            ws_mcp_server_id,
                            server.tools.len()
                        );
                    } else {
                        // Create a new MCP server entry for ws-mcp tools
                        let ws_mcp_server = McpServer {
                            id: ws_mcp_server_id.clone(),
                            name: "Build Container MCP".to_string(),
                            transport: crate::types::TransportConfig {
                                transport_type: "websocket".to_string(),
                                command: None,
                                args: None,
                                url: Some(self.build_container_ws_uri.clone()),
                                hypergrid_token: None,
                                hypergrid_client_id: None,
                                hypergrid_node: None,
                            },
                            tools,
                            connected: true,
                        };
                        self.mcp_servers.push(ws_mcp_server);
                        println!(
                            "Spider: Created new ws-mcp server {} with {} tools",
                            ws_mcp_server_id, tool_count
                        );
                    }

                    // Make sure the build_container server retains its native tools
                    // by refreshing them from the tool provider
                    if let Some(provider) = self
                        .tool_provider_registry
                        .find_provider_for_tool("load-project", self)
                    {
                        let native_tools = provider.get_tools(self);
                        if let Some(server) = self
                            .mcp_servers
                            .iter_mut()
                            .find(|s| s.id == "build_container")
                        {
                            server.tools = native_tools;
                            server.connected = true;
                            println!(
                                "Spider: Refreshed build_container server with {} native tools",
                                server.tools.len()
                            );
                        }
                    }
                } else {
                    // For non-build-container connections, update normally
                    if let Some(server) =
                        self.mcp_servers.iter_mut().find(|s| s.id == conn.server_id)
                    {
                        server.tools = tools;
                        server.connected = true;
                        println!(
                            "Spider: Updated MCP server {} with {} tools",
                            conn.server_id,
                            server.tools.len()
                        );
                    } else {
                        println!(
                            "Spider: Warning - could not find MCP server with id {}",
                            conn.server_id
                        );
                    }
                }
            }
        } else if let Some(error) = message.get("error") {
            println!(
                "Spider: Failed to get tools from MCP server {}: {:?}",
                conn.server_name, error
            );
        }
    }

    fn handle_tool_call_response(&mut self, pending: &PendingMcpRequest, message: &Value) {
        println!(
            "Spider: Received tool call response for request {}: {:?}",
            pending.request_id, message
        );

        // Store the response so execute_mcp_tool can retrieve it
        let result = if let Some(result_value) = message.get("result") {
            result_value.clone()
        } else if let Some(error) = message.get("error") {
            serde_json::to_value(ErrorResponse {
                error: error.clone(),
            })
            .unwrap_or_else(|_| Value::Null)
        } else {
            serde_json::to_value(ErrorResponse {
                error: Value::String("Invalid MCP response format".to_string()),
            })
            .unwrap_or_else(|_| Value::Null)
        };

        self.tool_responses
            .insert(pending.request_id.clone(), result);
    }

    async fn execute_mcp_tool(
        &mut self,
        server_id: &str,
        tool_name: &str,
        parameters: &Value,
        conversation_id: Option<String>,
    ) -> Result<Value, String> {
        println!(
            "[DEBUG] execute_mcp_tool called with server_id: {}, tool_name: {}",
            server_id, tool_name
        );
        println!("[DEBUG]   parameters: {}", parameters);
        println!(
            "Spider: Available MCP servers: {:?}",
            self.mcp_servers
                .iter()
                .map(|s| (&s.id, s.connected))
                .collect::<Vec<_>>()
        );

        // Special handling for ws_mcp servers (build container WebSocket connections)
        if server_id.starts_with("ws_mcp_") {
            // Extract channel_id from server_id (format: "ws_mcp_{channel_id}")
            let channel_id = server_id
                .strip_prefix("ws_mcp_")
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| format!("Invalid ws_mcp server id: {}", server_id))?;

            println!(
                "Spider: Looking for WebSocket connection with channel_id {} for server {}",
                channel_id, server_id
            );
            println!(
                "Spider: Available ws_connections: {:?}",
                self.ws_connections.keys().collect::<Vec<_>>()
            );

            // Verify the connection exists
            if !self.ws_connections.contains_key(&channel_id) {
                return Err(format!(
                    "No WebSocket connection found for server {}",
                    server_id
                ));
            }

            // Execute via WebSocket using MCP protocol
            let request_id = format!("tool_{}_{}", channel_id, Uuid::new_v4());
            let tool_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "tools/call".to_string(),
                params: Some(
                    serde_json::to_value(McpToolCallParams {
                        name: tool_name.to_string(),
                        arguments: parameters.clone(),
                    })
                    .unwrap(),
                ),
                id: request_id.clone(),
            };

            // Store pending request
            self.pending_mcp_requests.insert(
                request_id.clone(),
                PendingMcpRequest {
                    request_id: request_id.clone(),
                    conversation_id,
                    server_id: server_id.to_string(),
                    request_type: McpRequestType::ToolCall {
                        tool_name: tool_name.to_string(),
                    },
                },
            );

            // Send the request
            let request_json = serde_json::to_string(&tool_request).unwrap();
            let blob = LazyLoadBlob::new(Some("application/json"), request_json.into_bytes());
            send_ws_client_push(channel_id, WsMessageType::Text, blob);

            // Wait for response
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(30);

            loop {
                if start.elapsed() > timeout {
                    self.pending_mcp_requests.remove(&request_id);
                    return Err(format!("Tool call timed out: {}", tool_name));
                }

                if let Some(result) = self.tool_responses.remove(&request_id) {
                    // Parse the MCP result format
                    if let Some(content) = result.get("content") {
                        return Ok(serde_json::to_value(ToolExecutionResult {
                            result: content.clone(),
                            success: true,
                        })
                        .unwrap());
                    } else if let Some(error) = result.get("error") {
                        return Err(format!("Tool execution failed: {}", error));
                    } else {
                        // Fallback: return the raw result wrapped in ToolExecutionResult
                        return Ok(serde_json::to_value(ToolExecutionResult {
                            result: result,
                            success: true,
                        })
                        .unwrap());
                    }
                }

                // Sleep briefly before checking again
                let _ = hyperware_process_lib::hyperapp::sleep(100).await;
            }
        }

        // Regular MCP server handling
        let server = self
            .mcp_servers
            .iter()
            .find(|s| s.id == server_id && s.connected)
            .ok_or_else(|| format!("MCP server {} not found or not connected", server_id))?;

        // Check if the tool exists
        let _tool = server
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| format!("Tool {} not found on server {}", tool_name, server_id))?;

        // Execute the tool based on transport type
        match server.transport.transport_type.as_str() {
            "hypergrid" => {
                // Use the hypergrid tool provider
                if let Some(provider) = self
                    .tool_provider_registry
                    .find_provider_for_tool(tool_name, self)
                {
                    let command = provider.prepare_execution(tool_name, parameters, self)?;
                    self.execute_tool_command(command, conversation_id).await
                } else {
                    // Map old tool names to new ones for backward compatibility
                    let normalized_tool_name = match tool_name {
                        "authorize" => "hypergrid_authorize",
                        "search-registry" => "hypergrid_search",
                        "call-provider" => "hypergrid_call",
                        name => name,
                    };

                    // Try with normalized name
                    if let Some(provider) = self
                        .tool_provider_registry
                        .find_provider_for_tool(normalized_tool_name, self)
                    {
                        let command =
                            provider.prepare_execution(normalized_tool_name, parameters, self)?;
                        self.execute_tool_command(command, conversation_id).await
                    } else {
                        // Fall back to old implementation for backward compatibility
                        match normalized_tool_name {
                            "hypergrid_authorize" => {
                                println!(
                                    "Spider: hypergrid_authorize called for server_id: {}",
                                    server_id
                                );
                                println!("  Parameters received: {:?}", parameters);

                                // Update hypergrid credentials
                                let new_url = parameters
                                    .get("url")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing url parameter".to_string())?;
                                let new_token = parameters
                                    .get("token")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing token parameter".to_string())?;
                                let new_client_id = parameters
                                    .get("client_id")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing client_id parameter".to_string())?;
                                let new_node = parameters
                                    .get("node")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing node parameter".to_string())?;

                                println!("Spider: Authorizing hypergrid with:");
                                println!("  - URL: {}", new_url);
                                println!("  - Token: {}...", &new_token[..new_token.len().min(20)]);
                                println!("  - Client ID: {}", new_client_id);
                                println!("  - Node: {}", new_node);

                                // Test new connection
                                println!("Spider: Testing hypergrid connection...");
                                self.test_hypergrid_connection(new_url, new_token, new_client_id)
                                    .await?;
                                println!("Spider: Connection test successful!");

                                // Create or update the hypergrid connection
                                let hypergrid_conn = HypergridConnection {
                                    server_id: server_id.to_string(),
                                    url: new_url.to_string(),
                                    token: new_token.to_string(),
                                    client_id: new_client_id.to_string(),
                                    node: new_node.to_string(),
                                    last_retry: Instant::now(),
                                    retry_count: 0,
                                    connected: true,
                                };

                                self.hypergrid_connections
                                    .insert(server_id.to_string(), hypergrid_conn);
                                println!("Spider: Stored hypergrid connection in memory");

                                // Update transport config
                                if let Some(server) =
                                    self.mcp_servers.iter_mut().find(|s| s.id == server_id)
                                {
                                    println!(
                                        "Spider: Updating server '{}' transport config",
                                        server.name
                                    );
                                    server.transport.url = Some(new_url.to_string());
                                    server.transport.hypergrid_token = Some(new_token.to_string());
                                    server.transport.hypergrid_client_id =
                                        Some(new_client_id.to_string());
                                    server.transport.hypergrid_node = Some(new_node.to_string());
                                    println!(
                                        "Spider: Server transport config updated successfully"
                                    );
                                    println!(
                                        "Spider: State should auto-save due to SaveOptions::OnDiff"
                                    );
                                } else {
                                    println!(
                                        "Spider: WARNING - Could not find server with id: {}",
                                        server_id
                                    );
                                }

                                Ok(serde_json::to_value(ToolResponseContent {
                                    content: vec![ToolResponseContentItem {
                                        content_type: "text".to_string(),
                                        text: format!("✅ Successfully authorized! Hypergrid is now configured with:\n- Node: {}\n- Client ID: {}\n- URL: {}", new_node, new_client_id, new_url),
                                    }],
                                })
                                .map_err(|e| format!("Failed to serialize response: {}", e))?)
                            }
                            "hypergrid_search" => {
                                // Check if configured
                                let hypergrid_conn = self.hypergrid_connections.get(server_id)
                            .ok_or_else(|| "Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string())?;
                                let query = parameters
                                    .get("query")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing query parameter".to_string())?;

                                let response = self
                                    .call_hypergrid_api(
                                        &hypergrid_conn.url,
                                        &hypergrid_conn.token,
                                        &hypergrid_conn.client_id,
                                        &HypergridMessage {
                                            request: HypergridMessageType::SearchRegistry(
                                                query.to_string(),
                                            ),
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
                            "hypergrid_call" => {
                                // Check if configured
                                let hypergrid_conn = self.hypergrid_connections.get(server_id)
                            .ok_or_else(|| "Hypergrid not configured. Please use hypergrid_authorize first with your credentials.".to_string())?;
                                let provider_id = parameters
                                    .get("providerId")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing providerId parameter".to_string())?;
                                let provider_name = parameters
                                    .get("providerName")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| "Missing providerName parameter".to_string())?;
                                // Support both "callArgs" (old) and "arguments" (new) parameter names
                                let call_args = parameters
                                    .get("arguments")
                                    .or_else(|| parameters.get("callArgs"))
                                    .and_then(|v| v.as_array())
                                    .ok_or_else(|| "Missing arguments parameter".to_string())?;

                                // Convert callArgs to Vec<(String, String)>
                                let mut arguments = Vec::new();
                                for arg in call_args {
                                    if let Some(pair) = arg.as_array() {
                                        if pair.len() == 2 {
                                            if let (Some(key), Some(val)) =
                                                (pair[0].as_str(), pair[1].as_str())
                                            {
                                                arguments.push((key.to_string(), val.to_string()));
                                            }
                                        }
                                    }
                                }

                                let response = self
                                    .call_hypergrid_api(
                                        &hypergrid_conn.url,
                                        &hypergrid_conn.token,
                                        &hypergrid_conn.client_id,
                                        &HypergridMessage {
                                            request: HypergridMessageType::CallProvider {
                                                provider_id: provider_id.to_string(),
                                                provider_name: provider_name.to_string(),
                                                arguments,
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
                            _ => Err(format!("Unknown hypergrid tool: {}", tool_name)),
                        }
                    }
                }
            }
            "build_container" => {
                // Native build container tools are handled by the tool provider
                if let Some(provider) = self
                    .tool_provider_registry
                    .find_provider_for_tool(tool_name, self)
                {
                    let command = provider.prepare_execution(tool_name, parameters, self)?;
                    self.execute_tool_command(command, conversation_id).await
                } else {
                    Err(format!("Unknown build container tool: {}", tool_name))
                }
            }
            "stdio" | "websocket" => {
                // Find the WebSocket connection for this server
                let channel_id = self
                    .ws_connections
                    .iter()
                    .find(|(_, conn)| conn.server_id == server_id)
                    .map(|(id, _)| *id)
                    .ok_or_else(|| {
                        format!("No WebSocket connection found for server {}", server_id)
                    })?;

                // Execute via WebSocket
                let request_id = format!("tool_{}_{}", channel_id, Uuid::new_v4());

                let tool_request = JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    method: "tools/call".to_string(),
                    params: Some(
                        serde_json::to_value(McpToolCallParams {
                            name: tool_name.to_string(),
                            arguments: parameters.clone(),
                        })
                        .unwrap(),
                    ),
                    id: request_id.clone(),
                };

                // Store pending request
                self.pending_mcp_requests.insert(
                    request_id.clone(),
                    PendingMcpRequest {
                        request_id: request_id.clone(),
                        conversation_id: conversation_id.clone(),
                        server_id: server_id.to_string(),
                        request_type: McpRequestType::ToolCall {
                            tool_name: tool_name.to_string(),
                        },
                    },
                );

                // Send the tool call to MCP server
                println!(
                    "Spider: Sending tool call {} to MCP server {} with request_id {}",
                    tool_name, server_id, request_id
                );
                let blob = LazyLoadBlob::new(
                    Some("application/json"),
                    serde_json::to_string(&tool_request).unwrap().into_bytes(),
                );
                send_ws_client_push(channel_id, WsMessageType::Text, blob);

                // Wait for response with async polling
                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(60);

                loop {
                    // Check if we have a response
                    if let Some(response) = self.tool_responses.remove(&request_id) {
                        self.pending_mcp_requests.remove(&request_id);

                        println!("[DEBUG] Tool response received:");
                        println!("[DEBUG]   - response: {}", response);

                        // Parse the MCP result
                        if let Some(content) = response.get("content") {
                            let result = serde_json::to_value(ToolExecutionResult {
                                result: content.clone(),
                                success: true,
                            })
                            .unwrap();
                            println!("[DEBUG]   - returning content result: {}", result);
                            return Ok(result);
                        } else {
                            println!("[DEBUG]   - returning full response: {}", response);
                            return Ok(response);
                        }
                    }

                    // Check timeout
                    if start.elapsed() > timeout {
                        self.pending_mcp_requests.remove(&request_id);
                        return Err(format!(
                            "Tool call {} timed out after 60 seconds",
                            tool_name
                        ));
                    }

                    // Sleep briefly to yield to other tasks
                    // This allows the event loop to process incoming messages
                    let _ = hyperware_process_lib::hyperapp::sleep(100).await;
                }
            }
            "http" => {
                // Execute via HTTP
                // This is a placeholder - actual implementation would make HTTP requests
                Ok(serde_json::to_value(ToolExecutionResult {
                    result: Value::String(format!(
                        "HTTP execution of {} with params: {}",
                        tool_name, parameters
                    )),
                    success: true,
                })
                .unwrap())
            }
            _ => Err(format!(
                "Unsupported transport type: {}",
                server.transport.transport_type
            )),
        }
    }

    async fn process_tool_calls(
        &mut self,
        tool_calls_json: &str,
        conversation_id: Option<String>,
    ) -> Result<Vec<ToolResult>, String> {
        println!("[DEBUG] process_tool_calls called");
        println!("[DEBUG]   tool_calls_json: {}", tool_calls_json);

        let tool_calls: Vec<ToolCall> = serde_json::from_str(tool_calls_json)
            .map_err(|e| format!("Failed to parse tool calls: {}", e))?;

        println!("[DEBUG]   Parsed {} tool calls", tool_calls.len());
        let mut results = Vec::new();

        for tool_call in tool_calls {
            println!("[DEBUG]   Processing tool call:");
            println!("[DEBUG]     - id: {}", tool_call.id);
            println!("[DEBUG]     - tool_name: {}", tool_call.tool_name);
            println!("[DEBUG]     - parameters: {}", tool_call.parameters);
            // Find which MCP server has this tool and get its ID
            let server_id = self
                .mcp_servers
                .iter()
                .find(|s| s.connected && s.tools.iter().any(|t| t.name == tool_call.tool_name))
                .map(|s| s.id.clone());

            let result = if let Some(server_id) = server_id {
                println!("[DEBUG]     Found tool in server: {}", server_id);
                let params: Value = serde_json::from_str(&tool_call.parameters)
                    .unwrap_or(Value::Object(serde_json::Map::new()));
                match self
                    .execute_mcp_tool(
                        &server_id,
                        &tool_call.tool_name,
                        &params,
                        conversation_id.clone(),
                    )
                    .await
                {
                    Ok(res) => {
                        let result_str = res.to_string();
                        println!("[DEBUG]     Tool execution successful: {}", result_str);
                        result_str
                    }
                    Err(e) => {
                        let error_str = format!(r#"{{"error":"{}"}}"#, e);
                        println!("[DEBUG]     Tool execution error: {}", error_str);
                        error_str
                    }
                }
            } else {
                let error_str = format!(
                    r#"{{"error":"Tool {} not found in any connected MCP server"}}"#,
                    tool_call.tool_name
                );
                println!("[DEBUG]     {}", error_str);
                error_str
            };

            results.push(ToolResult {
                tool_call_id: tool_call.id,
                result,
            });
        }

        println!("[DEBUG]   Returning {} tool results", results.len());
        Ok(results)
    }

    async fn test_hypergrid_connection(
        &self,
        url: &str,
        token: &str,
        client_id: &str,
    ) -> Result<String, String> {
        // Test the hypergrid connection with a simple search request
        let test_message = HypergridMessage {
            request: HypergridMessageType::SearchRegistry("test".to_string()),
        };

        let body = serde_json::to_string(&test_message)
            .map_err(|e| format!("Failed to serialize test message: {}", e))?;

        // Make HTTP request to test the connection
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;

        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Client-ID".to_string(), client_id.to_string());
        headers.insert("X-Token".to_string(), token.to_string());

        let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

        let response = send_request_await_response(
            Method::POST,
            parsed_url,
            Some(headers),
            30000, // 30 second timeout
            body.into_bytes(),
        )
        .await
        .map_err(|e| format!("Failed to test hypergrid connection: {:?}", e))?;

        // Check if response is successful (status 200 or 404 for search not found)
        let status_code = response.status().as_u16();
        if status_code != 200 && status_code != 404 {
            return Err(format!(
                "Hypergrid connection test failed with status: {}",
                status_code
            ));
        }

        Ok("Connection test successful".to_string())
    }

    async fn call_hypergrid_api(
        &self,
        url: &str,
        token: &str,
        client_id: &str,
        message: &HypergridMessage,
    ) -> Result<String, String> {
        let body = serde_json::to_string(message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;

        println!("Spider: Calling hypergrid API with message: {}", body);

        // Make HTTP request
        use hyperware_process_lib::http::client::send_request_await_response;
        use hyperware_process_lib::http::Method;

        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Client-ID".to_string(), client_id.to_string());
        headers.insert("X-Token".to_string(), token.to_string());

        let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

        let response = send_request_await_response(
            Method::POST,
            parsed_url,
            Some(headers),
            60000, // 60 second timeout for actual calls
            body.into_bytes(),
        )
        .await
        .map_err(|e| format!("Failed to call hypergrid API: {:?}", e))?;

        // Convert response body to string
        let response_text = String::from_utf8(response.body().to_vec())
            .unwrap_or_else(|_| "Invalid UTF-8 response".to_string());

        let status_code = response.status().as_u16();
        println!(
            "Spider: Hypergrid API response (status {}): {}",
            status_code, response_text
        );

        if status_code >= 400 {
            return Err(format!(
                "Hypergrid API error (status {}): {}",
                status_code, response_text
            ));
        }

        Ok(response_text)
    }

    // Execute tool commands returned by tool providers
    async fn execute_tool_command(
        &mut self,
        command: tool_providers::ToolExecutionCommand,
        _conversation_id: Option<String>,
    ) -> Result<Value, String> {
        use tool_providers::ToolExecutionCommand;

        match command {
            ToolExecutionCommand::InitBuildContainer { metadata } => {
                self.execute_init_build_container_impl(metadata).await
            }
            ToolExecutionCommand::LoadProject {
                project_uuid,
                name,
                initial_zip,
                channel_id,
            } => {
                self.execute_load_project_impl(project_uuid, name, initial_zip, channel_id)
                    .await
            }
            ToolExecutionCommand::StartPackage {
                channel_id,
                package_dir,
            } => {
                self.execute_start_package_impl(channel_id, package_dir)
                    .await
            }
            ToolExecutionCommand::Persist {
                channel_id,
                directories,
            } => self.execute_persist_impl(channel_id, directories).await,
            ToolExecutionCommand::GetProjects => {
                // Return the project name to UUID mapping as JSON
                Ok(serde_json::to_value(&self.project_name_to_uuids)
                    .map_err(|e| format!("Failed to serialize project mapping: {}", e))?)
            }
            ToolExecutionCommand::DoneBuildContainer {
                metadata,
                channel_id,
            } => {
                self.execute_done_build_container_impl(metadata, channel_id)
                    .await
            }
            ToolExecutionCommand::HypergridAuthorize {
                server_id,
                url,
                token,
                client_id,
                node,
                name,
            } => {
                self.execute_hypergrid_authorize_impl(server_id, url, token, client_id, node, name)
                    .await
            }
            ToolExecutionCommand::HypergridSearch { server_id, query } => {
                self.execute_hypergrid_search_impl(server_id, query).await
            }
            ToolExecutionCommand::HypergridCall {
                server_id,
                provider_id,
                provider_name,
                call_args,
            } => {
                self.execute_hypergrid_call_impl(server_id, provider_id, provider_name, call_args)
                    .await
            }
            ToolExecutionCommand::DirectResult(result) => result,
        }
    }
}
