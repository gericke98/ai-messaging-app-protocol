import { describe, it, expect } from "vitest";
import { performHandshake } from "../src/protocol/handshake.js";
import type { Transport } from "../src/transport/interface.js";
import { generateKeypair } from "../src/identity/keypair.js";

function mockTransport(): Transport & { sent: string[]; triggerMessage: (msg: string) => void } {
  let messageHandler: ((data: string) => void) | null = null;
  const sent: string[] = [];
  return {
    sent,
    triggerMessage(msg: string) { messageHandler?.(msg); },
    async connect() {},
    async send(data: string) { sent.push(data); },
    onMessage(handler) { messageHandler = handler; },
    onClose() {},
    onError() {},
    close() {},
    isConnected() { return true; },
  };
}

describe("handshake", () => {
  it("sends initialize, receives response, sends initialized", async () => {
    const transport = mockTransport();
    const kp = await generateKeypair();

    const handshakePromise = performHandshake(transport, {
      identity: "test-agent@server.local",
      name: "Test Agent",
      capabilities: ["text", "code"],
      keypair: kp,
    });

    // Wait for initialize to be sent
    await new Promise((r) => setTimeout(r, 10));
    expect(transport.sent.length).toBe(1);
    const initReq = JSON.parse(transport.sent[0]);
    expect(initReq.method).toBe("ainm.initialize");
    expect(initReq.params.identity).toBe("test-agent@server.local");
    expect(initReq.params.capabilities.participant_type).toBe("agent");

    // Server sends response
    transport.triggerMessage(JSON.stringify({
      jsonrpc: "2.0",
      id: initReq.id,
      result: {
        protocolVersion: "0.1.0",
        capabilities: { streaming: true },
        nonce: "abc123",
      },
    }));

    const result = await handshakePromise;
    expect(result.protocolVersion).toBe("0.1.0");

    // SDK should have sent ainm.initialized notification
    expect(transport.sent.length).toBe(2);
    const initialized = JSON.parse(transport.sent[1]);
    expect(initialized.method).toBe("ainm.initialized");
  });

  it("includes a card object with name, description, version, skills, and tools", async () => {
    const transport = mockTransport();
    const kp = await generateKeypair();

    const handshakePromise = performHandshake(transport, {
      identity: "coder@server.local",
      name: "Coder",
      description: "Writes and reviews code.",
      version: "2.1.0",
      capabilities: ["text", "code"],
      skills: [{ name: "review", description: "Reviews diffs." }],
      tools: [
        { name: "run_tests", description: "Runs the test suite.", parameters: { type: "object", properties: {} } },
      ],
      keypair: kp,
    });

    await new Promise((r) => setTimeout(r, 10));
    const initReq = JSON.parse(transport.sent[0]);
    expect(initReq.params.card).toBeDefined();
    expect(initReq.params.card.name).toBe("Coder");
    expect(initReq.params.card.description).toBe("Writes and reviews code.");
    expect(initReq.params.card.version).toBe("2.1.0");
    expect(initReq.params.card.skills).toEqual([{ name: "review", description: "Reviews diffs." }]);
    expect(initReq.params.card.tools).toEqual([
      { name: "run_tests", description: "Runs the test suite.", parameters: { type: "object", properties: {} } },
    ]);
    expect(initReq.params.capabilities.participant_type).toBe("agent");
    expect(initReq.params.capabilities.contentTypes).toEqual(["text", "code"]);

    transport.triggerMessage(JSON.stringify({ jsonrpc: "2.0", id: initReq.id, result: { protocolVersion: "0.1.0", capabilities: {} } }));
    await handshakePromise;
  });
});
