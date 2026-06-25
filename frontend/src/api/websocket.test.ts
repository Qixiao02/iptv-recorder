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

  describe('VITE_API_BASE_URL resolution', () => {
    afterEach(() => {
      vi.unstubAllEnvs();
    });

    it('uses window.location.host when VITE_API_BASE_URL is a relative path like /api', () => {
      vi.stubEnv('VITE_API_BASE_URL', '/api');
      localStorage.setItem(
        'auth-storage',
        JSON.stringify({ state: { token: 'token-123' } }),
      );

      const client = new WebSocketClient();
      client.connect();

      expect(MockWebSocket.instances).toHaveLength(1);
      expect(MockWebSocket.instances[0].url).toContain(
        `${window.location.host}/ws`,
      );
    });

    it('uses the absolute URL host when VITE_API_BASE_URL is a full URL', () => {
      vi.stubEnv('VITE_API_BASE_URL', 'https://api.example.com');
      localStorage.setItem(
        'auth-storage',
        JSON.stringify({ state: { token: 'token-123' } }),
      );

      const client = new WebSocketClient();
      client.connect();

      expect(MockWebSocket.instances).toHaveLength(1);
      expect(MockWebSocket.instances[0].url).toContain('api.example.com/ws');
      expect(MockWebSocket.instances[0].url).not.toContain(
        `${window.location.host}/ws`,
      );
    });

    it('falls back to window.location.host when VITE_API_BASE_URL is undefined', () => {
      vi.stubEnv('VITE_API_BASE_URL', '');
      localStorage.setItem(
        'auth-storage',
        JSON.stringify({ state: { token: 'token-123' } }),
      );

      const client = new WebSocketClient();
      client.connect();

      expect(MockWebSocket.instances).toHaveLength(1);
      expect(MockWebSocket.instances[0].url).toContain(
        `${window.location.host}/ws`,
      );
    });
  });
});
