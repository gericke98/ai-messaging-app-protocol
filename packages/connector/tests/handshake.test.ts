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
});
