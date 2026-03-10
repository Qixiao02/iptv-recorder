import type {
  WsMessage,
  TaskUpdateData,
  TaskProgressData,
  ChannelStatusData,
  SystemAlertData,
} from '@/types';

export type WsEventHandler<T = unknown> = (data: T) => void;

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private url: string;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private handlers: Map<string, Set<WsEventHandler>> = new Map();

  constructor() {
    const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsHost = import.meta.env.VITE_API_BASE_URL
      ? new URL(import.meta.env.VITE_API_BASE_URL).host
      : window.location.host;
    this.url = `${wsProtocol}//${wsHost}/ws`;
  }

  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) return;

    const token = localStorage.getItem('token');
    const wsUrl = token ? `${this.url}?token=${encodeURIComponent(token)}` : this.url;
    this.ws = new WebSocket(wsUrl);

    this.ws.onopen = () => {
      console.log('WebSocket connected');
    };

    this.ws.onmessage = (event) => {
      try {
        const message: WsMessage = JSON.parse(event.data);
        this.emit(message.type, message.data);
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error);
      }
    };

    this.ws.onclose = () => {
      console.log('WebSocket disconnected, reconnecting in 3s...');
      this.reconnectTimer = setTimeout(() => this.connect(), 3000);
    };

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }

  disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
  }

  on<T = unknown>(event: string, handler: WsEventHandler<T>): () => void {
    if (!this.handlers.has(event)) {
      this.handlers.set(event, new Set());
    }
    this.handlers.get(event)!.add(handler as WsEventHandler);

    // 返回取消订阅函数
    return () => {
      this.handlers.get(event)?.delete(handler as WsEventHandler);
    };
  }

  private emit(event: string, data: unknown): void {
    this.handlers.get(event)?.forEach((handler) => {
      try {
        handler(data);
      } catch (error) {
        console.error(`Error in WebSocket handler for ${event}:`, error);
      }
    });
  }

  // 便捷方法
  onTaskUpdate(handler: WsEventHandler<TaskUpdateData>): () => void {
    return this.on('task.update', handler);
  }

  onTaskProgress(handler: WsEventHandler<TaskProgressData>): () => void {
    return this.on('task.progress', handler);
  }

  onChannelStatus(handler: WsEventHandler<ChannelStatusData>): () => void {
    return this.on('channel.status', handler);
  }

  onSystemAlert(handler: WsEventHandler<SystemAlertData>): () => void {
    return this.on('system.alert', handler);
  }
}

// 单例
export const wsClient = new WebSocketClient();
