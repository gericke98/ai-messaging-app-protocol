import WebSocket from "ws";
import type { Transport } from "./interface.js";

export interface WebSocketTransportOptions {
  /** Connection attempts beyond the first (default 3). */
  retries?: number;
  /** Base backoff in ms; doubles per attempt (default 500). */
  backoffMs?: number;
}

export class WebSocketTransport implements Transport {
  private ws: WebSocket | null = null;
  private messageHandler: ((data: string) => void) | null = null;
  private closeHandler: ((code: number, reason: string) => void) | null = null;
  private errorHandler: ((error: Error) => void) | null = null;
  private lastUrl: string | null = null;
  private readonly retries: number;
  private readonly backoffMs: number;

  constructor(options: WebSocketTransportOptions = {}) {
    this.retries = options.retries ?? 3;
    this.backoffMs = options.backoffMs ?? 500;
  }

  /**
   * Connect with retries: the initial attempt plus up to `retries` more,
   * backing off exponentially, before surfacing the last error.
   */
  async connect(url: string): Promise<void> {
    this.lastUrl = url;
    let lastError: Error = new Error("connect failed");
    for (let attempt = 0; attempt <= this.retries; attempt++) {
      if (attempt > 0) {
        const delay = this.backoffMs * 2 ** (attempt - 1);
        await new Promise((r) => setTimeout(r, delay));
      }
      try {
        await this.dial(url);
        return;
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));
      }
    }
    throw lastError;
  }

  /** Re-dial the most recent URL with the same retry/backoff policy. */
  async reconnect(): Promise<void> {
    if (!this.lastUrl) {
      throw new Error("reconnect() before any connect()");
    }
    return this.connect(this.lastUrl);
  }

  private dial(url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(url);
      let settled = false;
      ws.on("open", () => {
        settled = true;
        this.ws = ws;
        resolve();
      });
      ws.on("error", (err) => {
        if (this.errorHandler) this.errorHandler(err);
        if (!settled) {
          settled = true;
          reject(err);
        }
      });
      ws.on("message", (data) => {
        if (this.messageHandler) this.messageHandler(data.toString());
      });
      ws.on("close", (code, reason) => {
        if (this.closeHandler) this.closeHandler(code, reason.toString());
      });
    });
  }

  async send(data: string): Promise<void> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("WebSocket not connected");
    }
    return new Promise((resolve, reject) => {
      this.ws!.send(data, (err) => (err ? reject(err) : resolve()));
    });
  }

  onMessage(handler: (data: string) => void): void {
    this.messageHandler = handler;
  }

  onClose(handler: (code: number, reason: string) => void): void {
    this.closeHandler = handler;
  }

  onError(handler: (error: Error) => void): void {
    this.errorHandler = handler;
  }

  close(): void {
    this.ws?.close();
    this.ws = null;
  }

  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}
