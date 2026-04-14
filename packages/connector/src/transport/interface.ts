export interface Transport {
  connect(url: string): Promise<void>;
  send(data: string): Promise<void>;
  onMessage(handler: (data: string) => void): void;
  onClose(handler: (code: number, reason: string) => void): void;
  onError(handler: (error: Error) => void): void;
  close(): void;
  isConnected(): boolean;
}
