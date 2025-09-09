declare module '../../target/ui/caller-utils' {
  export function setApiKey(requestBody: string): Promise<string>;
  export function listApiKeys(requestBody: string): Promise<string>;
  export function removeApiKey(provider: string): Promise<string>;
  export function createSpiderKey(requestBody: string): Promise<string>;
  export function listSpiderKeys(requestBody: string): Promise<string>;
  export function revokeSpiderKey(keyId: string): Promise<string>;
  export function addMcpServer(requestBody: string): Promise<string>;
  export function listMcpServers(requestBody: string): Promise<string>;
  export function connectMcpServer(serverId: string): Promise<string>;
  export function listConversations(requestBody: string): Promise<string>;
  export function getConversation(conversationId: string): Promise<string>;
  export function getConfig(requestBody: string): Promise<string>;
  export function updateConfig(requestBody: string): Promise<string>;
  export function chat(requestBody: string): Promise<string>;
}