// WebSocket message types for Spider chat

import { Message, ConversationMetadata, ChatResponse } from '@caller-utils';

// Client -> Server messages
export type WsClientMessage = 
  | AuthMessage 
  | ChatMessage 
  | CancelMessage
  | PingMessage;

export interface AuthMessage {
  type: 'auth';
  apiKey: string;
}

export interface ChatMessage {
  type: 'chat';
  payload: {
    messages: Message[];
    llmProvider?: string;
    mcpServers?: string[];
    metadata?: ConversationMetadata;
  };
}

export interface CancelMessage {
  type: 'cancel';
}

export interface PingMessage {
  type: 'ping';
}

// Server -> Client messages
export type WsServerMessage =
  | AuthSuccessMessage
  | AuthErrorMessage
  | StatusMessage
  | StreamMessage
  | MessageUpdate
  | ChatCompleteMessage
  | ErrorMessage
  | PongMessage;

export interface AuthSuccessMessage {
  type: 'auth_success';
  message: string;
}

export interface AuthErrorMessage {
  type: 'auth_error';
  error: string;
}

export interface StatusMessage {
  type: 'status';
  status: string;
  message?: string;
}

export interface StreamMessage {
  type: 'stream';
  iteration: number;
  message: string;
  tool_calls?: string;
}

export interface MessageUpdate {
  type: 'message';
  message: Message;
}

export interface ChatCompleteMessage {
  type: 'chat_complete';
  payload: ChatResponse;
}

export interface ErrorMessage {
  type: 'error';
  error: string;
}

export interface PongMessage {
  type: 'pong';
}