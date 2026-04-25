import { isTauri } from "@tauri-apps/api/core";

type WsHandler = (data: Record<string, unknown>) => void;

const RECONNECT_MS = 2000;

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private port: number | null = null;
  private readonly handlers = new Map<string, Set<WsHandler>>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private shouldReconnect = false;

  connect(port: number): void {
    if (!isTauri()) {
      return;
    }
    if (this.port === port && this.ws?.readyState === WebSocket.OPEN) {
      return;
    }
    this.clearReconnectTimer();
    this.shouldReconnect = true;
    this.port = port;
    this.openSocket();
  }

  private openSocket(): void {
    if (this.port === null) {
      return;
    }
    const url = `ws://127.0.0.1:${this.port}/ws`;
    try {
      const socket = new WebSocket(url);
      this.ws = socket;

      socket.onopen = () => {
        if (import.meta.env.DEV) {
          console.debug("[TikClip] sidecar WebSocket connected", url);
        }
      };

      socket.onmessage = (ev) => {
        try {
          const msg = JSON.parse(String(ev.data)) as {
            type?: string;
            data?: Record<string, unknown>;
          };
          if (typeof msg.type !== "string" || !msg.data || typeof msg.data !== "object") {
            return;
          }
          const set = this.handlers.get(msg.type);
          if (!set) {
            return;
          }
          for (const h of set) {
            try {
              h(msg.data);
            } catch {
              /* ignore handler errors */
            }
          }
        } catch {
          /* ignore malformed messages */
        }
      };

      socket.onclose = () => {
        this.ws = null;
        if (this.shouldReconnect && this.port !== null) {
          this.reconnectTimer = setTimeout(() => {
            this.reconnectTimer = null;
            if (this.shouldReconnect && this.port !== null) {
              this.openSocket();
            }
          }, RECONNECT_MS);
        }
      };

      socket.onerror = () => {
        if (import.meta.env.DEV) {
          console.debug("[TikClip] sidecar WebSocket error (check Wry / CSP / port)");
        }
        socket.close();
      };
    } catch {
      this.ws = null;
      if (this.shouldReconnect && this.port !== null) {
        this.reconnectTimer = setTimeout(() => {
          this.reconnectTimer = null;
          this.openSocket();
        }, RECONNECT_MS);
      }
    }
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  disconnect(): void {
    this.shouldReconnect = false;
    this.clearReconnectTimer();
    this.port = null;
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
  }

  /** Subscribe to a sidecar event and return an unsubscribe function. */
  on(eventType: string, handler: WsHandler): () => void {
    let set = this.handlers.get(eventType);
    if (!set) {
      set = new Set();
      this.handlers.set(eventType, set);
    }
    set.add(handler);
    return () => {
      const s = this.handlers.get(eventType);
      if (!s) {
        return;
      }
      s.delete(handler);
      if (s.size === 0) {
        this.handlers.delete(eventType);
      }
    };
  }
}

export const wsClient = new WebSocketClient();
