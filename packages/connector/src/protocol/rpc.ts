import type { JsonRpcRequest, JsonRpcNotification, ParsedMessage } from "./types.js";

let nextId = 1;

export function buildRequest(method: string, params: Record<string, unknown>): JsonRpcRequest {
  return { jsonrpc: "2.0", id: nextId++, method, params };
}

export function buildNotification(method: string, params: Record<string, unknown>): JsonRpcNotification {
  return { jsonrpc: "2.0", method, params };
}

export function parseMessage(raw: string): ParsedMessage {
  const msg = JSON.parse(raw);
  if (msg.error) {
    return { type: "error", id: msg.id, code: msg.error.code, message: msg.error.message };
  }
  if (msg.id !== undefined && msg.result !== undefined) {
    return { type: "response", id: msg.id, result: msg.result };
  }
  return { type: "notification", method: msg.method, params: msg.params ?? {} };
}

export function resetIdCounter(): void {
  nextId = 1;
}
