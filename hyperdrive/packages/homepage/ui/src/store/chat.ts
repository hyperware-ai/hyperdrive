import { create } from 'zustand';
import { Chat as api } from '#caller-utils';
import { ChatWebSocket } from '../utils/websocket';
import { idbStorage } from '../utils/indexeddb';
import { WsServerMessage } from 'src/types/chat';
import { useGroupStore } from './groups';

interface ChatStore {
  // State
  nodeId: string | null;
  isConnected: boolean;
  profile: api.UserProfile | null;
  chats: api.Chat[];
  activeChat: api.Chat | null;
  settings: api.Settings;
  chatKeys: api.ChatKey[];
  wsConnection: ChatWebSocket | null;
  connectionStatus: 'connected' | 'disconnected' | 'connecting';
  error: string | null;
  isLoading: boolean;
  replyingTo: api.ChatMessage | null; // Message being replied to
  editingMessage: { id: string; content: string } | null; // Message being edited
  tempIdToRealId: { [tempId: string]: string }; // Map temp IDs to real message IDs
  pendingMessageHashes: { [hash: string]: string }; // Map content hashes to temp IDs for deduplication
  jumpToMessageId: string | null;
  
  // Actions
  initialize: () => Promise<void>;
  loadChatsFromServer: () => Promise<void>;
  syncWithServer: () => Promise<void>;
  verifySyncStatus: () => Promise<void>;
  forceSyncChat: (chatId: string) => Promise<void>;
  loadProfile: () => Promise<void>;
  loadSettings: () => Promise<void>;
  createChat: (counterparty: string) => Promise<void>;
  sendMessage: (chatId: string, content: string, replyTo?: string) => Promise<void>;
  recordPayment: (req: api.RecordPaymentReq) => Promise<void>;
  editMessage: (messageId: string, newContent: string) => Promise<void>;
  deleteMessage: (messageId: string) => Promise<void>;
  deleteMessageLocally: (messageId: string) => void;
  deleteChat: (chatId: string) => Promise<void>;
  updateChatSettings: (chatId: string, settings: { notify?: boolean | null }) => Promise<void>;
  updateSettings: (settings: api.Settings) => Promise<void>;
  updateProfile: (profile: api.UserProfile) => Promise<void>;
  searchChats: (query: string) => Promise<api.Chat[]>;
  searchIndex: (
    query: string,
    options?: { scope?: api.SearchScope; limit?: number }
  ) => Promise<api.SearchResultItem[]>;
  setJumpToMessageId: (messageId: string | null) => void;
  setActiveChat: (chat: api.Chat | null) => void;
  markChatAsRead: (chatId: string) => Promise<void>;
  connectWebSocket: () => void;
  disconnectWebSocket: () => void;
  handleWebSocketMessage: (message: WsServerMessage) => void;
  createChatLink: (singleUse: boolean) => Promise<string>;
  loadChatKeys: () => Promise<void>;
  revokeChatKey: (key: string) => Promise<void>;
  setError: (error: string | null) => void;
  clearError: () => void;
  setReplyingTo: (message: api.ChatMessage | null) => void;
  setEditingMessage: (message: { id: string; content: string } | null) => void;
}

// Track if already initialized to prevent double initialization
let isInitialized = false;
// Store timer handle for cleanup
let cleanupTimerId: ReturnType<typeof setInterval> | null = null;

// Helper function to generate a hash for message deduplication
function generateMessageHash(content: string, sender: string, timestamp: number): string {
  // Use 5-second buckets for timestamp to handle minor time differences
  const timeBucket = Math.floor(timestamp / 5);
  return `${sender}-${timeBucket}-${content.substring(0, 100)}`;
}

function normalizeDmChatId(nodeA?: string | null, nodeB?: string | null): string | null {
  const left = (nodeA || '').trim();
  const right = (nodeB || '').trim();
  if (!left || !right) return null;
  return left < right ? `${left}:${right}` : `${right}:${left}`;
}

function collapseAliasDmChats(
  chats: api.Chat[],
  ourNode?: string | null,
  activeChat?: api.Chat | null,
): { chats: api.Chat[]; activeChat: api.Chat | null; aliasChatIds: string[] } {
  const localNode = (ourNode || '').trim();
  if (!localNode || chats.length === 0) {
    return { chats, activeChat: activeChat || null, aliasChatIds: [] };
  }

  const dmGroups = new Map<string, api.Chat[]>();
  chats.forEach((chat) => {
    if (chat.id.startsWith('system:') || chat.id.startsWith('browser:')) return;
    const canonicalId = normalizeDmChatId(localNode, chat.counterparty);
    if (!canonicalId) return;
    const group = dmGroups.get(canonicalId) || [];
    group.push(chat);
    dmGroups.set(canonicalId, group);
  });

  const mergedByCanonicalId = new Map<string, api.Chat>();
  const aliasChatIds = new Set<string>();

  dmGroups.forEach((group, canonicalId) => {
    if (group.length < 2) return;

    const canonicalChat = group.find((chat) => chat.id === canonicalId);
    const baseChat = canonicalChat || group[0];

    const seenMessageIds = new Set<string>();
    const mergedMessages: api.ChatMessage[] = [];
    group.forEach((chat) => {
      chat.messages.forEach((message) => {
        if (seenMessageIds.has(message.id)) return;
        seenMessageIds.add(message.id);
        mergedMessages.push(message);
      });
    });
    mergedMessages.sort((left, right) => left.timestamp - right.timestamp);

    const profileSource = group.find((chat) => !!chat.counterparty_profile) || baseChat;
    const mergedChat: api.Chat = {
      ...baseChat,
      counterparty_profile: profileSource.counterparty_profile,
      messages: mergedMessages,
      last_activity: Math.max(...group.map((chat) => chat.last_activity || 0)),
      unread_count: Math.max(...group.map((chat) => chat.unread_count || 0)),
    };

    mergedByCanonicalId.set(canonicalId, mergedChat);
    group.forEach((chat) => {
      if (chat.id !== mergedChat.id) aliasChatIds.add(chat.id);
    });
  });

  if (aliasChatIds.size === 0) {
    return { chats, activeChat: activeChat || null, aliasChatIds: [] };
  }

  const emittedCanonicalIds = new Set<string>();
  const collapsedChats: api.Chat[] = [];

  chats.forEach((chat) => {
    if (chat.id.startsWith('system:') || chat.id.startsWith('browser:')) {
      collapsedChats.push(chat);
      return;
    }

    const canonicalId = normalizeDmChatId(localNode, chat.counterparty);
    if (!canonicalId) {
      collapsedChats.push(chat);
      return;
    }

    if (emittedCanonicalIds.has(canonicalId)) return;
    emittedCanonicalIds.add(canonicalId);

    const mergedChat = mergedByCanonicalId.get(canonicalId);
    collapsedChats.push(mergedChat || chat);
  });

  let collapsedActiveChat = activeChat || null;
  if (collapsedActiveChat) {
    const canonicalId = normalizeDmChatId(localNode, collapsedActiveChat.counterparty);
    if (canonicalId) {
      const mergedChat = mergedByCanonicalId.get(canonicalId);
      if (mergedChat) {
        collapsedActiveChat = mergedChat;
      } else if (!collapsedChats.some((chat) => chat.id === collapsedActiveChat?.id)) {
        collapsedActiveChat =
          collapsedChats.find(
            (chat) =>
              !chat.id.startsWith('system:') &&
              !chat.id.startsWith('browser:') &&
              chat.counterparty === activeChat?.counterparty,
          ) || null;
      }
    } else if (!collapsedChats.some((chat) => chat.id === collapsedActiveChat?.id)) {
      collapsedActiveChat = null;
    }
  }

  return {
    chats: collapsedChats,
    activeChat: collapsedActiveChat,
    aliasChatIds: Array.from(aliasChatIds),
  };
}

export const useChatStore = create<ChatStore>((set, get) => ({
  // Initial state
  nodeId: null,
  isConnected: false,
  profile: null,
  chats: [],
  activeChat: null,
  settings: {
    show_images: true,
    show_profile_pics: true,
    combine_chats_groups: true,
    notify_chats: true,
    notify_groups: true,
    notify_calls: true,
    allow_browser_chats: true,
    stt_enabled: false,
    stt_api_key: null,
    max_file_size_mb: 10,
  },
  chatKeys: [],
  wsConnection: null,
  connectionStatus: 'disconnected',
  error: null,
  isLoading: false,
  replyingTo: null,
  editingMessage: null,
  tempIdToRealId: {},
  pendingMessageHashes: {},
  jumpToMessageId: null,

  // Initialize the app
  initialize: async () => {
    console.log('[INIT] Starting initialization...');
    
    // Prevent double initialization
    if (isInitialized) {
      console.log('[INIT] Already initialized, skipping...');
      return;
    }
    isInitialized = true;
    
    // Set up periodic cleanup of temp ID mappings (every 2 minutes)
    // Clear any existing timer first
    if (cleanupTimerId !== null) {
      clearInterval(cleanupTimerId);
    }
    cleanupTimerId = setInterval(() => {
      const state = get();
      const fiveMinutesAgo = Date.now() / 1000 - 300;
      const cleanedMappings: typeof state.tempIdToRealId = {};
      let cleanedCount = 0;

      for (const [tempId, realId] of Object.entries(state.tempIdToRealId)) {
        const match = tempId.match(/^temp-(\d+)-/);
        if (match) {
          const tempTimestamp = parseInt(match[1]);
          if (tempTimestamp > fiveMinutesAgo) {
            cleanedMappings[tempId] = realId;
          } else {
            cleanedCount++;
          }
        }
      }

      if (cleanedCount > 0) {
        console.log('[CLEANUP] Removed', cleanedCount, 'old temp ID mappings');
        set({ tempIdToRealId: cleanedMappings });
      }
    }, 120000); // Run every 2 minutes
    
    try {
      // Check if we're connected to Hyperware
      const our = (window as any).our;
      console.log('[INIT] our.node:', our?.node);
      
      if (our?.node) {
        set({ nodeId: our.node, isConnected: true });
        
        // Initialize IndexedDB
        console.log('[INIT] Initializing IndexedDB...');
        await idbStorage.init();
        
        // Load cached chats from IndexedDB
        console.log('[INIT] Loading chats from IndexedDB...');
        const cachedChats = await idbStorage.loadChats();
        const activeChatId = await idbStorage.loadMetadata('activeChatId');
        const lastSync = await idbStorage.loadMetadata('lastSyncTimestamp');
        
        if (cachedChats.length > 0) {
          console.log('[INIT] Loaded', cachedChats.length, 'chats from IndexedDB');
          
          // Restore active chat
          let activeChat = null;
          if (activeChatId) {
            activeChat = cachedChats.find(c => c.id === activeChatId) || null;
          }

          const collapsedCached = collapseAliasDmChats(cachedChats, our.node, activeChat);
          if (collapsedCached.aliasChatIds.length > 0) {
            idbStorage.saveChats(collapsedCached.chats).catch((err) =>
              console.error('[IDB] Failed to rewrite cache after DM alias collapse:', err),
            );
          }
          
          set({ 
            chats: collapsedCached.chats,
            activeChat: collapsedCached.activeChat,
            isLoading: false // Don't show loading since we have cached data
          });
          
          // Check if we need to sync with server
          const ageMs = lastSync ? Date.now() - lastSync : Infinity;
          const ageMinutes = ageMs / (1000 * 60);
          console.log('[INIT] Cache age:', ageMinutes.toFixed(1), 'minutes');
          
          // Sync in background if data is older than 5 minutes
          if (ageMinutes > 5) {
            setTimeout(() => get().syncWithServer(), 1000);
          }
        } else {
          console.log('[INIT] No cached data, loading from server...');
          set({ isLoading: true });
          await get().loadChatsFromServer();
        }
        
        // Connect WebSocket for real-time updates
        console.log('[INIT] Connecting WebSocket...');
        get().connectWebSocket();
        
        // Load profile and settings in background
        Promise.all([
          get().loadProfile(),
          get().loadSettings(),
        ]).catch(error => {
          console.error('[INIT] Failed to load profile/settings:', error);
        });
        
        // Verify sync status after initial load
        setTimeout(() => {
          console.log('[INIT] Running initial sync verification...');
          get().verifySyncStatus();
        }, 3000);
        
        // Set up periodic sync verification (every 5 minutes)
        setInterval(() => {
          console.log('[PERIODIC] Running periodic sync verification...');
          get().verifySyncStatus();
        }, 5 * 60 * 1000);
        
      } else {
        set({ isConnected: false, error: 'Not connected to Hyperware' });
      }
    } catch (error) {
      console.error('[INIT] Initialization error:', error);
      set({ error: error instanceof Error ? error.message : 'Failed to initialize' });
    } finally {
      set({ isLoading: false });
    }
  },

  // Load chats from server and save to IndexedDB
  loadChatsFromServer: async () => {
    try {
      console.log('[SYNC] Loading chats from server...');
      const chats = await api.get_chats();
      console.log('[SYNC] Loaded', chats.length, 'chats from server');

      const state = get();
      const collapsed = collapseAliasDmChats(
        chats,
        state.nodeId || (window as any).our?.node,
        state.activeChat,
      );

      set({ chats: collapsed.chats, activeChat: collapsed.activeChat });
      
      // Save complete data to IndexedDB
      await idbStorage.saveChats(collapsed.chats);
      
      // Save active chat ID if we have one
      const nextState = get();
      if (nextState.activeChat) {
        await idbStorage.saveMetadata('activeChatId', nextState.activeChat.id);
      }
      
      console.log('[SYNC] Saved chats to IndexedDB');
    } catch (error) {
      console.error('[SYNC] Failed to load chats:', error);
      set({ error: 'Failed to load chats' });
    }
  },

  // Sync with server - much simpler now with IndexedDB
  syncWithServer: async () => {
    try {
      console.log('[SYNC] Syncing with server...');
      
      // Fetch all chats from server (complete history)
      const serverChats = await api.get_chats();
      console.log('[SYNC] Got', serverChats.length, 'chats from server');

      const state = get();
      const collapsed = collapseAliasDmChats(
        serverChats,
        state.nodeId || (window as any).our?.node,
        state.activeChat,
      );

      // Update state with server data
      set({ chats: collapsed.chats, activeChat: collapsed.activeChat });
      
      // Save everything to IndexedDB
      await idbStorage.saveChats(collapsed.chats);
      
      // Update metadata
      const nextState = get();
      if (nextState.activeChat) {
        // Update activeChat with fresh data
        const updatedActiveChat = collapsed.chats.find(c => c.id === nextState.activeChat?.id);
        if (updatedActiveChat) {
          set({ activeChat: updatedActiveChat });
        }
        await idbStorage.saveMetadata('activeChatId', nextState.activeChat.id);
      }
      
      console.log('[SYNC] Sync complete, saved to IndexedDB');
    } catch (error) {
      console.error('[SYNC] Failed to sync with server:', error);
    }
  },
  
  // Verify sync status - check for desyncs between frontend and backend
  verifySyncStatus: async () => {
    try {
      console.log('[SYNC-VERIFY] Starting sync verification...');
      
      // Get all sync hashes from backend
      const backendHashes = await api.get_all_sync_hashes();
      console.log('[SYNC-VERIFY] Got', backendHashes.length, 'hashes from backend');
      
      const state = get();
      const desyncedChats: string[] = [];
      
      // Check each chat's hash
      for (const backendHash of backendHashes) {
        const localChat = state.chats.find(c => c.id === backendHash.chat_id);
        
        if (!localChat) {
          console.log('[SYNC-VERIFY] Chat missing locally:', backendHash.chat_id);
          desyncedChats.push(backendHash.chat_id);
          continue;
        }
        
        // Lightweight consistency checks without hashing
        const localCount = localChat.messages.length;
        const localLast = localChat.messages[localCount - 1];
        const backendLastId = backendHash.last_message_id || null;
        const backendLastTs = backendHash.last_message_timestamp || null;

        const countMismatch = localCount !== backendHash.message_count;
        const idMismatch = (localLast?.id || null) !== backendLastId;
        const tsMismatch = (localLast?.timestamp || null) !== backendLastTs;

        if (countMismatch || idMismatch || tsMismatch) {
          console.log('[SYNC-VERIFY] Mismatch for chat:', backendHash.chat_id, {
            localCount,
            backendCount: backendHash.message_count,
            localLastId: localLast?.id,
            backendLastId,
            localLastTs: localLast?.timestamp,
            backendLastTs: backendLastTs,
          });
          desyncedChats.push(backendHash.chat_id);
        } else {
          console.log('[SYNC-VERIFY] Chat appears in sync:', backendHash.chat_id);
        }
      }
      
      // Check for local chats not on backend
      for (const localChat of state.chats) {
        if (!backendHashes.find(h => h.chat_id === localChat.id)) {
          console.log('[SYNC-VERIFY] Chat exists locally but not on backend:', localChat.id);
          desyncedChats.push(localChat.id);
        }
      }
      
      // If any chats are desynced, force a full sync
      if (desyncedChats.length > 0) {
        console.log('[SYNC-VERIFY] Found', desyncedChats.length, 'desynced chats, forcing full sync...');
        await get().syncWithServer();
      } else {
        console.log('[SYNC-VERIFY] All chats are in sync');
      }
    } catch (error) {
      console.error('[SYNC-VERIFY] Failed to verify sync status:', error);
      // On error, force a full sync to be safe
      await get().syncWithServer();
    }
  },
  
  // Force sync a specific chat
  forceSyncChat: async (chatId: string) => {
    try {
      console.log('[FORCE-SYNC] Force syncing chat:', chatId);
      
      // Get the chat from the server
      const serverChat = await api.get_chat({ chat_id: chatId });
      console.log('[FORCE-SYNC] Got chat from server with', serverChat.messages.length, 'messages');
      
      // Update local state
      set(state => {
        const updatedChats = state.chats.map(chat => 
          chat.id === chatId ? serverChat : chat
        );
        
        // Also update activeChat if it's the same
        const updatedActiveChat = state.activeChat?.id === chatId 
          ? serverChat 
          : state.activeChat;
        
        return {
          chats: updatedChats,
          activeChat: updatedActiveChat
        };
      });
      
      // Save to IndexedDB
      await idbStorage.saveChat(serverChat);
      
      console.log('[FORCE-SYNC] Chat synced successfully');
    } catch (error) {
      console.error('[FORCE-SYNC] Failed to force sync chat:', error);
      set({ error: `Failed to sync chat: ${error}` });
    }
  },

  // Load user profile
  loadProfile: async () => {
    try {
      const profile = await api.get_profile();
      set({ profile });
    } catch (error) {
      set({ error: 'Failed to load profile' });
    }
  },

  // Load settings
  loadSettings: async () => {
    try {
      const settings = await api.get_settings();
      set({ settings: { ...settings, combine_chats_groups: true } });
    } catch (error) {
      set({ error: 'Failed to load settings' });
    }
  },

  // Create a new chat
  createChat: async (counterparty: string) => {
    try {
      set({ isLoading: true });
      const chat = await api.create_chat({ counterparty });

      let aliasChatIds: string[] = [];
      let selectedChat: api.Chat | null = chat;

      set((state) => {
        const collapsed = collapseAliasDmChats(
          [chat, ...state.chats],
          state.nodeId || (window as any).our?.node,
          chat,
        );

        aliasChatIds = collapsed.aliasChatIds;
        selectedChat =
          collapsed.activeChat ||
          collapsed.chats.find((existingChat) => existingChat.counterparty === chat.counterparty) ||
          chat;

        return {
          chats: collapsed.chats,
          activeChat: selectedChat,
        };
      });

      if (selectedChat) {
        await idbStorage.saveChat(selectedChat);
        await idbStorage.saveMetadata('activeChatId', selectedChat.id);
      }
      if (aliasChatIds.length > 0) {
        await Promise.all(
          aliasChatIds.map((chatId) =>
            idbStorage
              .deleteChat(chatId)
              .catch((err) => console.error('[IDB] Failed to delete DM alias chat:', chatId, err)),
          ),
        );
      }
    } catch (error) {
      set({ error: 'Failed to create chat' });
    } finally {
      set({ isLoading: false });
    }
  },

  // Send a message
  sendMessage: async (chatId: string, content: string, replyTo?: string) => {
    // Create optimistic message immediately
    const timestamp = Math.floor(Date.now() / 1000);
    const tempId = `temp-${timestamp}-${Math.random()}`;
    const sender = (window as any).our?.node || '';
    const optimisticMessage: api.ChatMessage = {
      id: tempId,
      sender,
      content,
      timestamp,
      sequence: null,
      status: api.MessageStatus.Sending,
      reply_to: replyTo || null,
      reactions: [],
      message_type: api.MessageType.Text,
      file_info: null,
      payment_info: null,
    };
    
    // Generate hash for this message to detect duplicates
    const messageHash = generateMessageHash(content, sender, timestamp);
    console.log('[SEND] Creating optimistic message:', tempId, 'hash:', messageHash, 'content:', content.substring(0, 30));
    
    // Immediately show the message with "Sending" status
    set(state => {
      const updatedChats = state.chats.map(chat => 
        chat.id === chatId 
          ? { 
              ...chat, 
              messages: [...chat.messages, optimisticMessage], 
              last_activity: timestamp 
            }
          : chat
      );
      
      const updatedActiveChat = state.activeChat?.id === chatId 
        ? { 
            ...state.activeChat, 
            messages: [...state.activeChat.messages, optimisticMessage],
            last_activity: timestamp
          }
        : state.activeChat;
      
      console.log('[SEND] Added optimistic message to UI. Total messages:', updatedActiveChat?.messages.length);
      
      return {
        chats: updatedChats,
        activeChat: updatedActiveChat,
        pendingMessageHashes: {
          ...state.pendingMessageHashes,
          [messageHash]: tempId
        }
      };
    });
    
    try {
      const message = await api.send_message({ 
        chat_id: chatId, 
        content, 
        reply_to: replyTo || null,
        file_info: null
      });
      
      console.log('[SEND] Received real message from API:', message.id, 'replacing temp:', tempId);
      
      // Store the mapping from temp ID to real ID
      set(state => {
        const newTempIdToRealId = { ...state.tempIdToRealId, [tempId]: message.id };
        
        // Clean up the pending hash since API confirmed the message
        const cleanedPendingHashes = { ...state.pendingMessageHashes };
        Object.entries(cleanedPendingHashes).forEach(([hash, tid]) => {
          if (tid === tempId) {
            delete cleanedPendingHashes[hash];
          }
        });
        
        // Clean up old temp ID mappings (older than 5 minutes)
        const fiveMinutesAgo = Date.now() / 1000 - 300;
        const cleanedTempIdToRealId: typeof newTempIdToRealId = {};
        for (const [oldTempId, realId] of Object.entries(newTempIdToRealId)) {
          // Extract timestamp from temp ID format: temp-{timestamp}-{random}
          const match = oldTempId.match(/^temp-(\d+)-/);
          if (match) {
            const tempTimestamp = parseInt(match[1]);
            if (tempTimestamp > fiveMinutesAgo) {
              cleanedTempIdToRealId[oldTempId] = realId;
            } else {
              console.log('[SEND] Cleaning up old temp ID mapping:', oldTempId);
            }
          }
        }
        
        const updatedChats = state.chats.map(chat => 
          chat.id === chatId 
            ? { 
                ...chat, 
                messages: chat.messages.map(m => 
                  m.id === tempId ? { ...message, status: api.MessageStatus.Sent } : m
                ), 
                last_activity: message.timestamp 
              }
            : chat
        );
        
        // Save the updated chat to IndexedDB
        const updatedChat = updatedChats.find(c => c.id === chatId);
        if (updatedChat) {
          idbStorage.saveChat(updatedChat).catch(err =>
            console.error('[IDB] Failed to save chat after send:', updatedChat.id, err)
          );
        }

        const updatedActiveChat = state.activeChat?.id === chatId 
          ? { 
              ...state.activeChat, 
              messages: state.activeChat.messages.map(m => 
                m.id === tempId ? { ...message, status: api.MessageStatus.Sent } : m
              ) 
            }
          : state.activeChat;
        
        console.log('[SEND] Replaced optimistic message in UI. Total messages:', updatedActiveChat?.messages.length);
        
        return {
          chats: updatedChats,
          activeChat: updatedActiveChat,
          tempIdToRealId: cleanedTempIdToRealId,
          pendingMessageHashes: cleanedPendingHashes,
        };
      });
      
    } catch (error) {
      // On error, update the optimistic message to show failed status
      set(state => ({
        chats: state.chats.map(chat => 
          chat.id === chatId 
            ? { 
                ...chat, 
                messages: chat.messages.map(m => 
                  m.id === tempId ? { ...m, status: api.MessageStatus.Failed } : m
                )
              }
            : chat
        ),
        activeChat: state.activeChat?.id === chatId 
          ? { 
              ...state.activeChat, 
              messages: state.activeChat.messages.map(m => 
                m.id === tempId ? { ...m, status: api.MessageStatus.Failed } : m
              )
            }
          : state.activeChat,
        error: 'Failed to send message'
      }));
    }
  },

  // Record a blockchain payment as a non-spoofable system message
  recordPayment: async (req: api.RecordPaymentReq) => {
    try {
      const paymentMessage = await api.record_payment(req);
      set(state => {
        const appendIfMissing = (chat: api.Chat): api.Chat => {
          if (chat.id !== req.chat_id) return chat;
          if (chat.messages.some((m) => m.id === paymentMessage.id)) return chat;
          return {
            ...chat,
            messages: [...chat.messages, paymentMessage],
            last_activity: paymentMessage.timestamp,
          };
        };

        const chats = state.chats.map(appendIfMissing);
        const activeChat =
          state.activeChat?.id === req.chat_id
            ? appendIfMissing(state.activeChat)
            : state.activeChat;

        const changed = chats.find((chat) => chat.id === req.chat_id);
        if (changed) {
          idbStorage.saveChat(changed).catch((err) =>
            console.error('[IDB] Failed to save chat after recordPayment:', changed.id, err),
          );
        }

        return { chats, activeChat };
      });
    } catch (error) {
      set({ error: 'Failed to record payment event' });
      throw error;
    }
  },

  // Edit a message
  editMessage: async (messageId: string, newContent: string) => {
    try {
      const chatId = get().activeChat?.id;
      if (!chatId) throw new Error('No active chat');
      
      await api.edit_message({ 
        chat_id: chatId,
        message_id: messageId, 
        new_content: newContent 
      });
      
      // Update local state
      set(state => ({
        chats: state.chats.map(chat => ({
          ...chat,
          messages: chat.messages.map(msg => 
            msg.id === messageId ? { ...msg, content: newContent } : msg
          )
        }))
      }));
    } catch (error) {
      set({ error: 'Failed to edit message' });
    }
  },

  // Delete a message for both parties
  deleteMessage: async (messageId: string) => {
    try {
      const chatId = get().activeChat?.id;
      if (!chatId) throw new Error('No active chat');
      
      await api.delete_message({ 
        chat_id: chatId,
        message_id: messageId,
        delete_for_both: true
      });
      
      // Update local state
      set(state => {
        const updatedChats = state.chats.map(chat => ({
          ...chat,
          messages: chat.messages.filter(msg => msg.id !== messageId)
        }));
        
        // Save the updated chat to IndexedDB
        const updatedChat = updatedChats.find(c => c.id === chatId);
        if (updatedChat) {
          idbStorage.saveChat(updatedChat).catch(err =>
            console.error('[IDB] Failed to save chat after delete:', updatedChat.id, err)
          );
        }

        return {
          chats: updatedChats,
          // Also update activeChat if it's the same chat
          activeChat: state.activeChat?.id === chatId
            ? {
                ...state.activeChat,
                messages: state.activeChat.messages.filter(msg => msg.id !== messageId)
              }
            : state.activeChat
        };
      });
    } catch (error) {
      set({ error: 'Failed to delete message' });
    }
  },

  // Delete a message locally only
  deleteMessageLocally: (messageId: string) => {
    const chatId = get().activeChat?.id;
    if (!chatId) {
      console.error('No active chat');
      return;
    }
    
    // Update local state only (no API call)
    set(state => {
      const updatedChats = state.chats.map(chat => ({
        ...chat,
        messages: chat.messages.filter(msg => msg.id !== messageId)
      }));
      
      // Save the updated chat to IndexedDB
      const updatedChat = updatedChats.find(c => c.id === chatId);
      if (updatedChat) {
        idbStorage.saveChat(updatedChat).catch(err =>
          console.error('[IDB] Failed to save chat after local delete:', updatedChat.id, err)
        );
      }

      return {
        chats: updatedChats,
        // Also update activeChat if it's the same chat
        activeChat: state.activeChat?.id === chatId
          ? {
              ...state.activeChat,
              messages: state.activeChat.messages.filter(msg => msg.id !== messageId)
            }
          : state.activeChat
      };
    });
  },

  // Delete a chat
  deleteChat: async (chatId: string) => {
    try {
      await api.delete_chat({ chat_id: chatId });

      set(state => ({
        chats: state.chats.filter(chat => chat.id !== chatId),
        activeChat: state.activeChat?.id === chatId ? null : state.activeChat,
      }));
    } catch (error) {
      set({ error: 'Failed to delete chat' });
    }
  },

  // Update per-chat settings (notifications, etc)
  updateChatSettings: async (chatId: string, settings: { notify?: boolean | null }) => {
    try {
      const updatedChat = await api.update_chat_settings({
        chat_id: chatId,
        notify: settings.notify ?? null,
      });

      set(state => ({
        chats: state.chats.map(chat =>
          chat.id === chatId ? updatedChat : chat
        ),
        activeChat: state.activeChat?.id === chatId ? updatedChat : state.activeChat,
      }));
    } catch (error) {
      set({ error: 'Failed to update chat settings' });
    }
  },

  // Update settings
  updateSettings: async (settings: api.Settings) => {
    try {
      const nextSettings = { ...settings, combine_chats_groups: true };
      await api.update_settings(nextSettings);
      set({ settings: nextSettings });
    } catch (error) {
      set({ error: 'Failed to update settings' });
    }
  },

  // Update profile
  updateProfile: async (profile: api.UserProfile) => {
    try {
      await api.update_profile(profile);
      set({ profile });
    } catch (error) {
      set({ error: 'Failed to update profile' });
    }
  },

  // Search chats
  searchChats: async (query: string) => {
    try {
      return await api.search_chats({ query });
    } catch (error) {
      set({ error: 'Failed to search chats' });
      return [];
    }
  },

  // Search index (chats + groups)
  searchIndex: async (query: string, options = {}) => {
    try {
      const res = await api.search_index({
        query,
        scope: options.scope ?? api.SearchScope.All,
        limit: options.limit ?? null,
      });
      return res.results ?? [];
    } catch (error) {
      set({ error: 'Failed to search' });
      return [];
    }
  },

  setJumpToMessageId: (messageId: string | null) => set({ jumpToMessageId: messageId }),

  // Set active chat
  setActiveChat: (chat: api.Chat | null) => {
    set({ activeChat: chat, jumpToMessageId: chat ? get().jumpToMessageId : null });
    // Save active chat ID to IndexedDB
    if (chat) {
      idbStorage.saveMetadata('activeChatId', chat.id);
    } else {
      idbStorage.saveMetadata('activeChatId', null);
    }
  },

  // Mark chat as read
  markChatAsRead: async (chatId: string) => {
    set(state => ({
      chats: state.chats.map(chat => 
        chat.id === chatId ? { ...chat, unread_count: 0 } : chat
      )
    }));
    
    // Send via WebSocket if connected
    const ws = get().wsConnection;
    if (ws) {
      ws.send({ MarkRead: { chat_id: chatId } });
    }
  },

  // WebSocket connection
  connectWebSocket: () => {
    const ws = new ChatWebSocket();
    
    ws.connect((message: WsServerMessage) => {
      get().handleWebSocketMessage(message);
    });
    
    set({ 
      wsConnection: ws, 
      connectionStatus: 'connecting' 
    });
  },

  disconnectWebSocket: () => {
    const ws = get().wsConnection;
    if (ws) {
      ws.disconnect();
      set({ wsConnection: null, connectionStatus: 'disconnected' });
    }
    // Clear cleanup timer
    if (cleanupTimerId !== null) {
      clearInterval(cleanupTimerId);
      cleanupTimerId = null;
    }
  },

  handleWebSocketMessage: (message: WsServerMessage) => {
    console.log('[WS] Received message:', message);

    if (message.GroupUpdate) {
      const { group_id } = message.GroupUpdate;
      const { activeGroupId, refreshActiveGroup, refreshGroupPreviews, loadGroups } = useGroupStore.getState();
      if (activeGroupId === group_id) {
        refreshActiveGroup();
      } else {
        // Reload groups to get updated unread counts when not viewing this group
        loadGroups();
      }
      // Always update the chat list preview for this group
      refreshGroupPreviews([group_id]);
      return;
    }
    
    if (message.ChatUpdate) {
      console.log('[WS] Processing ChatUpdate:', message.ChatUpdate);
      // Handle new chat or chat update
      const serverChat = message.ChatUpdate;
      console.log('[WS] Chat update received for chat:', serverChat.id, 'with', serverChat.messages.length, 'messages');
      
      set(state => {
        const existingChatIndex = state.chats.findIndex(c => c.id === serverChat.id);
        let newChats;
        
        if (existingChatIndex >= 0) {
          // Update existing chat - merge messages carefully
          console.log('[WS] Updating existing chat at index:', existingChatIndex);
          const existingChat = state.chats[existingChatIndex];
          
          // Build a map of server messages by ID for quick lookup
          const serverMessageIds = new Set(serverChat.messages.map(m => m.id));
          
          // Start with all server messages
          const mergedMessages = [...serverChat.messages];
          
          // Check server messages against pending hashes to detect duplicates early
          const pendingHashes = { ...state.pendingMessageHashes };
          const tempToRealMap = { ...state.tempIdToRealId };
          let hashesUpdated = false;
          
          serverChat.messages.forEach(serverMsg => {
            const serverHash = generateMessageHash(serverMsg.content, serverMsg.sender, serverMsg.timestamp);
            const pendingTempId = pendingHashes[serverHash];
            
            if (pendingTempId) {
              // Found a match! Map the temp ID to real ID immediately
              console.log('[WS] Found pending message match via hash:', pendingTempId, '->', serverMsg.id);
              tempToRealMap[pendingTempId] = serverMsg.id;
              delete pendingHashes[serverHash];
              hashesUpdated = true;
            }
          });
          
          // Update state with new mappings if any were found
          if (hashesUpdated) {
            state.tempIdToRealId = tempToRealMap;
            state.pendingMessageHashes = pendingHashes;
          }
          
          // Preserve local messages that aren't on the server yet
          existingChat.messages.forEach(localMsg => {
            // Check if this is a temp message
            if (localMsg.id.startsWith('temp-')) {
              // Check if we have a real ID mapping for this temp message
              const realId = tempToRealMap[localMsg.id];
              
              if (realId && serverMessageIds.has(realId)) {
                // The server has the real version, so we don't need the temp one
                console.log('[WS] Temp message', localMsg.id, 'replaced by server version', realId);
              } else if (realId) {
                // We have a mapping but server doesn't have it yet (shouldn't happen)
                console.log('[WS] Temp message has mapping but not in server update yet:', localMsg.id);
              } else {
                // No mapping yet - keep the temp message
                console.log('[WS] Keeping unconfirmed temp message:', localMsg.id);
                mergedMessages.push(localMsg);
              }
            } else if (!serverMessageIds.has(localMsg.id)) {
              // This is a real message that's not in the server update
              // This should NOT happen for reaction updates - the server should have all messages
              console.log('[WS] WARNING: Local message not in server update:', localMsg.id);
              // Don't keep it - trust the server for the complete state
            }
          });
          
          // Sort messages by timestamp to maintain order
          mergedMessages.sort((a, b) => a.timestamp - b.timestamp);
          
          newChats = [...state.chats];
          newChats[existingChatIndex] = {
            ...serverChat,
            messages: mergedMessages
          };
        } else {
          // Add new chat
          console.log('[WS] Adding new chat');
          newChats = [...state.chats, serverChat];
        }
        
        // Update activeChat if it's the same chat being updated
        let updatedActiveChat = state.activeChat;
        if (state.activeChat && state.activeChat.id === serverChat.id) {
          console.log('[WS] Updating activeChat with merged data');
          const mergedChat = newChats.find(c => c.id === serverChat.id);
          if (mergedChat) {
            updatedActiveChat = mergedChat;
          }
        }

        const collapsed = collapseAliasDmChats(
          newChats,
          (window as any).our?.node as string | undefined,
          updatedActiveChat,
        );
        newChats = collapsed.chats;
        updatedActiveChat = collapsed.activeChat;
        if (collapsed.aliasChatIds.length > 0) {
          collapsed.aliasChatIds.forEach((chatId) => {
            idbStorage
              .deleteChat(chatId)
              .catch((err) => console.error('[IDB] Failed to delete DM alias after ChatUpdate:', chatId, err));
          });
        }
        
        // Save updated chat to IndexedDB
        const changedChat =
          newChats.find((chat) => chat.id === serverChat.id) ||
          newChats.find(
            (chat) =>
              !chat.id.startsWith('system:') &&
              !chat.id.startsWith('browser:') &&
              chat.counterparty === serverChat.counterparty,
          );
        if (changedChat) {
          idbStorage.saveChat(changedChat).catch(err =>
            console.error('[IDB] Failed to save chat after ChatUpdate:', changedChat.id, err)
          );
        }

        return {
          chats: newChats,
          activeChat: updatedActiveChat
        };
      });
    } else if (message.NewMessage) {
      const newMsg = message.NewMessage;
      console.log('[WS] Processing NewMessage:', newMsg);
      const our = (window as any).our;
      console.log('[WS] Our node:', our?.node, 'Message sender:', newMsg.sender);
      
      // Check if this message already exists (by ID or temp ID mapping)
      const messageAlreadyExists = get().tempIdToRealId[newMsg.id] || 
        Object.values(get().tempIdToRealId).includes(newMsg.id);
      
      if (messageAlreadyExists) {
        console.log('[WS] Message already exists via temp ID mapping, skipping:', newMsg.id);
        return;
      }
      
      // Only add the message if we didn't send it (prevents duplicates)
      if (newMsg.sender !== our?.node) {
        console.log('[WS] Adding message from other node');
        set(state => {
          const expectedChatId = normalizeDmChatId(our?.node, newMsg.sender);
          const exactChat = expectedChatId
            ? state.chats.find((chat) => chat.id === expectedChatId)
            : undefined;
          const fallbackChat = exactChat
            ? undefined
            : state.chats.find((chat) => chat.counterparty === newMsg.sender);
          const targetChatId = exactChat?.id || fallbackChat?.id || null;

          if (!targetChatId) {
            console.log('[WS] Warning: Could not find chat for message from:', newMsg.sender);
            return {
              chats: state.chats,
              activeChat: state.activeChat,
            };
          }

          let foundChat = false;
          const updatedChats = state.chats.map((chat) => {
            if (chat.id !== targetChatId) return chat;
            foundChat = true;
            console.log('[WS] Found chat for message:', chat.id);
            const messageExists = chat.messages.some((message) => message.id === newMsg.id);
            if (messageExists) {
              console.log('[WS] Message already exists, skipping');
              return chat;
            }
            console.log('[WS] Adding new message to chat');
            return {
              ...chat,
              messages: [...chat.messages, newMsg],
              last_activity: newMsg.timestamp,
              unread_count:
                chat.id !== state.activeChat?.id ? chat.unread_count + 1 : chat.unread_count,
            };
          });

          let updatedActiveChat = state.activeChat;
          if (state.activeChat?.id === targetChatId) {
            const messageExists = state.activeChat.messages.some((message) => message.id === newMsg.id);
            if (!messageExists) {
              console.log('[WS] Updating activeChat with new message');
              updatedActiveChat = {
                ...state.activeChat,
                messages: [...state.activeChat.messages, newMsg],
                last_activity: newMsg.timestamp,
              };
            }
          }

          console.log('[WS] Updated chats after NewMessage. Found:', foundChat);
          console.log('[WS] ActiveChat updated:', updatedActiveChat !== state.activeChat);

          const chatToSave = updatedChats.find((chat) => chat.id === targetChatId);
          if (chatToSave) {
            idbStorage.saveChat(chatToSave).catch((err) =>
              console.error('[IDB] Failed to save chat after NewMessage:', chatToSave.id, err),
            );
          }

          return {
            chats: updatedChats,
            activeChat: updatedActiveChat,
          };
        });
      }
    } else if (message.MessageAck) {
      const { message_id } = message.MessageAck;
      console.log('[WS] Processing MessageAck for message:', message_id);
      
      set(state => {
        // Find the temp ID that maps to this real message ID (if any)
        let tempIdForThisMessage: string | null = null;
        for (const [tempId, realId] of Object.entries(state.tempIdToRealId)) {
          if (realId === message_id) {
            tempIdForThisMessage = tempId;
            break;
          }
        }
        
        console.log('[WS] Message ACK for:', message_id, 'temp ID:', tempIdForThisMessage);
        
        const updatedChats = state.chats.map(chat => ({
          ...chat,
          messages: chat.messages.map(msg => {
            // Check if this message matches either the real ID or the temp ID
            if (msg.id === message_id || (tempIdForThisMessage && msg.id === tempIdForThisMessage)) {
              const oldStatus = msg.status;
              const newStatus: api.MessageStatus = api.MessageStatus.Delivered;
              console.log(`[WS] Updating message ${msg.id} status from ${oldStatus} to ${newStatus}`);
              return { ...msg, status: newStatus };
            }
            return msg;
          })
        }));
        
        // Update activeChat if it contains the message
        let updatedActiveChat = state.activeChat;
        if (state.activeChat) {
          updatedActiveChat = {
            ...state.activeChat,
            messages: state.activeChat.messages.map(msg => {
              if (msg.id === message_id || (tempIdForThisMessage && msg.id === tempIdForThisMessage)) {
                const oldStatus = msg.status;
                const newStatus: api.MessageStatus = api.MessageStatus.Delivered;
                console.log(`[WS] Updating activeChat message ${msg.id} status from ${oldStatus} to ${newStatus}`);
                return { ...msg, status: newStatus };
              }
              return msg;
            })
          };
        }
        
        return {
          chats: updatedChats,
          activeChat: updatedActiveChat
        }
      });
    } else if (message.StatusUpdate) {
      // Handle status updates
      set({ connectionStatus: 'connected' });
    } else if (message.Heartbeat) {
      // Handle heartbeat
      set({ connectionStatus: 'connected' });
    }
  },

  // Browser chat management
  createChatLink: async (singleUse: boolean) => {
    try {
      const chatId = get().activeChat?.id;
      if (!chatId) throw new Error('No active chat');
      
      return await api.create_chat_link({ 
        chat_id: chatId,
        single_use: singleUse 
      });
    } catch (error) {
      set({ error: 'Failed to create chat link' });
      throw error;
    }
  },

  loadChatKeys: async () => {
    try {
      const chatKeys = await api.get_chat_keys();
      set({ chatKeys });
    } catch (error) {
      set({ error: 'Failed to load chat keys' });
    }
  },

  revokeChatKey: async (key: string) => {
    try {
      await api.revoke_chat_key({ key });
      await get().loadChatKeys();
    } catch (error) {
      set({ error: 'Failed to revoke chat key' });
    }
  },

  // Error handling
  setError: (error: string | null) => set({ error }),
  clearError: () => set({ error: null }),
  
  // Reply functionality
  setReplyingTo: (message: any | null) => set({ replyingTo: message }),

  // Edit functionality
  setEditingMessage: (message: { id: string; content: string } | null) => set({ editingMessage: message }),
}));
