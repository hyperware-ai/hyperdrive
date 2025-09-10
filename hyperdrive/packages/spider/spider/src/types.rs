use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_providers::ToolProviderRegistry;

#[derive(Default, Serialize, Deserialize)]
pub struct SpiderState {
    pub api_keys: Vec<(String, ApiKey)>,
    pub spider_api_keys: Vec<SpiderApiKey>,
    pub mcp_servers: Vec<McpServer>,
    pub active_conversations: Vec<(String, Conversation)>,
    pub default_llm_provider: String,
    pub max_tokens: u32,
    pub temperature: f32,
    #[serde(skip)]
    pub ws_connections: HashMap<u32, WsConnection>, // channel_id -> connection info
    #[serde(skip)]
    pub pending_mcp_requests: HashMap<String, PendingMcpRequest>, // request_id -> pending request
    #[serde(skip)]
    pub tool_responses: HashMap<String, Value>, // request_id -> response received from MCP
    #[serde(skip)]
    pub next_channel_id: u32,
    #[serde(skip)]
    pub chat_clients: HashMap<u32, ChatClient>, // channel_id -> chat client connection
    #[serde(skip)]
    pub active_chat_cancellation: HashMap<u32, Arc<AtomicBool>>, // channel_id -> cancellation flag
    #[serde(skip)]
    pub hypergrid_connections: HashMap<String, HypergridConnection>, // server_id -> hypergrid connection
    #[serde(skip)]
    pub show_trial_key_notification: bool, // Flag to show trial key notification popup
    #[serde(skip)]
    pub tool_provider_registry: ToolProviderRegistry, // Registry for modular tool providers
}

#[derive(Clone, Debug)]
pub(crate) struct WsConnection {
    pub(crate) server_id: String,
    pub(crate) server_name: String,
    pub(crate) channel_id: u32,
    pub(crate) tools: Vec<Tool>,
    pub(crate) initialized: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingMcpRequest {
    pub(crate) request_id: String,
    pub(crate) conversation_id: Option<String>,
    pub(crate) server_id: String,
    pub(crate) request_type: McpRequestType,
}

#[derive(Clone, Debug)]
pub(crate) enum McpRequestType {
    Initialize,
    ToolsList,
    ToolCall { tool_name: String },
}

#[derive(Clone, Debug)]
pub(crate) struct ChatClient {
    pub(crate) channel_id: u32,
    pub(crate) api_key: String,
    pub(crate) conversation_id: Option<String>,
    pub(crate) connected_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ApiKey {
    pub(crate) provider: String,
    pub(crate) key: String,
    pub(crate) created_at: u64,
    pub(crate) last_used: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ApiKeyInfo {
    pub(crate) provider: String,
    #[serde(rename = "createdAt")]
    pub(crate) created_at: u64,
    #[serde(rename = "lastUsed")]
    pub(crate) last_used: Option<u64>,
    #[serde(rename = "keyPreview")]
    pub(crate) key_preview: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct SpiderApiKey {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) permissions: Vec<String>,
    #[serde(rename = "createdAt")]
    pub(crate) created_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct McpServer {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) transport: TransportConfig,
    pub(crate) tools: Vec<Tool>,
    pub(crate) connected: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TransportConfig {
    #[serde(rename = "transportType")]
    pub(crate) transport_type: String, // "stdio", "http", "websocket", or "hypergrid"
    pub(crate) command: Option<String>,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) url: Option<String>,
    // Hypergrid-specific fields
    #[serde(rename = "hypergridToken")]
    pub(crate) hypergrid_token: Option<String>,
    #[serde(rename = "hypergridClientId")]
    pub(crate) hypergrid_client_id: Option<String>,
    #[serde(rename = "hypergridNode")]
    pub(crate) hypergrid_node: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Tool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: String, // Deprecated: use input_schema_json instead
    #[serde(rename = "inputSchema")]
    pub(crate) input_schema_json: Option<String>, // Complete JSON schema as string including $defs, annotations, etc.
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Conversation {
    pub(crate) id: String,
    pub(crate) messages: Vec<Message>,
    pub(crate) metadata: ConversationMetadata,
    #[serde(rename = "llmProvider")]
    pub(crate) llm_provider: String,
    #[serde(rename = "mcpServers")]
    pub(crate) mcp_servers: Vec<String>,
    #[serde(rename = "mcpServersDetails", skip_serializing_if = "Option::is_none")]
    pub(crate) mcp_servers_details: Option<Vec<McpServerDetails>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct McpServerDetails {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tools: Vec<McpToolInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct McpToolInfo {
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ConversationMetadata {
    #[serde(rename = "startTime")]
    pub(crate) start_time: String,
    pub(crate) client: String,
    #[serde(rename = "fromStt")]
    pub(crate) from_stt: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Message {
    pub(crate) role: String,
    pub(crate) content: String,
    #[serde(rename = "toolCallsJson")]
    pub(crate) tool_calls_json: Option<String>, // JSON string of tool calls
    #[serde(rename = "toolResultsJson")]
    pub(crate) tool_results_json: Option<String>, // JSON string of tool results
    pub(crate) timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ToolCall {
    pub(crate) id: String,
    pub(crate) tool_name: String,
    pub(crate) parameters: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ToolResult {
    pub(crate) tool_call_id: String,
    pub(crate) result: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct SetApiKeyRequest {
    pub(crate) provider: String,
    pub(crate) key: String,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct CreateSpiderKeyRequest {
    pub(crate) name: String,
    pub(crate) permissions: Vec<String>,
    #[serde(rename = "adminKey")]
    pub(crate) admin_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ListSpiderKeysRequest {
    #[serde(rename = "adminKey")]
    pub(crate) admin_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct RevokeSpiderKeyRequest {
    #[serde(rename = "keyId")]
    pub(crate) key_id: String,
    #[serde(rename = "adminKey")]
    pub(crate) admin_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ListApiKeysRequest {
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct RemoveApiKeyRequest {
    pub(crate) provider: String,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ConnectMcpServerRequest {
    #[serde(rename = "serverId")]
    pub(crate) server_id: String,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct DisconnectMcpServerRequest {
    #[serde(rename = "serverId")]
    pub(crate) server_id: String,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct RemoveMcpServerRequest {
    #[serde(rename = "serverId")]
    pub(crate) server_id: String,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ListMcpServersRequest {
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct GetConversationRequest {
    #[serde(rename = "conversationId")]
    pub(crate) conversation_id: String,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct GetConfigRequest {
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct AddMcpServerRequest {
    pub(crate) name: String,
    pub(crate) transport: TransportConfig,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ListConversationsRequest {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) client: Option<String>,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct UpdateConfigRequest {
    #[serde(rename = "defaultLlmProvider")]
    pub(crate) default_llm_provider: Option<String>,
    #[serde(rename = "maxTokens")]
    pub(crate) max_tokens: Option<u32>,
    pub(crate) temperature: Option<f32>,
    #[serde(rename = "authKey")]
    pub(crate) auth_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ChatRequest {
    #[serde(rename = "apiKey")]
    pub(crate) api_key: String,
    pub(crate) messages: Vec<Message>,
    #[serde(rename = "llmProvider")]
    pub(crate) llm_provider: Option<String>,
    #[serde(rename = "model")]
    pub(crate) model: Option<String>,
    #[serde(rename = "mcpServers")]
    pub(crate) mcp_servers: Option<Vec<String>>,
    pub(crate) metadata: Option<ConversationMetadata>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ChatResponse {
    #[serde(rename = "conversationId")]
    pub(crate) conversation_id: String,
    pub(crate) response: Message,
    #[serde(rename = "allMessages")]
    pub(crate) all_messages: Vec<Message>, // Include all messages from the conversation
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ConfigResponse {
    #[serde(rename = "defaultLlmProvider")]
    pub(crate) default_llm_provider: String,
    #[serde(rename = "maxTokens")]
    pub(crate) max_tokens: u32,
    pub(crate) temperature: f32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ProcessRequest {
    pub(crate) action: String,
    pub(crate) payload: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ProcessResponse {
    pub(crate) success: bool,
    pub(crate) data: String,
}

// JSON-RPC Message Types for MCP Protocol
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct JsonRpcRequest {
    pub(crate) jsonrpc: String,
    pub(crate) method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<Value>,
    pub(crate) id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct JsonRpcNotification {
    pub(crate) jsonrpc: String,
    pub(crate) method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct JsonRpcResponse {
    pub(crate) jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
    pub(crate) id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<Value>,
}

// MCP Protocol Specific Types
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct McpInitializeParams {
    #[serde(rename = "protocolVersion")]
    pub(crate) protocol_version: String,
    #[serde(rename = "clientInfo")]
    pub(crate) client_info: McpClientInfo,
    pub(crate) capabilities: McpCapabilities,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct McpClientInfo {
    pub(crate) name: String,
    pub(crate) version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct McpCapabilities {
    // Empty for now, can be extended as needed
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct McpToolCallParams {
    pub(crate) name: String,
    pub(crate) arguments: Value,
}

// Tool execution result types
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ToolExecutionResult {
    pub(crate) result: Value,
    pub(crate) success: bool,
}

// Anthropic schema transformation result
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct AnthropicSchema {
    #[serde(rename = "type")]
    pub(crate) schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) properties: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) required: Option<Vec<String>>,
}

// WebSocket Message Types
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum WsClientMessage {
    #[serde(rename = "auth")]
    Auth {
        #[serde(rename = "apiKey")]
        api_key: String,
    },
    #[serde(rename = "chat")]
    Chat { payload: WsChatPayload },
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct WsChatPayload {
    pub(crate) messages: Vec<Message>,
    #[serde(rename = "llmProvider")]
    pub(crate) llm_provider: Option<String>,
    #[serde(rename = "model")]
    pub(crate) model: Option<String>,
    #[serde(rename = "mcpServers")]
    pub(crate) mcp_servers: Option<Vec<String>>,
    pub(crate) metadata: Option<ConversationMetadata>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub(crate) enum WsServerMessage {
    #[serde(rename = "auth_success")]
    AuthSuccess { message: String },
    #[serde(rename = "auth_error")]
    AuthError { error: String },
    #[serde(rename = "status")]
    Status {
        status: String,
        message: Option<String>,
    },
    #[serde(rename = "stream")]
    Stream {
        iteration: u32,
        message: String,
        #[serde(rename = "tool_calls")]
        tool_calls: Option<String>,
    },
    #[serde(rename = "message")]
    Message { message: Message },
    #[serde(rename = "chat_complete")]
    ChatComplete { payload: ChatResponse },
    #[serde(rename = "error")]
    Error { error: String },
    #[serde(rename = "pong")]
    Pong,
}

// Trial notification type
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TrialNotification {
    pub(crate) show: bool,
    pub(crate) title: String,
    pub(crate) message: String,
    #[serde(rename = "allowDismiss")]
    pub(crate) allow_dismiss: bool,
    #[serde(rename = "allowDoNotShowAgain")]
    pub(crate) allow_do_not_show_again: bool,
}

// Hypergrid types
#[derive(Clone, Debug)]
pub(crate) struct HypergridConnection {
    pub(crate) server_id: String,
    pub(crate) url: String,
    pub(crate) token: String,
    pub(crate) client_id: String,
    pub(crate) node: String,
    pub(crate) last_retry: std::time::Instant,
    pub(crate) retry_count: u32,
    pub(crate) connected: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub(crate) enum HypergridRequest {
    SearchRegistry(String),
    CallProvider {
        #[serde(rename = "providerId")]
        provider_id: String,
        #[serde(rename = "providerName")]
        provider_name: String,
        arguments: Vec<(String, String)>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct HypergridMessage {
    #[serde(flatten)]
    pub(crate) request: HypergridMessageType,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub(crate) enum HypergridMessageType {
    SearchRegistry(String),
    CallProvider {
        #[serde(rename = "providerId")]
        provider_id: String,
        #[serde(rename = "providerName")]
        provider_name: String,
        arguments: Vec<(String, String)>,
    },
}

// OAuth types
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct OAuthExchangeRequest {
    pub(crate) code: String,
    pub(crate) verifier: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct OAuthTokenResponse {
    pub(crate) refresh: String,
    pub(crate) access: String,
    pub(crate) expires: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct OAuthRefreshRequest {
    #[serde(rename = "refreshToken")]
    pub(crate) refresh_token: String,
}
