import { create } from 'zustand';
import { Chat as api } from '#caller-utils';
import { spiderWebSocketService, MessageHandler } from '../spider/websocket';
import {
  SpiderMessage,
  SpiderConversationMetadata,
  WsServerMessage,
} from '../types/spider';
import { idbStorage } from '../utils/indexeddb';

// Debug logging
const DEBUG = true;
const log = (...args: unknown[]) => DEBUG && console.log('[Spider Store]', ...args);

const generateMessageId = () =>
  `spider-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;

const normalizeContent = (content: SpiderMessage['content'] | unknown): SpiderMessage['content'] => {
  if (!content || typeof content !== 'object') {
    return { text: typeof content === 'string' ? content : undefined };
  }

  const raw = content as Record<string, unknown>;
  if (typeof raw.Text === 'string') {
    return { text: raw.Text };
  }
  if (typeof raw.BaseSixFourAudio === 'string') {
    return { base_six_four_audio: raw.BaseSixFourAudio };
  }
  if (Array.isArray(raw.Audio)) {
    return { audio: raw.Audio as number[] };
  }

  const typed = content as SpiderMessage['content'];
  return {
    text: typed.text ?? undefined,
    audio: typed.audio ?? undefined,
    base_six_four_audio: typed.base_six_four_audio ?? undefined,
  };
};

const toLocalMessage = (message: SpiderMessage | api.SpiderMessage): SpiderMessage => {
  const raw = message as SpiderMessage;
  const toolCallsJson =
    'tool_calls_json' in message ? message.tool_calls_json : raw.toolCallsJson ?? null;
  const toolResultsJson =
    'tool_results_json' in message ? message.tool_results_json : raw.toolResultsJson ?? null;

  return {
    id: raw.id,
    role: message.role,
    content: normalizeContent((message as SpiderMessage).content),
    replyTo: raw.replyTo ?? null,
    toolCallsJson,
    toolResultsJson,
    timestamp: message.timestamp,
    preservedText: raw.preservedText,
    hidden: raw.hidden,
  };
};

const toApiMessage = (message: SpiderMessage): api.SpiderMessage => ({
  role: message.role,
  content: {
    text: message.content.text ?? null,
    audio: message.content.audio ?? null,
    base_six_four_audio: message.content.base_six_four_audio ?? null,
  },
  tool_calls_json: message.toolCallsJson ?? null,
  tool_results_json: message.toolResultsJson ?? null,
  timestamp: message.timestamp,
});

const normalizeMessage = (
  message: SpiderMessage | api.SpiderMessage,
  overrides: Partial<SpiderMessage> = {},
) => {
  const localMessage = toLocalMessage(message);
  const id = overrides.id ?? localMessage.id ?? generateMessageId();
  const replyTo = overrides.replyTo ?? localMessage.replyTo ?? null;
  return {
    ...localMessage,
    ...overrides,
    id,
    replyTo,
    content: normalizeContent(overrides.content ?? localMessage.content),
  };
};

const buildReplyChain = (messages: SpiderMessage[], replyToId: string) => {
  const messageById = new Map(
    messages.flatMap((msg) => (msg.id ? [[msg.id, msg]] : [])),
  );
  const chain: SpiderMessage[] = [];
  const visited = new Set<string>();
  let currentId: string | null = replyToId;

  while (currentId) {
    if (visited.has(currentId)) break;
    const message = messageById.get(currentId);
    if (!message) break;
    chain.push(message);
    visited.add(currentId);
    currentId = message.replyTo ?? null;
  }

  return chain.reverse();
};

const isSameMessage = (left: SpiderMessage, right: SpiderMessage) => {
  if (left.role !== right.role || left.timestamp !== right.timestamp) {
    return false;
  }

  return JSON.stringify(left.content ?? {}) === JSON.stringify(right.content ?? {});
};

interface SpiderStore {
  // State
  messages: SpiderMessage[];
  isConnected: boolean;
  isAuthenticated: boolean;
  isLoading: boolean;
  apiKey: string | null;
  conversationIdByRoot: Record<string, string>;
  error: string | null;
  streamingMessage: string | null;
  spiderAvailable: boolean;
  replyingTo: SpiderMessage | null;
  pendingConversationRootId: string | null;
  pendingReplyToId: string | null;
  isHistoryLoaded: boolean;
  isHistorySaving: boolean;

  // Actions
  connect: () => Promise<void>;
  disconnect: () => void;
  sendMessage: (content: string, replyToId?: string | null) => Promise<void>;
  cancelRequest: () => void;
  clearMessages: () => void;
  checkStatus: () => Promise<api.SpiderStatusInfo | null>;
  setError: (error: string | null) => void;
  setReplyingTo: (message: SpiderMessage | null) => void;
  loadHistory: () => Promise<void>;
  persistHistory: () => Promise<void>;
}

export const useSpiderStore = create<SpiderStore>((set, get) => ({
  // Initial state
  messages: [],
  isConnected: false,
  isAuthenticated: false,
  isLoading: false,
  apiKey: null,
  conversationIdByRoot: {},
  error: null,
  streamingMessage: null,
  spiderAvailable: false,
  replyingTo: null,
  pendingConversationRootId: null,
  pendingReplyToId: null,
  isHistoryLoaded: false,
  isHistorySaving: false,

  checkStatus: async () => {
    log('Checking Spider status...');
    try {
      const status = await api.spider_status();
      log('Spider status result:', status);
      set({ spiderAvailable: status.spider_available });
      return status;
    } catch (error) {
      log('Spider status check failed:', error);
      console.error('[Spider] Failed to check status:', error);
      set({ spiderAvailable: false });
      return null;
    }
  },

  connect: async () => {
    const state = get();
    log('connect() called, current state:', { isConnected: state.isConnected, isAuthenticated: state.isAuthenticated });
    if (state.isConnected && state.isAuthenticated) {
      log('Already connected and authenticated, skipping');
      return;
    }

    set({ isLoading: true, error: null });

    try {
      // Get API key from backend
      log('Calling spider_connect(false)...');
      const connectResult = await api.spider_connect(false);
      const apiKey = connectResult.api_key;
      log('Got API key:', apiKey.slice(0, 8) + '...');
      set({ apiKey });

      // Build WebSocket URL
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsUrl = `${protocol}//${window.location.host}/spider:spider:sys/ws`;
      log('Connecting WebSocket to:', wsUrl);

      // Connect WebSocket
      await spiderWebSocketService.connect(wsUrl);
      log('WebSocket connected successfully');
      set({ isConnected: true });

      // Authenticate
      log('Authenticating with API key...');
      await spiderWebSocketService.authenticate(apiKey);
      log('Authentication successful');
      set({ isAuthenticated: true });

      // Set up message handler
      log('Setting up message handler');
      const messageHandler: MessageHandler = (message: WsServerMessage) => {
        handleSpiderMessage(message, set, get);
      };
      spiderWebSocketService.addMessageHandler(messageHandler);

      set({ isLoading: false });
      log('Connection complete, isLoading set to false');
    } catch (error) {
      log('Connection failed:', error);
      console.error('[Spider] Connection failed:', error);
      set({
        isLoading: false,
        isConnected: false,
        isAuthenticated: false,
        error: error instanceof Error ? error.message : 'Connection failed',
      });
    }
  },

  disconnect: () => {
    spiderWebSocketService.disconnect();
    set({
      isConnected: false,
      isAuthenticated: false,
      streamingMessage: null,
    });
  },

  sendMessage: async (content: string, replyToId: string | null = null) => {
    log('sendMessage() called with content:', content);
    const state = get();

    if (!state.isAuthenticated) {
      log('Not authenticated, calling connect() first');
      await get().connect();
    }

    // Add user message to local state
    const userMessage: SpiderMessage = normalizeMessage({
      role: 'user',
      content: { text: content },
      timestamp: Math.floor(Date.now() / 1000),
      replyTo: replyToId,
    });

    log('Adding user message to state, setting isLoading=true');
    set((state) => ({
      messages: [...state.messages, userMessage],
      isLoading: true,
      error: null,
      streamingMessage: '',
      pendingReplyToId: userMessage.id ?? null,
    }));
    get().persistHistory().catch(() => null);

    // Build metadata
    const metadata: SpiderConversationMetadata = {
      startTime: new Date().toISOString(),
      client: 'homepage-chat',
      fromStt: false,
    };

    // Send via WebSocket
    try {
      const replyChain = replyToId ? buildReplyChain(get().messages, replyToId) : [];
      const messages = [...replyChain, userMessage];
      const conversationRootId = replyChain[0]?.id ?? userMessage.id ?? null;
      const conversationId = replyToId && conversationRootId
        ? get().conversationIdByRoot[conversationRootId]
        : undefined;
      log('Calling sendChatMessage with', messages.length, 'messages');
      log('Messages:', messages);
      log('ConversationId:', conversationId);
      set({ pendingConversationRootId: conversationRootId });
      spiderWebSocketService.sendChatMessage(
        messages,
        null, // llmProvider
        null, // model
        null, // mcpServers
        metadata,
        conversationId
      );
      log('sendChatMessage completed (no error)');
    } catch (error) {
      log('sendChatMessage failed:', error);
      console.error('[Spider] Failed to send message:', error);
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : 'Failed to send message',
        pendingConversationRootId: null,
        pendingReplyToId: null,
      });
    }
  },

  cancelRequest: () => {
    log('cancelRequest() called');
    try {
      spiderWebSocketService.sendCancel();
      log('Cancel sent, setting isLoading=false');
      set({
        isLoading: false,
        streamingMessage: null,
        pendingConversationRootId: null,
        pendingReplyToId: null,
      });
    } catch (error) {
      log('Cancel failed:', error);
      console.error('[Spider] Failed to cancel:', error);
    }
  },

  clearMessages: () => {
    set({
      messages: [],
      conversationIdByRoot: {},
      streamingMessage: null,
      replyingTo: null,
      pendingConversationRootId: null,
      pendingReplyToId: null,
      isHistoryLoaded: false,
    });
    get().persistHistory().catch(() => null);
  },

  setError: (error: string | null) => set({ error }),
  setReplyingTo: (message: SpiderMessage | null) => set({ replyingTo: message }),
  loadHistory: async () => {
    const state = get();
    if (state.isHistoryLoaded || state.messages.length > 0) {
      return;
    }

    try {
      const cachedMessages = await idbStorage.loadSpiderMessages();
      if (cachedMessages.length > 0) {
        set({
          messages: cachedMessages.map((message) => normalizeMessage(message)),
          isHistoryLoaded: true,
        });
      }

      const history = await api.spider_get_history();
      if (!history || !Array.isArray(history.messages) || history.messages.length === 0) {
        set({ isHistoryLoaded: true });
        return;
      }

      const normalizedMessages = history.messages.map((msg) =>
        normalizeMessage(msg, { replyTo: null }),
      );
      await idbStorage.saveSpiderMessages(normalizedMessages);
      set({
        messages: normalizedMessages,
        isHistoryLoaded: true,
      });
    } catch (error) {
      console.error('[Spider] Failed to load conversation history:', error);
      set({
        isHistoryLoaded: true,
        error: error instanceof Error ? error.message : 'Failed to load conversation history',
      });
    }
  },
  persistHistory: async () => {
    const state = get();
    if (state.isHistorySaving) {
      return;
    }

    const payloadMessages = state.messages.map(toApiMessage);
    set({ isHistorySaving: true });
    try {
      await api.spider_set_history({ messages: payloadMessages });
      await idbStorage.saveSpiderMessages(state.messages);
    } catch (error) {
      console.error('[Spider] Failed to persist conversation history:', error);
    } finally {
      set({ isHistorySaving: false });
    }
  },
}));

// External message handler function
function handleSpiderMessage(
  message: WsServerMessage,
  set: (partial: Partial<SpiderStore> | ((state: SpiderStore) => Partial<SpiderStore>)) => void,
  get: () => SpiderStore
) {
  log('handleSpiderMessage received:', message.type, message);

  switch (message.type) {
    case 'auth_success':
      log('Auth success, setting isAuthenticated=true');
      set({ isAuthenticated: true, error: null });
      break;

    case 'auth_error':
      log('Auth error:', message.error);
      set({
        isAuthenticated: false,
        error: message.error || 'Authentication failed',
      });
      break;

    case 'stream':
      // Streaming response - update the streaming message
      log('Stream update, length:', message.message?.length);
      set({ streamingMessage: message.message });
      break;

    case 'message':
      // Complete message received
      log('Message received:', message.message);
      const replyToId = get().pendingReplyToId;
      const assistantMessage = normalizeMessage(message.message, { replyTo: replyToId ?? null });
      set((state) => ({
        messages: [...state.messages, assistantMessage],
        streamingMessage: null,
      }));
      get().persistHistory().catch(() => null);
      break;

    case 'chat_complete':
      // Chat completed - update conversation ID and messages
      log('Chat complete received');
      const result = message.payload;
      const rootId = get().pendingConversationRootId;
      const responseReplyToId = get().pendingReplyToId;
      log('Chat complete payload:', result);
      set((state) => ({
        conversationIdByRoot: rootId
          ? { ...state.conversationIdByRoot, [rootId]: result.conversationId }
          : state.conversationIdByRoot,
        isLoading: false,
        streamingMessage: null,
        pendingConversationRootId: null,
        pendingReplyToId: null,
      }));
      log('Set isLoading=false');

      if (result.allMessages && result.allMessages.length > 0) {
        log('Ignoring allMessages to preserve local timeline:', result.allMessages.length);
      }

      const responseMessage = normalizeMessage(result.response, {
        replyTo: responseReplyToId ?? null,
      });
      log('Adding response to messages:', responseMessage);
      set((state) => {
        const hasResponse = state.messages.some((msg) => isSameMessage(msg, responseMessage));
        if (hasResponse) {
          return {};
        }
        return { messages: [...state.messages, responseMessage] };
      });
      get().persistHistory().catch(() => null);
      break;

    case 'error':
      log('Error received:', message.error);
      console.error('[Spider] Server error:', message.error);
      set({
        isLoading: false,
        error: message.error,
        streamingMessage: null,
        pendingConversationRootId: null,
        pendingReplyToId: null,
      });
      break;

    case 'status':
      log('Status received:', message.status);
      if (message.status === 'cancelled') {
        log('Status cancelled, setting isLoading=false');
        set({
          isLoading: false,
          streamingMessage: null,
          pendingConversationRootId: null,
          pendingReplyToId: null,
        });
      }
      break;

    case 'pong':
      // Keepalive response - no action needed
      log('Pong received');
      break;

    default:
      log('Unknown message type:', (message as any).type);
  }
}
