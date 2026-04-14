import { describe, it, expect, vi } from "vitest";
import { AINMAgent } from "../src/agent.js";
import type { Transport } from "../src/transport/interface.js";

function mockTransport(): Transport & {
  sent: string[];
  triggerMessage: (msg: string) => void;
  triggerClose: (code: number) => void;
} {
  let messageHandler: ((data: string) => void) | null = null;
  let closeHandler: ((code: number, reason: string) => void) | null = null;
  const sent: string[] = [];

  return {
    sent,
    triggerMessage(msg: string) { messageHandler?.(msg); },
    triggerClose(code: number) { closeHandler?.(code, ""); },
    async connect() {},
    async send(data: string) { sent.push(data); },
    onMessage(handler) { messageHandler = handler; },
    onClose(handler) { closeHandler = handler; },
    onError() {},
    close() {},
    isConnected() { return true; },
  };
}

describe("AINMAgent", () => {
  it("creates an agent with config", () => {
    const agent = new AINMAgent({
      server: "wss://localhost:8080/ws",
      identity: "test-agent",
      name: "Test Agent",
      capabilities: ["text"],
    });
    expect(agent).toBeDefined();
  });

  it("registers a message handler", () => {
    const agent = new AINMAgent({
      server: "wss://localhost:8080/ws",
      identity: "test-agent",
      name: "Test Agent",
    });
    const handler = vi.fn();
    agent.onMessage(handler);
    expect(agent).toBeDefined();
  });

  it("registers tools", () => {
    const agent = new AINMAgent({
      server: "wss://localhost:8080/ws",
      identity: "test-agent",
      name: "Test Agent",
    });
    agent.registerTool(
      "search",
      { description: "Search docs", parameters: { query: { type: "string" } } },
      async (params) => ({ results: [] })
    );
    expect(agent).toBeDefined();
  });
});
