import { Chat } from '#caller-utils';
import { SpiderMessage } from '../types/spider';
export type Chat = Chat.Chat;
export type ChatMessage = Chat.ChatMessage;

const DB_VERSION = 2;
const CHATS_STORE = 'chats';
const MESSAGES_STORE = 'messages';
const METADATA_STORE = 'metadata';
const SPIDER_MESSAGES_STORE = 'spider_messages';

// Get node-scoped DB name for localhost isolation
function getDBName(): string {
  const node = (window as any).our?.node || 'default';
  // Sanitize node name for DB name (keep alphanumeric, dots, dashes, underscores)
  return `ChatAppDB_${node.replace(/[^a-zA-Z0-9_.-]/g, '_')}`;
}

class IndexedDBStorage {
  private db: IDBDatabase | null = null;

  async init(): Promise<void> {
    return new Promise((resolve, reject) => {
      const dbName = getDBName();
      console.log('[IDB] Initializing IndexedDB:', dbName);
      const request = indexedDB.open(dbName, DB_VERSION);

      request.onerror = () => {
        console.error('[IDB] Failed to open database:', request.error);
        reject(request.error);
      };

      request.onsuccess = () => {
        this.db = request.result;
        console.log('[IDB] Database opened successfully');
        resolve();
      };

      request.onupgradeneeded = (event) => {
        console.log('[IDB] Upgrading database schema...');
        const db = (event.target as IDBOpenDBRequest).result;

        // Create chats store
        if (!db.objectStoreNames.contains(CHATS_STORE)) {
          const chatsStore = db.createObjectStore(CHATS_STORE, { keyPath: 'id' });
          chatsStore.createIndex('counterparty', 'counterparty', { unique: false });
          chatsStore.createIndex('last_activity', 'last_activity', { unique: false });
        }

        // Create messages store with compound index for efficient queries
        if (!db.objectStoreNames.contains(MESSAGES_STORE)) {
          const messagesStore = db.createObjectStore(MESSAGES_STORE, { keyPath: ['chatId', 'id'] });
          messagesStore.createIndex('chatId', 'chatId', { unique: false });
          messagesStore.createIndex('timestamp', 'timestamp', { unique: false });
          messagesStore.createIndex('chatId_timestamp', ['chatId', 'timestamp'], { unique: false });
        }

        // Create metadata store for sync timestamps and active chat
        if (!db.objectStoreNames.contains(METADATA_STORE)) {
          db.createObjectStore(METADATA_STORE, { keyPath: 'key' });
        }

        if (!db.objectStoreNames.contains(SPIDER_MESSAGES_STORE)) {
          const spiderStore = db.createObjectStore(SPIDER_MESSAGES_STORE, { keyPath: 'id' });
          spiderStore.createIndex('timestamp', 'timestamp', { unique: false });
        }

        console.log('[IDB] Database schema upgraded');
      };
    });
  }

  private async ensureDB(): Promise<IDBDatabase> {
    if (!this.db) {
      await this.init();
    }
    if (!this.db) {
      throw new Error('Failed to initialize database');
    }
    return this.db;
  }

  // Save all chats and their complete message history
  async saveChats(chats: Chat[]): Promise<void> {
    console.log('[IDB] Saving', chats.length, 'chats with complete history...');
    const db = await this.ensureDB();
    const transaction = db.transaction([CHATS_STORE, MESSAGES_STORE], 'readwrite');
    const chatsStore = transaction.objectStore(CHATS_STORE);
    const messagesStore = transaction.objectStore(MESSAGES_STORE);

    // Replace cached snapshot completely so deleted backend chats/messages
    // do not flash back in from stale IndexedDB rows.
    await this.promisifyRequest(chatsStore.clear());
    await this.promisifyRequest(messagesStore.clear());

    // Save each chat and its messages
    for (const chat of chats) {
      // Save chat metadata (without messages)
      const chatData = {
        ...chat,
        messages: undefined // Don't store messages in chat object
      };
      await this.promisifyRequest(chatsStore.put(chatData));

      // Save all messages for this chat
      for (const message of chat.messages) {
        const messageData = {
          ...message,
          chatId: chat.id // Add chatId for indexing
        };
        await this.promisifyRequest(messagesStore.put(messageData));
      }
    }

    // Update sync timestamp
    await this.saveMetadata('lastSyncTimestamp', Date.now());
    console.log('[IDB] Saved all chats and messages');
  }

  // Load all chats with their complete message history
  async loadChats(): Promise<Chat[]> {
    console.log('[IDB] Loading chats from IndexedDB...');
    const db = await this.ensureDB();
    const transaction = db.transaction([CHATS_STORE, MESSAGES_STORE], 'readonly');
    const chatsStore = transaction.objectStore(CHATS_STORE);
    const messagesStore = transaction.objectStore(MESSAGES_STORE);

    // Get all chats
    const chats = await this.promisifyRequest(chatsStore.getAll()) as Chat[];
    console.log('[IDB] Loaded', chats.length, 'chats');

    // Load messages for each chat
    for (const chat of chats) {
      const index = messagesStore.index('chatId');
      const messages = await this.promisifyRequest(
        index.getAll(chat.id)
      ) as ChatMessage[];
      
      // Sort messages by timestamp
      chat.messages = messages.sort((a, b) => a.timestamp - b.timestamp);
      console.log('[IDB] Loaded', messages.length, 'messages for chat', chat.id);
    }

    return chats;
  }

  // Load messages for a specific chat (useful for lazy loading)
  async loadMessagesForChat(chatId: string, limit?: number, beforeTimestamp?: number): Promise<ChatMessage[]> {
    console.log('[IDB] Loading messages for chat', chatId);
    const db = await this.ensureDB();
    const transaction = db.transaction([MESSAGES_STORE], 'readonly');
    const messagesStore = transaction.objectStore(MESSAGES_STORE);
    const index = messagesStore.index('chatId_timestamp');

    let messages: ChatMessage[];
    
    if (beforeTimestamp) {
      // Get messages before a certain timestamp (for pagination)
      const range = IDBKeyRange.bound(
        [chatId, 0],
        [chatId, beforeTimestamp],
        false,
        true // Exclude the beforeTimestamp
      );
      messages = await this.promisifyRequest(index.getAll(range, limit)) as ChatMessage[];
    } else {
      // Get all or latest messages
      const allMessages = await this.promisifyRequest(
        index.getAll(IDBKeyRange.only(chatId))
      ) as ChatMessage[];
      
      messages = limit ? allMessages.slice(-limit) : allMessages;
    }

    return messages.sort((a, b) => a.timestamp - b.timestamp);
  }

  // Save a single message
  async saveMessage(chatId: string, message: ChatMessage): Promise<void> {
    const db = await this.ensureDB();
    const transaction = db.transaction([MESSAGES_STORE], 'readwrite');
    const store = transaction.objectStore(MESSAGES_STORE);
    
    const messageData = {
      ...message,
      chatId
    };
    
    await this.promisifyRequest(store.put(messageData));
  }

  // Update a single chat
  async saveChat(chat: Chat): Promise<void> {
    const db = await this.ensureDB();
    const transaction = db.transaction([CHATS_STORE, MESSAGES_STORE], 'readwrite');
    const chatsStore = transaction.objectStore(CHATS_STORE);
    const messagesStore = transaction.objectStore(MESSAGES_STORE);

    // Save chat metadata
    const chatData = {
      ...chat,
      messages: undefined
    };
    await this.promisifyRequest(chatsStore.put(chatData));

    // Save/update messages
    for (const message of chat.messages) {
      const messageData = {
        ...message,
        chatId: chat.id
      };
      await this.promisifyRequest(messagesStore.put(messageData));
    }

    // Update sync timestamp to mark cache as fresh
    await this.saveMetadata('lastSyncTimestamp', Date.now());
  }

  // Delete a chat and all its messages
  async deleteChat(chatId: string): Promise<void> {
    const db = await this.ensureDB();
    const transaction = db.transaction([CHATS_STORE, MESSAGES_STORE], 'readwrite');
    const chatsStore = transaction.objectStore(CHATS_STORE);
    const messagesStore = transaction.objectStore(MESSAGES_STORE);

    // Delete chat
    await this.promisifyRequest(chatsStore.delete(chatId));

    // Delete all messages for this chat
    const index = messagesStore.index('chatId');
    const messages = await this.promisifyRequest(index.getAllKeys(chatId)) as IDBValidKey[];
    
    for (const key of messages) {
      await this.promisifyRequest(messagesStore.delete(key));
    }
  }

  // Save metadata (active chat, sync timestamp, etc.)
  async saveMetadata(key: string, value: any): Promise<void> {
    const db = await this.ensureDB();
    const transaction = db.transaction([METADATA_STORE], 'readwrite');
    const store = transaction.objectStore(METADATA_STORE);
    
    await this.promisifyRequest(store.put({ key, value }));
  }

  // Load metadata
  async loadMetadata(key: string): Promise<any> {
    try {
      const db = await this.ensureDB();
      const transaction = db.transaction([METADATA_STORE], 'readonly');
      const store = transaction.objectStore(METADATA_STORE);
      
      const result = await this.promisifyRequest(store.get(key)) as any;
      return result?.value;
    } catch (error) {
      console.error('[IDB] Failed to load metadata:', error);
      return null;
    }
  }

  // Clear all data
  async clearAll(): Promise<void> {
    console.log('[IDB] Clearing all data...');
    const db = await this.ensureDB();
    const transaction = db.transaction(
      [CHATS_STORE, MESSAGES_STORE, METADATA_STORE, SPIDER_MESSAGES_STORE],
      'readwrite'
    );
    
    await this.promisifyRequest(transaction.objectStore(CHATS_STORE).clear());
    await this.promisifyRequest(transaction.objectStore(MESSAGES_STORE).clear());
    await this.promisifyRequest(transaction.objectStore(METADATA_STORE).clear());
    await this.promisifyRequest(transaction.objectStore(SPIDER_MESSAGES_STORE).clear());
    
    console.log('[IDB] All data cleared');
  }

  // Get database size estimate
  async getStorageEstimate(): Promise<{ usage: number; quota: number }> {
    if ('storage' in navigator && 'estimate' in navigator.storage) {
      const estimate = await navigator.storage.estimate();
      return {
        usage: estimate.usage || 0,
        quota: estimate.quota || 0
      };
    }
    return { usage: 0, quota: 0 };
  }

  // Helper to promisify IDB requests
  private promisifyRequest<T>(request: IDBRequest<T>): Promise<T> {
    return new Promise((resolve, reject) => {
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }

  async saveSpiderMessages(messages: SpiderMessage[]): Promise<void> {
    const db = await this.ensureDB();
    const transaction = db.transaction([SPIDER_MESSAGES_STORE], 'readwrite');
    const store = transaction.objectStore(SPIDER_MESSAGES_STORE);

    await this.promisifyRequest(store.clear());
    for (const message of messages) {
      if (!message.id) continue;
      await this.promisifyRequest(store.put(message));
    }
  }

  async loadSpiderMessages(): Promise<SpiderMessage[]> {
    const db = await this.ensureDB();
    const transaction = db.transaction([SPIDER_MESSAGES_STORE], 'readonly');
    const store = transaction.objectStore(SPIDER_MESSAGES_STORE);
    const messages = (await this.promisifyRequest(store.getAll())) as SpiderMessage[];
    return messages.sort((a, b) => a.timestamp - b.timestamp);
  }
}

export const idbStorage = new IndexedDBStorage();
