export interface AINMAgentConfig {
  server: string;
  identity: string;
  name: string;
  description?: string;
  version?: string;
  /** Opt out of agent-to-agent (peer) messages. Defaults to true (cooperative). */
  acceptsA2A?: boolean;
  capabilities?: string[];
  skills?: Array<{ name: string; description: string }>;
  privateKey?: Uint8Array;
  reconnect?: boolean;
  reconnectMaxDelay?: number;
}

export interface IncomingMessage {
  id: string;
  from: string;
  threadId: string;
  timestamp: string;
  text: string;
  contentBlocks: ContentBlock[];
  /** Server-stamped sender classification. "agent" when another agent sent this. */
  senderType: "human" | "agent" | "system";
  /** The canonical recipient the server resolved for this message (or null). */
  to: string | null;
  /** Convenience: true when `to` equals this agent's own identity. */
  addressed: boolean;
  /** Whether the server verified the sender's signature. */
  verified: boolean;
  raw: Record<string, unknown>;
}

export interface ContentBlock {
  type: string;
  version: string;
  source: string;
  [key: string]: unknown;
}

export interface MessageContext {
  threadId: string;
  from: string;
  participants: string[];
  parentThreadId?: string;
  stream: () => StreamHelper;
  memory: MemoryContext;
  sendMessage: (content: ContentBlock[], opts?: { to?: string }) => Promise<void>;
  emitToolStarted: (name: string, input: unknown) => void;
  emitToolProgress: (name: string, progress: number) => void;
  emitToolCompleted: (name: string, result: unknown) => void;
}

export interface StreamHelper {
  delta: (text: string) => void;
  end: () => void;
}

export interface MemoryContext {
  getBriefing: () => Promise<unknown>;
  queryGraph: (query?: string, limit?: number) => Promise<unknown>;
  contribute: (node: MemoryNodeInput) => Promise<string>;
}

export interface MemoryNodeInput {
  type: string;
  category: string;
  label: string;
  confidence: number;
  properties?: Record<string, unknown>;
  tags?: string[];
}

export type MessageHandler = (
  message: IncomingMessage,
  ctx: MessageContext
) => Promise<string | void>;

export interface ToolDefinition {
  description: string;
  parameters: Record<string, unknown>;
  needsApproval?: (params: Record<string, unknown>) => boolean;
}

export type ToolHandler = (
  params: Record<string, unknown>,
  ctx: MessageContext
) => Promise<unknown>;

export interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params: Record<string, unknown>;
}

export interface JsonRpcNotification {
  jsonrpc: "2.0";
  method: string;
  params: Record<string, unknown>;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number;
  result?: unknown;
  error?: { code: number; message: string };
}

export type ParsedMessage =
  | { type: "response"; id: number; result: Record<string, unknown> }
  | { type: "error"; id: number; code: number; message: string }
  | { type: "notification"; method: string; params: Record<string, unknown> };
