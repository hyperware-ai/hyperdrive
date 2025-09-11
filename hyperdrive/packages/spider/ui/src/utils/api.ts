// Import the generated functions and types directly
import {
  set_api_key as _setApiKey,
  list_api_keys as _listApiKeys,
  remove_api_key as _removeApiKey,
  create_spider_key as _createSpiderKey,
  list_spider_keys as _listSpiderKeys,
  revoke_spider_key as _revokeSpiderKey,
  add_mcp_server as _addMcpServer,
  list_mcp_servers as _listMcpServers,
  connect_mcp_server as _connectMcpServer,
  disconnect_mcp_server as _disconnectMcpServer,
  remove_mcp_server as _removeMcpServer,
  list_conversations as _listConversations,
  get_conversation as _getConversation,
  get_config as _getConfig,
  update_config as _updateConfig,
  chat as _chat,
  get_admin_key as _getAdminKey,
  type ApiKeyInfo,
  type SpiderApiKey,
  type McpServer,
  type Conversation,
  type ConfigResponse,
  type ChatResponse,
  type Message,
  type ConversationMetadata,
  type TransportConfig,
} from '@caller-utils';

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
