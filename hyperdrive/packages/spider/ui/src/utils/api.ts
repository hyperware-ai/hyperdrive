// Import the generated functions and types directly
import { Spider } from '@caller-utils';

// Alias the functions for internal use
const _setApiKey = Spider.set_api_key;
const _listApiKeys = Spider.list_api_keys;
const _removeApiKey = Spider.remove_api_key;
const _createSpiderKey = Spider.create_spider_key;
const _listSpiderKeys = Spider.list_spider_keys;
const _revokeSpiderKey = Spider.revoke_spider_key;
const _addMcpServer = Spider.add_mcp_server;
const _listMcpServers = Spider.list_mcp_servers;
const _connectMcpServer = Spider.connect_mcp_server;
const _disconnectMcpServer = Spider.disconnect_mcp_server;
const _removeMcpServer = Spider.remove_mcp_server;
const _listConversations = Spider.list_conversations;
const _getConversation = Spider.get_conversation;
const _getConfig = Spider.get_config;
const _updateConfig = Spider.update_config;
const _chat = Spider.chat;
const _getAdminKey = Spider.get_admin_key;

// Type aliases
type ApiKeyInfo = Spider.ApiKeyInfo;
type SpiderApiKey = Spider.SpiderApiKey;
type McpServer = Spider.McpServer;
type Conversation = Spider.Conversation;
type ConfigResponse = Spider.ConfigRes;
type ChatResponse = Spider.ChatRes;
type Message = Spider.Message;
type ConversationMetadata = Spider.ConversationMetadata;
type TransportConfig = Spider.TransportConfig;

export async function getAdminKey(): Promise<string> {
  return _getAdminKey();
}

export async function setApiKey(provider: string, key: string) {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _setApiKey({ provider, key, authKey });
}

export async function listApiKeys(): Promise<ApiKeyInfo[]> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _listApiKeys({ authKey });
}

export async function removeApiKey(provider: string) {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _removeApiKey({ provider, authKey });
}

export async function createSpiderKey(name: string, permissions: string[]): Promise<SpiderApiKey> {
  const adminKey = (window as any).__spiderAdminKey;
  if (!adminKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _createSpiderKey({ name, permissions, adminKey });
}

export async function listSpiderKeys(): Promise<SpiderApiKey[]> {
  const adminKey = (window as any).__spiderAdminKey;
  if (!adminKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _listSpiderKeys({ adminKey });
}

export async function revokeSpiderKey(key: string) {
  const adminKey = (window as any).__spiderAdminKey;
  if (!adminKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _revokeSpiderKey({ keyId: key, adminKey });
}

export async function addMcpServer(name: string, transport: TransportConfig): Promise<string> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _addMcpServer({ name, transport, authKey });
}

export async function listMcpServers(): Promise<McpServer[]> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _listMcpServers({ authKey });
}

export async function connectMcpServer(serverId: string) {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _connectMcpServer({ serverId, authKey });
}

export async function disconnectMcpServer(serverId: string) {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _disconnectMcpServer({ serverId, authKey });
}

export async function removeMcpServer(serverId: string) {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _removeMcpServer({ serverId, authKey });
}

export async function listConversations(client?: string, limit?: number, offset?: number): Promise<Conversation[]> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _listConversations({
    client: client || null,
    limit: limit || null,
    offset: offset || null,
    authKey
  });
}

export async function getConversation(conversationId: string): Promise<Conversation> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _getConversation({ conversationId, authKey });
}

export async function getConfig(): Promise<ConfigResponse> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _getConfig({ authKey });
}

export async function updateConfig(config: Partial<ConfigResponse>): Promise<string> {
  const authKey = (window as any).__spiderAdminKey;
  if (!authKey) {
    throw new Error('Admin key not available. Please refresh the page.');
  }
  return _updateConfig({
    defaultLlmProvider: config.defaultLlmProvider || null,
    maxTokens: config.maxTokens || null,
    temperature: config.temperature || null,
    buildContainerWsUri: config.buildContainerWsUri || null,
    buildContainerApiKey: config.buildContainerApiKey || null,
    authKey
  });
}

export async function chat(apiKey: string, messages: Message[], llmProvider?: string, model?: string, mcpServers?: string[], metadata?: ConversationMetadata, signal?: AbortSignal): Promise<ChatResponse> {
  // TODO: Pass signal to the underlying API call when supported
  return _chat({
    apiKey,
    messages,
    llmProvider: llmProvider || null,
    model: model || null,
    mcpServers: mcpServers || null,
    metadata: metadata || null
  });
}

// Re-export types for use in other files
export type {
  ApiKeyInfo,
  SpiderApiKey,
  McpServer,
  Conversation,
  ConfigResponse,
  ChatResponse,
  Message,
  ConversationMetadata,
  TransportConfig
};
