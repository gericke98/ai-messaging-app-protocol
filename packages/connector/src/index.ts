// @ainm/connector — TypeScript SDK for connecting AI agents to AINM

export { AINMAgent } from "./agent.js";
export { createStreamHelper } from "./protocol/streaming.js";
export { generateKeypair, exportPublicKey } from "./identity/keypair.js";
export { signInit, signEnvelope, verifyEnvelope } from "./identity/signing.js";
export { createMemoryContext } from "./memory/context.js";
export { WebSocketTransport } from "./transport/websocket.js";

export type {
  AINMAgentConfig,
  MessageHandler,
  MessageContext,
  IncomingMessage,
  ContentBlock,
  ToolDefinition,
  ToolHandler,
  MemoryContext,
  MemoryNodeInput,
  StreamHelper,
} from "./protocol/types.js";

export type { Transport } from "./transport/interface.js";
