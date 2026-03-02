import { WsClientMessage, WsServerMessage } from '../types/chat';
import { getChatBasePath } from './chatBase';

export class ChatWebSocket {
  private ws: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private onMessageCallback: ((msg: WsServerMessage) => void) | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private isPageVisible = !document.hidden;

  connect(onMessage: (msg: WsServerMessage) => void) {
    this.onMessageCallback = onMessage;

    // Determine WebSocket URL with chat base
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    const basePath = getChatBasePath();
    const wsUrl = `${protocol}//${host}${basePath}/ws`;

    try {
      this.ws = new WebSocket(wsUrl);

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.reconnectAttempts = 0;
        this.startHeartbeat();
        this.setupVisibilityTracking();
        
        // Send initial heartbeat to trigger chat sync from backend
        console.log('Sending initial heartbeat to trigger chat sync');
        this.send({ Heartbeat: null });
        
        // Send initial visibility status
        this.sendVisibilityStatus();
      };

      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          if (this.onMessageCallback) {
            this.onMessageCallback(message);
          }
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
      };

      this.ws.onclose = () => {
        console.log('WebSocket disconnected');
        this.stopHeartbeat();
        this.handleReconnect();
      };
    } catch (error) {
      console.error('Failed to connect WebSocket:', error);
      this.handleReconnect();
    }
  }

  send(message: WsClientMessage) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    } else {
      console.warn('WebSocket not connected, queuing message');
      // Could implement message queuing here
    }
  }

  disconnect() {
    this.stopHeartbeat();
    this.cleanupVisibilityTracking();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private handleReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnection attempts reached');
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);

    this.reconnectTimer = setTimeout(() => {
      if (this.onMessageCallback) {
        this.connect(this.onMessageCallback);
      }
    }, delay);
  }

  private startHeartbeat() {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      this.send({ Heartbeat: null });
    }, 30000); // Send heartbeat every 30 seconds
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  private setupVisibilityTracking() {
    // Set up page visibility tracking
    const handleVisibilityChange = () => {
      const wasVisible = this.isPageVisible;
      this.isPageVisible = !document.hidden;
      
      if (wasVisible !== this.isPageVisible) {
        console.log('Page visibility changed:', this.isPageVisible ? 'visible' : 'hidden');
        this.sendVisibilityStatus();
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    
    // Store handler for cleanup
    (this as any).visibilityHandler = handleVisibilityChange;
  }

  private cleanupVisibilityTracking() {
    if ((this as any).visibilityHandler) {
      document.removeEventListener('visibilitychange', (this as any).visibilityHandler);
      delete (this as any).visibilityHandler;
    }
  }

  private sendVisibilityStatus() {
    const status = this.isPageVisible ? 'active' : 'inactive';
    this.send({ UpdateStatus: { status } });
  }
}
