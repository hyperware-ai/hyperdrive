export interface SpiderMessageContent {
  text?: string | null;
  audio?: number[] | null;
  base_six_four_audio?: string | null;
}

export interface SpiderMessage {
  id?: string;
  role: string;
  content: SpiderMessageContent;
  replyTo?: string | null;
  toolCallsJson?: string | null;
  toolResultsJson?: string | null;
  timestamp: number;
  preservedText?: string;
  hidden?: boolean;
}

export interface SpiderConversationMetadata {
  startTime: string;
  client: string;
  fromStt: boolean;
}

export interface SpiderChatPayload {
  apiKey: string;
  messages: SpiderMessage[];
  llmProvider?: string | null;
  model?: string | null;
  mcpServers?: string[] | null;
  metadata?: SpiderConversationMetadata | null;
}

export interface SpiderChatResult {
  conversationId: string;
  response: SpiderMessage;
  allMessages?: SpiderMessage[];
  refreshedApiKey?: string;
}

export interface SpiderStatusInfo {
  connected: boolean;
  has_api_key: boolean;
  spider_available: boolean;
}

export interface SpiderConnectResult {
  api_key: string;
}

export interface SpiderMcpServersResult {
  servers: SpiderMcpServerSummary[];
}

export interface SpiderMcpServerSummary {
  id: string;
  name?: string | null;
  connected: boolean;
}

export type WsServerMessage =
  | { type: 'auth_success'; message: string }
  | { type: 'auth_error'; error: string }
  | { type: 'status'; status: string; message?: string }
  | { type: 'stream'; iteration: number; message: string; tool_calls?: string | null }
  | { type: 'message'; message: SpiderMessage }
  | { type: 'chat_complete'; payload: SpiderChatResult }
  | { type: 'error'; error: string }
  | { type: 'pong' };

export interface WsClientChatPayload {
  messages: SpiderMessage[];
  llmProvider?: string | null;
  model?: string | null;
  mcpServers?: string[] | null;
  metadata?: SpiderConversationMetadata | null;
  conversationId?: string | null;
}

export interface AuthMessage {
  type: 'auth';
  apiKey: string;
}

export interface ChatMessage {
  type: 'chat';
  payload: WsClientChatPayload;
}

export interface CancelMessage {
  type: 'cancel';
}

export interface PingMessage {
  type: 'ping';
}

export type WsClientMessage = AuthMessage | ChatMessage | CancelMessage | PingMessage;

export interface RateLimitError {
  error_type: string;
  message: string;
  retry_after_seconds: number;
}

export function isRateLimitError(obj: unknown): obj is RateLimitError {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'error_type' in obj &&
    (obj as RateLimitError).error_type === 'OutOfRequests'
  );
}
