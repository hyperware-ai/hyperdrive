import { Message, ConversationMetadata } from '@caller-utils';
import { 
  WsClientMessage, 
  WsServerMessage,
  AuthMessage,
  ChatMessage,
  CancelMessage,
  PingMessage 
} from '../types/websocket';

export type MessageHandler = (message: WsServerMessage) => void;

class WebSocketService {
  private ws: WebSocket | null = null;
  private messageHandlers: Set<MessageHandler> = new Set();
  private reconnectTimeout: NodeJS.Timeout | null = null;
  private url: string = '';
  private isAuthenticated: boolean = false;
  
  connect(url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        resolve();
        return;
      }
      
      this.url = url;
      this.ws = new WebSocket(url);
      
      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.clearReconnectTimeout();
        resolve();
      };
      
      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        reject(error);
      };
      
      this.ws.onclose = () => {
        console.log('WebSocket disconnected');
        this.isAuthenticated = false;
        this.scheduleReconnect();
      };
      
      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data) as WsServerMessage;
          this.handleMessage(message);
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };
    });
  }
  
  private handleMessage(message: WsServerMessage) {
    // Notify all handlers
    this.messageHandlers.forEach(handler => handler(message));
  }
  
  authenticate(apiKey: string): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error('WebSocket not connected'));
        return;
      }
      
      // Set up one-time handler for auth response
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
      
      // Send auth message
      const authMsg: AuthMessage = {
        type: 'auth',
        apiKey
      };
      this.send(authMsg);
    });
  }
  
  sendChatMessage(messages: Message[], llmProvider?: string, model?: string, mcpServers?: string[], metadata?: ConversationMetadata): void {
    if (!this.isAuthenticated) {
      throw new Error('Not authenticated');
    }
    
    const chatMsg: ChatMessage = {
      type: 'chat',
      payload: {
        messages,
        llmProvider,
        model,
        mcpServers,
        metadata
      }
    };
    this.send(chatMsg);
  }
  
  sendCancel(): void {
    if (!this.isAuthenticated) {
      throw new Error('Not authenticated');
    }
    
    const cancelMsg: CancelMessage = {
      type: 'cancel'
    };
    this.send(cancelMsg);
  }
  
  send(data: WsClientMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket not connected');
    }
    
    this.ws.send(JSON.stringify(data));
  }
  
  addMessageHandler(handler: MessageHandler): void {
    this.messageHandlers.add(handler);
  }
  
  removeMessageHandler(handler: MessageHandler): void {
    this.messageHandlers.delete(handler);
  }
  
  disconnect(): void {
    this.clearReconnectTimeout();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.isAuthenticated = false;
  }
  
  private scheduleReconnect(): void {
    if (this.reconnectTimeout) return;
    
    this.reconnectTimeout = setTimeout(() => {
      console.log('Attempting to reconnect WebSocket...');
      this.connect(this.url).catch(error => {
        console.error('Reconnection failed:', error);
      });
    }, 3000);
  }
  
  private clearReconnectTimeout(): void {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }
  }
  
  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
  
  get isReady(): boolean {
    return this.isConnected && this.isAuthenticated;
  }
}

export const webSocketService = new WebSocketService();