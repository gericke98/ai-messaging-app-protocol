import WebSocket from "ws";
import type { Transport } from "./interface.js";

export class WebSocketTransport implements Transport {
  private ws: WebSocket | null = null;
  private messageHandler: ((data: string) => void) | null = null;
  private closeHandler: ((code: number, reason: string) => void) | null = null;
  private errorHandler: ((error: Error) => void) | null = null;

  async connect(url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(url);
      this.ws.on("open", () => resolve());
      this.ws.on("error", (err) => {
        if (this.errorHandler) this.errorHandler(err);
        reject(err);
      });
      this.ws.on("message", (data) => {
        if (this.messageHandler) this.messageHandler(data.toString());
      });
      this.ws.on("close", (code, reason) => {
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
