import type { Transport } from "../transport/interface.js";
import type { Keypair } from "../identity/keypair.js";
import { signInit } from "../identity/signing.js";
import { buildRequest, buildNotification, parseMessage } from "./rpc.js";

export interface HandshakeOptions {
  identity: string;
  name: string;
  capabilities: string[];
  skills?: Array<{ name: string; description: string }>;
  keypair: Keypair;
}

export interface HandshakeResult {
  protocolVersion: string;
  capabilities: Record<string, unknown>;
  nonce?: string;
}

export async function performHandshake(
  transport: Transport,
  options: HandshakeOptions
): Promise<HandshakeResult> {
  const signature = await signInit(options.keypair, options.identity);

  const initReq = buildRequest("ainm.initialize", {
    protocolVersion: "0.1.0",
    identity: options.identity,
    signature,
    capabilities: {
      streaming: true,
      subThreads: true,
      contentTypes: options.capabilities,
      participant_type: "agent",
      toolVisibility: true,
    },
  });

  await transport.send(JSON.stringify(initReq));

  const result = await waitForResponse(transport, initReq.id);

  const initialized = buildNotification("ainm.initialized", {});
  await transport.send(JSON.stringify(initialized));

  return {
    protocolVersion: (result.protocolVersion as string) ?? "0.1.0",
    capabilities: (result.capabilities as Record<string, unknown>) ?? {},
    nonce: result.nonce as string | undefined,
  };
}

function waitForResponse(
  transport: Transport,
  expectedId: number
): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error("Handshake timeout (10s)")),
      10000
    );

    transport.onMessage((data: string) => {
      const msg = parseMessage(data);
      if (msg.type === "response" && msg.id === expectedId) {
        clearTimeout(timeout);
        resolve(msg.result);
      } else if (msg.type === "error" && msg.id === expectedId) {
        clearTimeout(timeout);
        reject(new Error(`Handshake error ${msg.code}: ${msg.message}`));
      }
    });
  });
}
