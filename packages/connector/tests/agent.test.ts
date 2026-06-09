import { describe, it, expect, vi } from "vitest";
import { AINMAgent } from "../src/agent.js";
import { generateKeypair } from "../src/identity/keypair.js";
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

  it("ctx.sendMessage emits a FLAT ainm.message.send envelope (not wrapped under params.message)", async () => {
    // Regression: the server's ainm.message.send handler reads from/thread_id/to/content
    // FLAT off params and verifies the signature over params. Wrapping as {message: signed}
    // makes every field undefined AND breaks signature verification, so ctx.sendMessage
    // (incl. the runtime-A2A {to}) silently never reaches the server.
    const transport = mockTransport();
    const agent = new AINMAgent(
      { server: "wss://localhost:8080/ws", identity: "alpha@local", name: "Alpha", capabilities: ["text"] },
      transport
    );
    // Keypair is normally set during connect(); inject it for this unit test.
    (agent as unknown as { keypair: unknown }).keypair = await generateKeypair();

    agent.onMessage(async (_incoming, ctx) => {
      await ctx.sendMessage(
        [{ type: "text", version: "1.0", source: "agent", text: "hi", fallback: "hi" }],
        { to: "beta@local" }
      );
    });

    // Drive an inbound delivery. The deliver WRAPPER is correctly nested ({message,...}).
    await (
      agent as unknown as { handleMessageDelivery: (p: Record<string, unknown>) => Promise<void> }
    ).handleMessageDelivery({
      message: { id: "m1", from: "human@local", thread_id: "t1", type: "message", content: [] },
      sender_type: "human",
      verified: true,
    });

    expect(transport.sent.length).toBe(1);
    const frame = JSON.parse(transport.sent[0]);
    expect(frame.method).toBe("ainm.message.send");
    // The canonical send shape is the signed envelope FLAT as params.
    expect(frame.params.message).toBeUndefined();
    expect(frame.params.from).toBe("alpha@local");
    expect(frame.params.thread_id).toBe("t1");
    expect(frame.params.to).toBe("beta@local");
    expect(Array.isArray(frame.params.content)).toBe(true);
    expect(typeof frame.params.signature).toBe("string");
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

describe("buildMinimalContext", () => {
  it("sendMessage uses the agent's AINM address as from, not the public key", async () => {
    const t = mockTransport();
    const agent = new AINMAgent(
      { server: "ws://x", identity: "tool-agent@server.local", name: "T" },
      t,
    );
    (agent as any).keypair = await generateKeypair();

    const ctx = (agent as any).buildMinimalContext("th-1");
    await ctx.sendMessage([
      { type: "text", version: "1.0", source: "agent", text: "hi", fallback: "hi" },
    ]);

    const frame = JSON.parse(t.sent.at(-1)!);
    expect(frame.method).toBe("ainm.message.send");
    expect(frame.params.from).toBe("tool-agent@server.local");
  });
});
