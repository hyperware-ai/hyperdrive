import { Chat } from '#caller-utils';
export type Chat = Chat.Chat;
export type ChatMessage = Chat.ChatMessage;

interface StoredChatState {
  chats: Chat[];
  lastSyncTimestamp: number;
  messageHashes: Record<string, string>; // chatId -> hash of message IDs
  activeChatId?: string; // Remember which chat was active
}

class BrowserStorage {
  private readonly STORAGE_KEY = 'chat_state';
  private readonly STORAGE_VERSION = '1.0';

  // Store chat state in localStorage
  saveChatState(chats: Chat[], activeChatId?: string): void {
    try {
      console.log('[STORAGE] Saving chat state:', {
        chatCount: chats.length,
        activeChatId,
        timestamp: new Date().toISOString()
      });
      
      // Much more aggressive message limiting to avoid quota issues
      const limitedChats: Chat[] = chats.map(chat => ({
        ...chat,
        // Only keep last 20 messages per chat, and strip unnecessary data
        messages: chat.messages.slice(-20).map(msg => ({
          id: msg.id,
          sender: msg.sender,
          content: msg.content.length > 500 ? msg.content.substring(0, 500) + '...' : msg.content,
          timestamp: msg.timestamp,
          sequence: msg.sequence ?? null,
          status: msg.status,
          reply_to: msg.reply_to,
          reactions: msg.reactions?.slice(0, 5) || [], // Limit reactions
          message_type: msg.message_type,
          file_info: msg.file_info ? {
            ...msg.file_info,
            // Don't store base64 data or large URLs
            url: msg.file_info.url.length > 100 ? '' : msg.file_info.url
          } : null
        } as ChatMessage))
      }));

      const state: StoredChatState = {
        chats: limitedChats,
        lastSyncTimestamp: Date.now(),
        messageHashes: {}, // Skip hashes to save space
        activeChatId
      };

      const serialized = JSON.stringify(state);
      const sizeKB = serialized.length / 1024;
      
      console.log('[STORAGE] Attempting to save, size:', sizeKB.toFixed(2), 'KB');
      
      // If still too large, reduce further
      if (sizeKB > 2048) { // 2MB limit to be safe
        console.log('[STORAGE] State too large, reducing to 10 messages per chat...');
        const minimalChats = chats.map(chat => ({
          ...chat,
          messages: chat.messages.slice(-10).map(msg => ({
            id: msg.id,
            sender: msg.sender,
            content: msg.content.substring(0, 200),
            timestamp: msg.timestamp,
            sequence: msg.sequence ?? null,
            status: msg.status,
            reply_to: null,
            reactions: [],
            message_type: msg.message_type,
            file_info: null
          }))
        }));
        
        const minimalState: StoredChatState = {
          chats: minimalChats,
          lastSyncTimestamp: Date.now(),
          messageHashes: {},
          activeChatId
        };
        
        const minimalSerialized = JSON.stringify(minimalState);
        localStorage.setItem(this.STORAGE_KEY, minimalSerialized);
        console.log('[STORAGE] Saved minimal state, size:', (minimalSerialized.length / 1024).toFixed(2), 'KB');
      } else {
        // Clear old data first to make room
        try {
          // Clear any other old storage keys from this app
          const keysToRemove: string[] = [];
          for (let i = 0; i < localStorage.length; i++) {
            const key = localStorage.key(i);
            if (key && key !== this.STORAGE_KEY && key.startsWith('chat_')) {
              keysToRemove.push(key);
            }
          }
          keysToRemove.forEach(key => localStorage.removeItem(key));
        } catch (e) {
          console.log('[STORAGE] Could not clear old keys:', e);
        }
        
        localStorage.setItem(this.STORAGE_KEY, serialized);
        console.log('[STORAGE] State saved successfully, size:', sizeKB.toFixed(2), 'KB');
      }
    } catch (error) {
      console.error('[STORAGE] Failed to save chat state:', error);
      // If quota exceeded, try to save just the chat list without messages
      if (error instanceof DOMException && error.name === 'QuotaExceededError') {
        try {
          console.log('[STORAGE] Quota exceeded, saving minimal state without messages...');
          const minimalState: StoredChatState = {
            chats: chats.map(chat => ({
              ...chat,
              messages: [] // No messages at all
            })),
            lastSyncTimestamp: Date.now(),
            messageHashes: {},
            activeChatId
          };
          localStorage.setItem(this.STORAGE_KEY, JSON.stringify(minimalState));
          console.log('[STORAGE] Saved minimal state without messages');
        } catch (e) {
          console.error('[STORAGE] Could not save even minimal state, clearing storage');
          this.clearStorage();
        }
      }
    }
  }

  // Load chat state from localStorage
  loadChatState(): StoredChatState | null {
    try {
      console.log('[STORAGE] Attempting to load chat state...');
      const serialized = localStorage.getItem(this.STORAGE_KEY);
      
      if (!serialized) {
        console.log('[STORAGE] No stored state found in localStorage');
        return null;
      }

      console.log('[STORAGE] Found stored state, size:', (serialized.length / 1024).toFixed(2), 'KB');
      const parsed = JSON.parse(serialized);
      
      // Check version compatibility
      if (parsed.version !== this.STORAGE_VERSION) {
        console.log('[STORAGE] Version mismatch, clearing storage. Expected:', this.STORAGE_VERSION, 'Got:', parsed.version);
        this.clearStorage();
        return null;
      }

      console.log('[STORAGE] Successfully loaded state with', parsed.state?.chats?.length || 0, 'chats');
      return parsed.state;
    } catch (error) {
      console.error('[STORAGE] Failed to load chat state:', error);
      this.clearStorage();
      return null;
    }
  }

  // Generate diff between stored and server state
  generateDiff(storedChats: Chat[], serverChats: Chat[]): ChatStateDiff {
    const diff: ChatStateDiff = {
      newChats: [],
      updatedChats: [],
      deletedChatIds: [],
      newMessages: {},
      updatedMessages: {},
      deletedMessageIds: {}
    };

    const storedChatsMap = new Map(storedChats.map(c => [c.id, c]));
    const serverChatsMap = new Map(serverChats.map(c => [c.id, c]));

    // Find new and updated chats
    serverChats.forEach(serverChat => {
      const storedChat = storedChatsMap.get(serverChat.id);
      
      if (!storedChat) {
        diff.newChats.push(serverChat);
      } else {
        // Check if chat metadata changed
        if (this.hasChatMetadataChanged(storedChat, serverChat)) {
          diff.updatedChats.push(serverChat);
        }

        // Find message differences
        const messageDiff = this.getMessageDiff(storedChat.messages, serverChat.messages);
        if (messageDiff.new.length > 0) {
          diff.newMessages[serverChat.id] = messageDiff.new;
        }
        if (messageDiff.updated.length > 0) {
          diff.updatedMessages[serverChat.id] = messageDiff.updated;
        }
        if (messageDiff.deleted.length > 0) {
          diff.deletedMessageIds[serverChat.id] = messageDiff.deleted;
        }
      }
    });

    // Find deleted chats
    storedChats.forEach(storedChat => {
      if (!serverChatsMap.has(storedChat.id)) {
        diff.deletedChatIds.push(storedChat.id);
      }
    });

    return diff;
  }

  // Apply diff to stored state
  applyDiff(storedChats: Chat[], diff: ChatStateDiff): Chat[] {
    const chatsMap = new Map(storedChats.map(c => [c.id, c]));

    // Remove deleted chats
    diff.deletedChatIds.forEach(id => chatsMap.delete(id));

    // Add new chats
    diff.newChats.forEach(chat => chatsMap.set(chat.id, chat));

    // Update existing chats
    diff.updatedChats.forEach(updatedChat => {
      const existingChat = chatsMap.get(updatedChat.id);
      if (existingChat) {
        chatsMap.set(updatedChat.id, {
          ...existingChat,
          ...updatedChat,
          messages: existingChat.messages // Keep messages, they're handled separately
        });
      }
    });

    // Apply message changes
    Object.entries(diff.newMessages).forEach(([chatId, messages]) => {
      const chat = chatsMap.get(chatId);
      if (chat) {
        chat.messages = [...chat.messages, ...messages].sort((a, b) => a.timestamp - b.timestamp);
      }
    });

    Object.entries(diff.updatedMessages).forEach(([chatId, messages]) => {
      const chat = chatsMap.get(chatId);
      if (chat) {
        const messageMap = new Map(chat.messages.map(m => [m.id, m]));
        messages.forEach(msg => messageMap.set(msg.id, msg));
        chat.messages = Array.from(messageMap.values()).sort((a, b) => a.timestamp - b.timestamp);
      }
    });

    Object.entries(diff.deletedMessageIds).forEach(([chatId, messageIds]) => {
      const chat = chatsMap.get(chatId);
      if (chat) {
        const idsToDelete = new Set(messageIds);
        chat.messages = chat.messages.filter(m => !idsToDelete.has(m.id));
      }
    });

    return Array.from(chatsMap.values());
  }

  // Get only changes since last sync
  getChangesSinceLastSync(timestamp: number): SyncRequest {
    const stored = this.loadChatState();
    if (!stored) {
      return { fullSync: true };
    }

    return {
      fullSync: false,
      lastSyncTimestamp: stored.lastSyncTimestamp,
      messageHashes: stored.messageHashes
    };
  }

  private hasChatMetadataChanged(stored: Chat, server: Chat): boolean {
    return stored.counterparty !== server.counterparty ||
           stored.is_blocked !== server.is_blocked ||
           stored.notify !== server.notify ||
           stored.unread_count !== server.unread_count;
  }

  private getMessageDiff(storedMessages: ChatMessage[], serverMessages: ChatMessage[]) {
    const storedMap = new Map(storedMessages.map(m => [m.id, m]));
    const serverMap = new Map(serverMessages.map(m => [m.id, m]));

    const newMessages: ChatMessage[] = [];
    const updatedMessages: ChatMessage[] = [];
    const deletedIds: string[] = [];

    // Find new and updated messages
    serverMessages.forEach(serverMsg => {
      const storedMsg = storedMap.get(serverMsg.id);
      if (!storedMsg) {
        newMessages.push(serverMsg);
      } else if (this.hasMessageChanged(storedMsg, serverMsg)) {
        updatedMessages.push(serverMsg);
      }
    });

    // Find deleted messages
    storedMessages.forEach(storedMsg => {
      if (!serverMap.has(storedMsg.id)) {
        deletedIds.push(storedMsg.id);
      }
    });

    return {
      new: newMessages,
      updated: updatedMessages,
      deleted: deletedIds
    };
  }

  private hasMessageChanged(stored: ChatMessage, server: ChatMessage): boolean {
    return stored.content !== server.content ||
           stored.status !== server.status ||
           JSON.stringify(stored.reactions) !== JSON.stringify(server.reactions);
  }

  private hashString(str: string): string {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash; // Convert to 32-bit integer
    }
    return hash.toString(36);
  }

  clearStorage(): void {
    console.log('[STORAGE] Clearing storage...');
    localStorage.removeItem(this.STORAGE_KEY);
    // Also clear any other app-related keys
    const keysToRemove: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && (key.startsWith('chat_') || key.startsWith('app_'))) {
        keysToRemove.push(key);
      }
    }
    keysToRemove.forEach(key => {
      console.log('[STORAGE] Removing old key:', key);
      localStorage.removeItem(key);
    });
  }
  
  // Get current storage size
  getStorageSize(): number {
    let totalSize = 0;
    for (let key in localStorage) {
      if (localStorage.hasOwnProperty(key)) {
        totalSize += localStorage[key].length + key.length;
      }
    }
    return totalSize / 1024; // Return in KB
  }
}

interface ChatStateDiff {
  newChats: Chat[];
  updatedChats: Chat[];
  deletedChatIds: string[];
  newMessages: Record<string, ChatMessage[]>;
  updatedMessages: Record<string, ChatMessage[]>;
  deletedMessageIds: Record<string, string[]>;
}

interface SyncRequest {
  fullSync: boolean;
  lastSyncTimestamp?: number;
  messageHashes?: Record<string, string>;
}

export const browserStorage = new BrowserStorage();
export type { StoredChatState, ChatStateDiff, SyncRequest };
