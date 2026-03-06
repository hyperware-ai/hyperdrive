import {
  SpiderConversationMetadata,
  SpiderMessage,
  WsClientMessage,
  WsServerMessage,
} from '../types/spider';

// Debug logging
const DEBUG = true;
const log = (...args: unknown[]) => DEBUG && console.log('[Spider WS]', ...args);

export type MessageHandler = (message: WsServerMessage) => void;

class WebSocketService {
  private ws: WebSocket | null = null;
  private messageHandlers: Set<MessageHandler> = new Set();
  private reconnectTimeout: number | null = null;
  private url = '';
  private isAuthenticated = false;
  private pingInterval: number | null = null;

  connect(url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        resolve();
        return;
      }

      this.url = url;
      log('Connecting to:', url);
      this.ws = new WebSocket(url);

      this.ws.onopen = () => {
        log('WebSocket connected to', url);
        this.clearReconnectTimeout();
        this.startPingInterval();
        resolve();
      };

      this.ws.onerror = (error) => {
        log('WebSocket error:', error);
        reject(error);
      };

      this.ws.onclose = (event) => {
        log('WebSocket closed, code:', event.code, 'reason:', event.reason);
        this.isAuthenticated = false;
        this.stopPingInterval();
        this.scheduleReconnect();
      };

      this.ws.onmessage = (event) => {
        try {
          log('Raw message received:', event.data);
          const message = JSON.parse(event.data) as WsServerMessage;
          log('Parsed message:', message.type, message);
          this.handleMessage(message);
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };
    });
  }

  private handleMessage(message: WsServerMessage) {
    this.messageHandlers.forEach((handler) => handler(message));
  }

  authenticate(apiKey: string): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error('WebSocket not connected'));
        return;
      }

      const authHandler = (message: WsServerMessage) => {
        if (message.type === 'auth_success') {
          this.isAuthenticated = true;
          this.removeMessageHandler(authHandler);
          resolve();
        } else if (message.type === 'auth_error') {
          this.removeMessageHandler(authHandler);
          reject(new Error(message.error || 'Authentication failed'));
        }
      };

      this.addMessageHandler(authHandler);

      log('Sending auth with key:', apiKey.slice(0, 8) + '...');
      const authMsg: WsClientMessage = {
        type: 'auth',
        apiKey,
      };

      this.send(authMsg);
    });
  }

  sendChatMessage(
    messages: SpiderMessage[],
    llmProvider?: string | null,
    model?: string | null,
    mcpServers?: string[] | null,
    metadata?: SpiderConversationMetadata | null,
    conversationId?: string,
  ): void {
    if (!this.isAuthenticated) {
      throw new Error('Not authenticated');
    }

    const encodeContent = (content: SpiderMessage['content']) => {
      if (!content) return { Text: '' };
      if ((content as any).base_six_four_audio) {
        return { BaseSixFourAudio: (content as any).base_six_four_audio };
      }
      if ((content as any).audio) {
        return { Audio: (content as any).audio };
      }
      if ((content as any).text !== undefined && (content as any).text !== null) {
        return { Text: (content as any).text };
      }
      return { Text: '' };
    };

    const wireMessages = messages.map(({ id, replyTo, content, ...rest }) => ({
      ...rest,
      content: encodeContent(content),
    })) as unknown as SpiderMessage[];

    const chatMsg: WsClientMessage = {
      type: 'chat',
      payload: {
        messages: wireMessages,
        llmProvider,
        model,
        mcpServers,
        metadata,
        conversationId,
      },
    };

    log('Sending chat message:', chatMsg);
    this.send(chatMsg);
  }

  sendCancel(): void {
    if (!this.isAuthenticated) {
      log('Cancel failed: not authenticated');
      throw new Error('Not authenticated');
    }

    log('Sending cancel request');
    const cancelMsg: WsClientMessage = {
      type: 'cancel',
    };
    this.send(cancelMsg);
  }

  send(data: WsClientMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      log('Send failed: WebSocket not connected, state:', this.ws?.readyState);
      throw new Error('WebSocket not connected');
    }

    const json = JSON.stringify(data);
    log('Raw send:', json);
    this.ws.send(json);
  }

  addMessageHandler(handler: MessageHandler): void {
    this.messageHandlers.add(handler);
  }

  removeMessageHandler(handler: MessageHandler): void {
    this.messageHandlers.delete(handler);
  }

  disconnect(): void {
    this.clearReconnectTimeout();
    this.stopPingInterval();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.isAuthenticated = false;
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimeout) return;

    this.reconnectTimeout = window.setTimeout(() => {
      this.connect(this.url).catch((error) => {
        console.error('Reconnection failed:', error);
      });
    }, 3000);
  }

  private clearReconnectTimeout(): void {
    if (this.reconnectTimeout) {
      window.clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }
  }

  private startPingInterval(): void {
    this.stopPingInterval();

    if (this.isConnected) {
      const pingMsg: WsClientMessage = { type: 'ping' };
      try {
        this.send(pingMsg);
      } catch (error) {
        console.error('Failed to send ping:', error);
      }
    }

    this.pingInterval = window.setInterval(() => {
      if (this.isConnected) {
        const pingMsg: WsClientMessage = { type: 'ping' };
        try {
          this.send(pingMsg);
        } catch (error) {
          console.error('Failed to send ping:', error);
        }
      }
    }, 30000);
  }

  private stopPingInterval(): void {
    if (this.pingInterval) {
      window.clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }

  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  get isReady(): boolean {
    return this.isConnected && this.isAuthenticated;
  }
}

export const spiderWebSocketService = new WebSocketService();
