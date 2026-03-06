import { Chat } from '#caller-utils';

// Additional frontend-specific types
export interface WsClientMessage {
  SendMessage?: { chat_id: string; content: string; reply_to?: string };
  Ack?: { message_id: string };
  MarkRead?: { chat_id: string };
  MarkGroupRead?: { group_id: string };
  UpdateStatus?: { status: string };
  AuthWithKey?: { chat_key: string };
  BrowserMessage?: { content: string };
  Heartbeat?: null;
}

export interface WsServerMessage {
  NewMessage?: Chat.ChatMessage;
  MessageAck?: { message_id: string };
  StatusUpdate?: { node: string; status: string };
  ChatUpdate?: Chat.Chat;
  ProfileUpdate?: { node: string; profile: Chat.UserProfile };
  AuthSuccess?: { chat_id: string; history: Chat.ChatMessage[] };
  AuthFailed?: { reason: string };
  GroupUpdate?: { group_id: string };
  Heartbeat?: null;
  Error?: { message: string };
}
