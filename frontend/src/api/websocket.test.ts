import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { WebSocketClient } from './websocket';

class MockWebSocket {
  static OPEN = 1;
  static CONNECTING = 0;
  static instances: MockWebSocket[] = [];

  readyState = MockWebSocket.CONNECTING;
  url: string;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  close() {
    this.readyState = 3;
  }
}

describe('WebSocketClient', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    localStorage.clear();
    vi.useFakeTimers();
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('does not connect without an auth token', () => {
    const client = new WebSocketClient();
    client.connect();

    expect(MockWebSocket.instances).toHaveLength(0);
  });

  it('stops reconnecting after auth-related close', () => {
    localStorage.setItem(
      'auth-storage',
      JSON.stringify({ state: { token: 'token-123' } }),
    );

    const client = new WebSocketClient();
    client.connect();

    expect(MockWebSocket.instances).toHaveLength(1);
    expect(MockWebSocket.instances[0].url).toContain('token-123');

    MockWebSocket.instances[0].onclose?.({ code: 1008 } as CloseEvent);
    vi.advanceTimersByTime(4000);

    expect(MockWebSocket.instances).toHaveLength(1);
  });

  it('emits connection state changes across the socket lifecycle', () => {
    localStorage.setItem(
      'auth-storage',
      JSON.stringify({ state: { token: 'token-123' } }),
    );

    const client = new WebSocketClient();
    const states: string[] = [];
    const unsubscribe = client.onConnectionStateChange((state) => {
      states.push(state);
    });

    client.connect();
    MockWebSocket.instances[0].onopen?.();
    MockWebSocket.instances[0].onclose?.({ code: 1006 } as CloseEvent);
    vi.advanceTimersByTime(3000);

    expect(states).toEqual(['idle', 'connecting', 'connected', 'reconnecting']);
    expect(MockWebSocket.instances).toHaveLength(2);
    unsubscribe();
  });
});
