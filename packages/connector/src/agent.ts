import type { Transport } from "./transport/interface.js";
import { WebSocketTransport } from "./transport/websocket.js";
import { buildRequest, buildNotification, parseMessage } from "./protocol/rpc.js";
import { performHandshake } from "./protocol/handshake.js";
import type { HandshakeResult } from "./protocol/handshake.js";
import { createStreamHelper } from "./protocol/streaming.js";
import { createMemoryContext } from "./memory/context.js";
import { generateKeypair } from "./identity/keypair.js";
import type { Keypair } from "./identity/keypair.js";
import { signEnvelope } from "./identity/signing.js";
import type {
  AINMAgentConfig,
  MessageHandler,
  MessageContext,
  IncomingMessage,
  ContentBlock,
  StreamHelper,
  ToolDefinition,
  ToolHandler,
} from "./protocol/types.js";
import { randomUUID } from "node:crypto";

interface PendingRpc {
  resolve: (result: Record<string, unknown>) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export class AINMAgent {
  private readonly config: AINMAgentConfig;
  private readonly transport: Transport;
  private messageHandler: MessageHandler | null = null;
  private readonly tools = new Map<string, { definition: ToolDefinition; handler: ToolHandler }>();
  private keypair: Keypair | null = null;
  private handshakeResult: HandshakeResult | null = null;
  private readonly pendingRpcs = new Map<number, PendingRpc>();
  private readonly RPC_TIMEOUT_MS = 30_000;

  constructor(config: AINMAgentConfig, transport?: Transport) {
    this.config = config;
    this.transport = transport ?? new WebSocketTransport();
  }

  /**
   * Register a handler for incoming messages.
   */
  onMessage(handler: MessageHandler): void {
    this.messageHandler = handler;
  }

  /**
   * Register a tool that the agent can expose.
   */
  registerTool(name: string, definition: ToolDefinition, handler: ToolHandler): void {
    this.tools.set(name, { definition, handler });
  }

  /**
   * Connect to the server, perform handshake, and start the message loop.
   */
  async connect(): Promise<void> {
    // Generate or load keypair
    if (this.config.privateKey) {
      const { getPublicKeyAsync } = await import("@noble/ed25519");
      const publicKey = await getPublicKeyAsync(this.config.privateKey);
      this.keypair = { privateKey: this.config.privateKey, publicKey };
    } else {
      this.keypair = await generateKeypair();
    }

    // Wire up close/error before connecting
    this.transport.onClose((_code: number, _reason: string) => {
      this.cleanupPendingRpcs();
    });
    this.transport.onError((_error: Error) => {
      // Errors are surfaced via pending RPC rejections on close
    });

    // Connect transport
    await this.transport.connect(this.config.server);

    // Perform handshake (sets its own onMessage handler temporarily)
    const tools = Array.from(this.tools.entries()).map(([name, { definition }]) => ({
      name,
      description: definition.description,
      parameters: definition.parameters,
    }));

    this.handshakeResult = await performHandshake(this.transport, {
      identity: this.config.identity,
      name: this.config.name,
      description: this.config.description,
      version: this.config.version,
      capabilities: this.config.capabilities ?? [],
      skills: this.config.skills,
      tools,
      keypair: this.keypair,
    });

    // Set the permanent message handler AFTER handshake completes
    this.transport.onMessage((data: string) => this.handleRawMessage(data));
  }

  /**
   * Disconnect from the server and clean up.
   */
  disconnect(): void {
    this.cleanupPendingRpcs();
    this.transport.close();
  }

  /**
   * Send a JSON-RPC request and wait for the correlated response.
   */
  private sendRpc(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    return new Promise((resolve, reject) => {
      const request = buildRequest(method, params);

      const timer = setTimeout(() => {
        this.pendingRpcs.delete(request.id);
        reject(new Error(`RPC timeout for ${method} (id=${request.id})`));
      }, this.RPC_TIMEOUT_MS);

      this.pendingRpcs.set(request.id, { resolve, reject, timer });

      this.transport.send(JSON.stringify(request)).catch((err) => {
        clearTimeout(timer);
        this.pendingRpcs.delete(request.id);
        reject(err);
      });
    });
  }

  /**
   * Handle a raw message from the transport.
   */
  private handleRawMessage(data: string): void {
    let parsed;
    try {
      parsed = parseMessage(data);
    } catch {
      return; // Ignore malformed messages
    }

    if (parsed.type === "response") {
      const pending = this.pendingRpcs.get(parsed.id);
      if (pending) {
        clearTimeout(pending.timer);
        this.pendingRpcs.delete(parsed.id);
        pending.resolve(parsed.result);
      }
      return;
    }

    if (parsed.type === "error") {
      const pending = this.pendingRpcs.get(parsed.id);
      if (pending) {
        clearTimeout(pending.timer);
        this.pendingRpcs.delete(parsed.id);
        pending.reject(new Error(`RPC error ${parsed.code}: ${parsed.message}`));
      }
      return;
    }

    // Notification
    if (parsed.method === "ainm.message.deliver") {
      this.handleMessageDelivery(parsed.params).catch((err) => {
        console.error("[AINMAgent] handleMessageDelivery error:", err);
      });
    }

    if (parsed.method === "ainm.tool.invoke") {
      this.handleToolInvoke(parsed.params).catch(() => {
        // Tool errors are silently dropped for now
      });
    }
  }

  /**
   * Handle an incoming message delivery notification.
   */
  private async handleMessageDelivery(params: Record<string, unknown>): Promise<void> {
    if (!this.messageHandler || !this.keypair) return;

    const message = params.message as Record<string, unknown> | undefined;
    if (!message) return;

    const contentBlocks = (message.content as ContentBlock[]) ?? [];
    const textBlock = contentBlocks.find((b) => b.type === "text");
    const text = (textBlock?.text as string) ?? "";

    const incoming: IncomingMessage = {
      id: (message.id as string) ?? randomUUID(),
      from: (message.from as string) ?? "",
      threadId: (message.thread_id as string) ?? "",
      timestamp: (message.timestamp as string) ?? new Date().toISOString(),
      text,
      contentBlocks,
      raw: message as Record<string, unknown>,
    };

    const threadId = incoming.threadId;
    const fromIdentity = incoming.from;
    const agentIdentity = this.config.identity;
    const keypair = this.keypair;
    const sendFn = (data: string) => this.transport.send(data);
    const sendRpcFn = (method: string, p: Record<string, unknown>) => this.sendRpc(method, p);

    const ctx: MessageContext = {
      threadId,
      from: fromIdentity,
      participants: (params.participants as string[]) ?? [fromIdentity, this.config.identity],
      parentThreadId: params.parent_thread_id as string | undefined,

      stream(): StreamHelper {
        const helper = createStreamHelper(sendFn, agentIdentity, threadId);
        // Auto-start the stream and return the bridge interface
        helper.start().catch(() => {});
        return {
          delta: (deltaText: string) => { helper.delta(deltaText).catch(() => {}); },
          end: () => { helper.end("").catch(() => {}); },
        };
      },

      memory: createMemoryContext(sendRpcFn, threadId, fromIdentity),

      async sendMessage(content: ContentBlock[]): Promise<void> {
        const envelope: Record<string, unknown> = {
          id: randomUUID(),
          from: keypair.publicKey.toString(),
          thread_id: threadId,
          timestamp: new Date().toISOString(),
          type: "message",
          content,
        };
        const signed = await signEnvelope(keypair, envelope);
        const notification = buildNotification("ainm.message.send", { message: signed });
        await sendFn(JSON.stringify(notification));
      },

      emitToolStarted(name: string, input: unknown): void {
        const notification = buildNotification("ainm.tool.started", {
          thread_id: threadId,
          tool: name,
          input,
        });
        sendFn(JSON.stringify(notification)).catch(() => {});
      },

      emitToolProgress(name: string, progress: number): void {
        const notification = buildNotification("ainm.tool.progress", {
          thread_id: threadId,
          tool: name,
          progress,
        });
        sendFn(JSON.stringify(notification)).catch(() => {});
      },

      emitToolCompleted(name: string, result: unknown): void {
        const notification = buildNotification("ainm.tool.completed", {
          thread_id: threadId,
          tool: name,
          result,
        });
        sendFn(JSON.stringify(notification)).catch(() => {});
      },
    };

    const response = await this.messageHandler(incoming, ctx);

    // If handler returns a string, auto-send it as a signed message
    if (typeof response === "string") {
      const envelope: Record<string, unknown> = {
        id: randomUUID(),
        from: this.config.identity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        type: "message",
        content: [
          {
            type: "text",
            version: "1.0",
            source: "agent",
            text: response,
            fallback: response,
          },
        ],
      };
      const signed = await signEnvelope(keypair, envelope);
      const request = buildRequest("ainm.message.send", signed as Record<string, unknown>);
      await this.transport.send(JSON.stringify(request));
    }
  }

  /**
   * Handle a tool invocation request from the server.
   */
  private async handleToolInvoke(params: Record<string, unknown>): Promise<void> {
    const toolName = params.tool as string;
    const toolParams = (params.parameters as Record<string, unknown>) ?? {};
    const invocationId = params.invocation_id as string;

    const tool = this.tools.get(toolName);
    if (!tool) {
      const errorNotification = buildNotification("ainm.tool.error", {
        invocation_id: invocationId,
        error: `Unknown tool: ${toolName}`,
      });
      await this.transport.send(JSON.stringify(errorNotification));
      return;
    }

    // Build a minimal context for tool execution
    const threadId = (params.thread_id as string) ?? "";
    const ctx = this.buildMinimalContext(threadId);

    try {
      const result = await tool.handler(toolParams, ctx);
      const resultNotification = buildNotification("ainm.tool.result", {
        invocation_id: invocationId,
        result,
      });
      await this.transport.send(JSON.stringify(resultNotification));
    } catch (err) {
      const errorNotification = buildNotification("ainm.tool.error", {
        invocation_id: invocationId,
        error: err instanceof Error ? err.message : String(err),
      });
      await this.transport.send(JSON.stringify(errorNotification));
    }
  }

  /**
   * Build a minimal MessageContext for tool invocations.
   */
  private buildMinimalContext(threadId: string): MessageContext {
    const keypair = this.keypair!;
    const agentIdentity = this.config.identity;
    const sendFn = (data: string) => this.transport.send(data);
    const sendRpcFn = (method: string, p: Record<string, unknown>) => this.sendRpc(method, p);

    return {
      threadId,
      from: this.config.identity,
      participants: [this.config.identity],
      stream(): StreamHelper {
        const helper = createStreamHelper(sendFn, agentIdentity, threadId);
        helper.start().catch(() => {});
        return {
          delta: (text: string) => { helper.delta(text).catch(() => {}); },
          end: () => { helper.end("").catch(() => {}); },
        };
      },
      memory: createMemoryContext(sendRpcFn, threadId, ""),
      async sendMessage(content: ContentBlock[]): Promise<void> {
        const envelope: Record<string, unknown> = {
          id: randomUUID(),
          from: keypair.publicKey.toString(),
          thread_id: threadId,
          timestamp: new Date().toISOString(),
          type: "message",
          content,
        };
        const signed = await signEnvelope(keypair, envelope);
        const notification = buildNotification("ainm.message.send", { message: signed });
        await sendFn(JSON.stringify(notification));
      },
      emitToolStarted(name: string, input: unknown): void {
        const notification = buildNotification("ainm.tool.started", {
          thread_id: threadId, tool: name, input,
        });
        sendFn(JSON.stringify(notification)).catch(() => {});
      },
      emitToolProgress(name: string, progress: number): void {
        const notification = buildNotification("ainm.tool.progress", {
          thread_id: threadId, tool: name, progress,
        });
        sendFn(JSON.stringify(notification)).catch(() => {});
      },
      emitToolCompleted(name: string, result: unknown): void {
        const notification = buildNotification("ainm.tool.completed", {
          thread_id: threadId, tool: name, result,
        });
        sendFn(JSON.stringify(notification)).catch(() => {});
      },
    };
  }

  /**
   * Clean up all pending RPCs with a rejection.
   */
  private cleanupPendingRpcs(): void {
    for (const [id, pending] of this.pendingRpcs) {
      clearTimeout(pending.timer);
      pending.reject(new Error("Connection closed"));
      this.pendingRpcs.delete(id);
    }
  }
}
