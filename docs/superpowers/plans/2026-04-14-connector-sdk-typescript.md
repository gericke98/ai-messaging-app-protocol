# TypeScript Connector SDK Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `@ainm/connector` — a TypeScript library that lets any developer connect an AI agent to an AINM server in 5 lines of code, with advanced support for streaming, tool events, and compound memory.

**Architecture:** Three-layer SDK (Transport → Protocol → Agent API) with pluggable transports, Ed25519 identity, and JSON-RPC 2.0 framing over WebSocket. The SDK handles all protocol plumbing; developers implement one callback (`onMessage`).

**Tech Stack:** TypeScript 5.x, Node.js 20+, `ws` (WebSocket), `@noble/ed25519` (signing), `canonicalize` (JCS/RFC 8785), vitest (testing)

---

## File Structure

```
packages/connector/
├── package.json
├── tsconfig.json
├── vitest.config.ts
├── src/
│   ├── index.ts                    # Public API exports
│   ├── agent.ts                    # AINMAgent class — main entry point
│   ├── transport/
│   │   ├── interface.ts            # Transport interface
│   │   └── websocket.ts           # WebSocket transport implementation
│   ├── protocol/
│   │   ├── rpc.ts                  # JSON-RPC 2.0 request/response/notification builders
│   │   ├── handshake.ts           # Initialize/initialized lifecycle
│   │   ├── streaming.ts           # StreamHelper for delta/end lifecycle
│   │   └── types.ts               # Message types, content block types, tool types
│   ├── identity/
│   │   ├── keypair.ts              # Ed25519 key generation and loading
│   │   └── signing.ts            # Message signing (JCS canonical + base64url)
│   └── memory/
│       └── context.ts             # MemoryContext: briefing, graph query, contribute
├── tests/
│   ├── rpc.test.ts
│   ├── handshake.test.ts
│   ├── streaming.test.ts
│   ├── signing.test.ts
│   ├── memory.test.ts
│   └── agent.test.ts
└── README.md
```

---

### Task 1: Project Scaffolding

**Files:**
- Create: `packages/connector/package.json`
- Create: `packages/connector/tsconfig.json`
- Create: `packages/connector/vitest.config.ts`
- Create: `packages/connector/src/index.ts`

- [ ] **Step 1: Create the package directory**

```bash
mkdir -p packages/connector/src packages/connector/tests
```

- [ ] **Step 2: Create package.json**

Create `packages/connector/package.json`:

```json
{
  "name": "@ainm/connector",
  "version": "0.1.0",
  "description": "AINM Protocol Connector SDK — connect any AI agent to the AINM messaging network",
  "type": "module",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.js",
      "types": "./dist/index.d.ts"
    }
  },
  "scripts": {
    "build": "tsc",
    "test": "vitest run",
    "test:watch": "vitest",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "ws": "^8.18.0",
    "@noble/ed25519": "^2.2.0",
    "canonicalize": "^2.0.0"
  },
  "devDependencies": {
    "typescript": "^5.7.0",
    "vitest": "^3.0.0",
    "@types/ws": "^8.5.0",
    "@types/node": "^22.0.0"
  },
  "engines": {
    "node": ">=20.0.0"
  },
  "license": "MIT"
}
```

- [ ] **Step 3: Create tsconfig.json**

Create `packages/connector/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "declaration": true,
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "tests"]
}
```

- [ ] **Step 4: Create vitest.config.ts**

Create `packages/connector/vitest.config.ts`:

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
  },
});
```

- [ ] **Step 5: Create empty index.ts**

Create `packages/connector/src/index.ts`:

```typescript
export { AINMAgent } from "./agent.js";
export type {
  AINMAgentConfig,
  MessageHandler,
  MessageContext,
  IncomingMessage,
} from "./protocol/types.js";
export type { Transport } from "./transport/interface.js";
```

- [ ] **Step 6: Install dependencies**

```bash
cd packages/connector && npm install
```

- [ ] **Step 7: Commit**

```bash
git add packages/connector/package.json packages/connector/tsconfig.json packages/connector/vitest.config.ts packages/connector/src/index.ts
git commit -m "feat(connector): scaffold TypeScript SDK package"
```

---

### Task 2: Protocol Types

**Files:**
- Create: `packages/connector/src/protocol/types.ts`
- Create: `packages/connector/tests/rpc.test.ts`

- [ ] **Step 1: Write the test for protocol types**

Create `packages/connector/tests/rpc.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { buildRequest, buildNotification, parseMessage } from "../src/protocol/rpc.js";

describe("JSON-RPC builders", () => {
  it("builds a request with incrementing ID", () => {
    const req = buildRequest("ainm.initialize", { identity: "test" });
    expect(req.jsonrpc).toBe("2.0");
    expect(req.method).toBe("ainm.initialize");
    expect(req.params.identity).toBe("test");
    expect(typeof req.id).toBe("number");
  });

  it("builds a notification (no id)", () => {
    const notif = buildNotification("ainm.initialized", {});
    expect(notif.jsonrpc).toBe("2.0");
    expect(notif.method).toBe("ainm.initialized");
    expect(notif.id).toBeUndefined();
  });

  it("parses a response", () => {
    const raw = JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      result: { protocolVersion: "0.1.0" },
    });
    const msg = parseMessage(raw);
    expect(msg.type).toBe("response");
    if (msg.type === "response") {
      expect(msg.id).toBe(1);
      expect(msg.result.protocolVersion).toBe("0.1.0");
    }
  });

  it("parses a notification", () => {
    const raw = JSON.stringify({
      jsonrpc: "2.0",
      method: "ainm.message.deliver",
      params: { message: { text: "hello" } },
    });
    const msg = parseMessage(raw);
    expect(msg.type).toBe("notification");
    if (msg.type === "notification") {
      expect(msg.method).toBe("ainm.message.deliver");
    }
  });

  it("parses an error response", () => {
    const raw = JSON.stringify({
      jsonrpc: "2.0",
      id: 2,
      error: { code: -32601, message: "Method not found" },
    });
    const msg = parseMessage(raw);
    expect(msg.type).toBe("error");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd packages/connector && npx vitest run tests/rpc.test.ts
```

Expected: FAIL — modules not found.

- [ ] **Step 3: Create protocol types**

Create `packages/connector/src/protocol/types.ts`:

```typescript
export interface AINMAgentConfig {
  server: string;
  identity: string;
  name: string;
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
  sendMessage: (content: ContentBlock[]) => Promise<void>;
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
  | {
      type: "notification";
      method: string;
      params: Record<string, unknown>;
    };
```

- [ ] **Step 4: Create JSON-RPC builders**

Create `packages/connector/src/protocol/rpc.ts`:

```typescript
import type {
  JsonRpcRequest,
  JsonRpcNotification,
  ParsedMessage,
} from "./types.js";

let nextId = 1;

export function buildRequest(
  method: string,
  params: Record<string, unknown>
): JsonRpcRequest {
  return { jsonrpc: "2.0", id: nextId++, method, params };
}

export function buildNotification(
  method: string,
  params: Record<string, unknown>
): JsonRpcNotification {
  return { jsonrpc: "2.0", method, params };
}

export function parseMessage(raw: string): ParsedMessage {
  const msg = JSON.parse(raw);

  if (msg.error) {
    return {
      type: "error",
      id: msg.id,
      code: msg.error.code,
      message: msg.error.message,
    };
  }

  if (msg.id !== undefined && msg.result !== undefined) {
    return { type: "response", id: msg.id, result: msg.result };
  }

  return {
    type: "notification",
    method: msg.method,
    params: msg.params ?? {},
  };
}

export function resetIdCounter(): void {
  nextId = 1;
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cd packages/connector && npx vitest run tests/rpc.test.ts
```

Expected: PASS (all 5 tests).

- [ ] **Step 6: Commit**

```bash
git add packages/connector/src/protocol/ packages/connector/tests/rpc.test.ts
git commit -m "feat(connector): add JSON-RPC 2.0 builders and protocol types"
```

---

### Task 3: Ed25519 Identity and Message Signing

**Files:**
- Create: `packages/connector/src/identity/keypair.ts`
- Create: `packages/connector/src/identity/signing.ts`
- Create: `packages/connector/tests/signing.test.ts`

- [ ] **Step 1: Write the signing tests**

Create `packages/connector/tests/signing.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { generateKeypair, exportPublicKey } from "../src/identity/keypair.js";
import { signInit, signEnvelope, verifyEnvelope } from "../src/identity/signing.js";

describe("Ed25519 identity", () => {
  it("generates a keypair", async () => {
    const kp = await generateKeypair();
    expect(kp.privateKey).toBeInstanceOf(Uint8Array);
    expect(kp.publicKey).toBeInstanceOf(Uint8Array);
    expect(kp.privateKey.length).toBe(32);
    expect(kp.publicKey.length).toBe(32);
  });

  it("exports public key as base64url", async () => {
    const kp = await generateKeypair();
    const exported = exportPublicKey(kp);
    expect(typeof exported).toBe("string");
    expect(exported.length).toBeGreaterThan(0);
    expect(exported).not.toContain("=");
    expect(exported).not.toContain("+");
    expect(exported).not.toContain("/");
  });
});

describe("message signing", () => {
  it("signs an init payload", async () => {
    const kp = await generateKeypair();
    const sig = await signInit(kp, "agent@test.local");
    expect(typeof sig).toBe("string");
    expect(sig.length).toBeGreaterThan(0);
  });

  it("signs and verifies an envelope", async () => {
    const kp = await generateKeypair();
    const envelope = {
      id: "msg-1",
      from: "agent@test.local",
      thread_id: "thread-1",
      timestamp: "2026-01-01T00:00:00.000Z",
      type: "message",
      content: [{ type: "text", text: "hello" }],
    };
    const signed = await signEnvelope(kp, envelope);
    expect(signed.signature).toBeDefined();
    expect(typeof signed.signature).toBe("string");

    const valid = await verifyEnvelope(kp.publicKey, signed);
    expect(valid).toBe(true);
  });

  it("rejects tampered envelope", async () => {
    const kp = await generateKeypair();
    const envelope = {
      id: "msg-2",
      from: "agent@test.local",
      thread_id: "thread-1",
      timestamp: "2026-01-01T00:00:00.000Z",
      type: "message",
      content: [{ type: "text", text: "hello" }],
    };
    const signed = await signEnvelope(kp, envelope);
    signed.content = [{ type: "text", text: "tampered" }];

    const valid = await verifyEnvelope(kp.publicKey, signed);
    expect(valid).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/signing.test.ts
```

Expected: FAIL — modules not found.

- [ ] **Step 3: Implement keypair generation**

Create `packages/connector/src/identity/keypair.ts`:

```typescript
import * as ed from "@noble/ed25519";

export interface Keypair {
  privateKey: Uint8Array;
  publicKey: Uint8Array;
}

export async function generateKeypair(): Promise<Keypair> {
  const privateKey = ed.utils.randomPrivateKey();
  const publicKey = await ed.getPublicKeyAsync(privateKey);
  return { privateKey, publicKey };
}

export function exportPublicKey(keypair: Keypair): string {
  return base64urlEncode(keypair.publicKey);
}

export function base64urlEncode(bytes: Uint8Array): string {
  const base64 = Buffer.from(bytes).toString("base64");
  return base64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function base64urlDecode(str: string): Uint8Array {
  const base64 = str.replace(/-/g, "+").replace(/_/g, "/");
  return new Uint8Array(Buffer.from(base64, "base64"));
}
```

- [ ] **Step 4: Implement message signing**

Create `packages/connector/src/identity/signing.ts`:

```typescript
import * as ed from "@noble/ed25519";
import canonicalize from "canonicalize";
import { base64urlEncode, base64urlDecode, type Keypair } from "./keypair.js";

export async function signInit(
  keypair: Keypair,
  identity: string
): Promise<string> {
  const timestamp = new Date().toISOString();
  const payload = `${identity}|${timestamp}`;
  const sig = await ed.signAsync(
    new TextEncoder().encode(payload),
    keypair.privateKey
  );
  return base64urlEncode(sig);
}

export async function signEnvelope(
  keypair: Keypair,
  envelope: Record<string, unknown>
): Promise<Record<string, unknown>> {
  const copy = { ...envelope };
  delete copy.signature;

  const canonical = canonicalize(copy);
  if (!canonical) throw new Error("Canonicalization failed");

  const sig = await ed.signAsync(
    new TextEncoder().encode(canonical),
    keypair.privateKey
  );

  return { ...envelope, signature: base64urlEncode(sig) };
}

export async function verifyEnvelope(
  publicKey: Uint8Array,
  envelope: Record<string, unknown>
): Promise<boolean> {
  const sig = envelope.signature;
  if (typeof sig !== "string") return false;

  const copy = { ...envelope };
  delete copy.signature;

  const canonical = canonicalize(copy);
  if (!canonical) return false;

  try {
    return await ed.verifyAsync(
      base64urlDecode(sig),
      new TextEncoder().encode(canonical),
      publicKey
    );
  } catch {
    return false;
  }
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
npx vitest run tests/signing.test.ts
```

Expected: PASS (all 5 tests).

- [ ] **Step 6: Commit**

```bash
git add packages/connector/src/identity/ packages/connector/tests/signing.test.ts
git commit -m "feat(connector): add Ed25519 keypair generation and message signing"
```

---

### Task 4: Transport Interface and WebSocket Implementation

**Files:**
- Create: `packages/connector/src/transport/interface.ts`
- Create: `packages/connector/src/transport/websocket.ts`

- [ ] **Step 1: Create Transport interface**

Create `packages/connector/src/transport/interface.ts`:

```typescript
export interface Transport {
  connect(url: string): Promise<void>;
  send(data: string): Promise<void>;
  onMessage(handler: (data: string) => void): void;
  onClose(handler: (code: number, reason: string) => void): void;
  onError(handler: (error: Error) => void): void;
  close(): void;
  isConnected(): boolean;
}
```

- [ ] **Step 2: Implement WebSocket transport**

Create `packages/connector/src/transport/websocket.ts`:

```typescript
import WebSocket from "ws";
import type { Transport } from "./interface.js";

export class WebSocketTransport implements Transport {
  private ws: WebSocket | null = null;
  private messageHandler: ((data: string) => void) | null = null;
  private closeHandler: ((code: number, reason: string) => void) | null = null;
  private errorHandler: ((error: Error) => void) | null = null;

  async connect(url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(url);

      this.ws.on("open", () => resolve());
      this.ws.on("error", (err) => {
        if (this.errorHandler) this.errorHandler(err);
        reject(err);
      });

      this.ws.on("message", (data) => {
        if (this.messageHandler) {
          this.messageHandler(data.toString());
        }
      });

      this.ws.on("close", (code, reason) => {
        if (this.closeHandler) {
          this.closeHandler(code, reason.toString());
        }
      });
    });
  }

  async send(data: string): Promise<void> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("WebSocket not connected");
    }
    return new Promise((resolve, reject) => {
      this.ws!.send(data, (err) => (err ? reject(err) : resolve()));
    });
  }

  onMessage(handler: (data: string) => void): void {
    this.messageHandler = handler;
  }

  onClose(handler: (code: number, reason: string) => void): void {
    this.closeHandler = handler;
  }

  onError(handler: (error: Error) => void): void {
    this.errorHandler = handler;
  }

  close(): void {
    this.ws?.close();
    this.ws = null;
  }

  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add packages/connector/src/transport/
git commit -m "feat(connector): add Transport interface and WebSocket implementation"
```

---

### Task 5: Handshake Protocol

**Files:**
- Create: `packages/connector/src/protocol/handshake.ts`
- Create: `packages/connector/tests/handshake.test.ts`

- [ ] **Step 1: Write the handshake test**

Create `packages/connector/tests/handshake.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
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

    // Simulate server response after initialize is sent
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
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/handshake.test.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Implement handshake**

Create `packages/connector/src/protocol/handshake.ts`:

```typescript
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
    protocolVersion:
      (result.protocolVersion as string) ?? "0.1.0",
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

    const originalHandler = transport.onMessage;
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
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/handshake.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/connector/src/protocol/handshake.ts packages/connector/tests/handshake.test.ts
git commit -m "feat(connector): add AINM handshake protocol (initialize/initialized)"
```

---

### Task 6: Streaming Protocol Helper

**Files:**
- Create: `packages/connector/src/protocol/streaming.ts`
- Create: `packages/connector/tests/streaming.test.ts`

- [ ] **Step 1: Write the streaming test**

Create `packages/connector/tests/streaming.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { createStreamHelper } from "../src/protocol/streaming.js";

describe("StreamHelper", () => {
  it("sends stream start, deltas, and end", async () => {
    const sent: string[] = [];
    const sendFn = async (data: string) => { sent.push(data); };

    const stream = createStreamHelper(sendFn, "agent@test", "thread-1");
    await stream.start();

    expect(sent.length).toBe(1);
    const start = JSON.parse(sent[0]);
    expect(start.method).toBe("ainm.stream.start");
    expect(start.params.thread_id).toBe("thread-1");
    expect(start.params.from).toBe("agent@test");
    expect(start.params.stream_id).toBeDefined();

    await stream.delta("Hello ");
    await stream.delta("world!");

    expect(sent.length).toBe(3);
    const d1 = JSON.parse(sent[1]);
    expect(d1.method).toBe("ainm.stream.delta");
    expect(d1.params.delta).toBe("Hello ");
    expect(d1.params.block_index).toBe(0);

    await stream.end("Hello world!");

    expect(sent.length).toBe(4);
    const end = JSON.parse(sent[3]);
    expect(end.method).toBe("ainm.stream.end");
    expect(end.params.status).toBe("completed");
    expect(end.params.collapsed_message).toBeDefined();
    expect(end.params.collapsed_message.content[0].text).toBe("Hello world!");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/streaming.test.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement streaming helper**

Create `packages/connector/src/protocol/streaming.ts`:

```typescript
import { buildNotification } from "./rpc.js";
import { randomUUID } from "node:crypto";

export interface StreamHelper {
  start: () => Promise<void>;
  delta: (text: string) => Promise<void>;
  end: (fullText: string) => Promise<void>;
  streamId: string;
  messageId: string;
}

export function createStreamHelper(
  send: (data: string) => Promise<void>,
  fromIdentity: string,
  threadId: string
): StreamHelper {
  const streamId = randomUUID();
  const messageId = randomUUID();

  return {
    streamId,
    messageId,

    async start() {
      const notification = buildNotification("ainm.stream.start", {
        stream_id: streamId,
        message_id: messageId,
        from: fromIdentity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        blocks: [{ type: "text", version: "1.0", source: "agent" }],
      });
      await send(JSON.stringify(notification));
    },

    async delta(text: string) {
      const notification = buildNotification("ainm.stream.delta", {
        stream_id: streamId,
        block_index: 0,
        delta: text,
      });
      await send(JSON.stringify(notification));
    },

    async end(fullText: string) {
      const collapsed = {
        id: messageId,
        from: fromIdentity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        type: "message",
        content: [
          {
            type: "text",
            version: "1.0",
            source: "agent",
            text: fullText,
            fallback: fullText,
          },
        ],
      };

      const notification = buildNotification("ainm.stream.end", {
        stream_id: streamId,
        status: "completed",
        collapsed_message: collapsed,
      });
      await send(JSON.stringify(notification));
    },
  };
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/streaming.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/connector/src/protocol/streaming.ts packages/connector/tests/streaming.test.ts
git commit -m "feat(connector): add streaming protocol helper (start/delta/end lifecycle)"
```

---

### Task 7: Memory Context

**Files:**
- Create: `packages/connector/src/memory/context.ts`
- Create: `packages/connector/tests/memory.test.ts`

- [ ] **Step 1: Write the memory test**

Create `packages/connector/tests/memory.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { createMemoryContext } from "../src/memory/context.js";

describe("MemoryContext", () => {
  it("requests a briefing via RPC", async () => {
    const sent: string[] = [];
    const sendRpc = vi.fn(async (method: string, params: Record<string, unknown>) => {
      return {
        briefing_id: "b-1",
        scope: "standard",
        sections: { identity: { name: "Test User" } },
      };
    });

    const mem = createMemoryContext(sendRpc, "thread-1", "user@test");
    const briefing = await mem.getBriefing();
    expect(sendRpc).toHaveBeenCalledWith("ainm.memory.briefing", {
      thread_id: "thread-1",
      user_identity: "user@test",
    });
    expect(briefing.briefing_id).toBe("b-1");
  });

  it("contributes a node to the graph", async () => {
    const sendRpc = vi.fn(async () => ({
      contributed: true,
      node_id: "node-123",
      edge_ids: [],
    }));

    const mem = createMemoryContext(sendRpc, "thread-1", "user@test");
    const nodeId = await mem.contribute({
      type: "semantic",
      category: "fact",
      label: "Likes Rust",
      confidence: 0.9,
    });
    expect(nodeId).toBe("node-123");
    expect(sendRpc).toHaveBeenCalledWith("ainm.memory.graph.contribute", {
      thread_id: "thread-1",
      node: {
        type: "semantic",
        category: "fact",
        label: "Likes Rust",
        confidence: 0.9,
      },
    });
  });

  it("queries the graph", async () => {
    const sendRpc = vi.fn(async () => ({
      nodes: [{ node_id: "n1", label: "Rust" }],
      edges: [],
      total_count: 1,
      returned_count: 1,
    }));

    const mem = createMemoryContext(sendRpc, "thread-1", "user@test");
    const result = await mem.queryGraph("Rust", 10);
    expect(sendRpc).toHaveBeenCalledWith("ainm.memory.graph.query", {
      query: "Rust",
      include_edges: true,
      limit: 10,
    });
    expect(result.nodes.length).toBe(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/memory.test.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement memory context**

Create `packages/connector/src/memory/context.ts`:

```typescript
import type { MemoryContext, MemoryNodeInput } from "../protocol/types.js";

type RpcSender = (
  method: string,
  params: Record<string, unknown>
) => Promise<Record<string, unknown>>;

export function createMemoryContext(
  sendRpc: RpcSender,
  threadId: string,
  userIdentity: string
): MemoryContext {
  return {
    async getBriefing() {
      return sendRpc("ainm.memory.briefing", {
        thread_id: threadId,
        user_identity: userIdentity,
      });
    },

    async queryGraph(query?: string, limit?: number) {
      const params: Record<string, unknown> = {
        include_edges: true,
      };
      if (query) params.query = query;
      if (limit) params.limit = limit;
      return sendRpc("ainm.memory.graph.query", params);
    },

    async contribute(node: MemoryNodeInput): Promise<string> {
      const result = await sendRpc("ainm.memory.graph.contribute", {
        thread_id: threadId,
        node: {
          type: node.type,
          category: node.category,
          label: node.label,
          confidence: node.confidence,
          ...(node.properties && { properties: node.properties }),
          ...(node.tags && { tags: node.tags }),
        },
      });
      return result.node_id as string;
    },
  };
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/memory.test.ts
```

Expected: PASS (all 3 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/connector/src/memory/ packages/connector/tests/memory.test.ts
git commit -m "feat(connector): add MemoryContext for briefing, graph query, contribute"
```

---

### Task 8: AINMAgent — Main Entry Point

**Files:**
- Create: `packages/connector/src/agent.ts`
- Create: `packages/connector/tests/agent.test.ts`

- [ ] **Step 1: Write the agent integration test**

Create `packages/connector/tests/agent.test.ts`:

```typescript
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
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/agent.test.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement AINMAgent**

Create `packages/connector/src/agent.ts`:

```typescript
import { WebSocketTransport } from "./transport/websocket.js";
import type { Transport } from "./transport/interface.js";
import { performHandshake } from "./protocol/handshake.js";
import { buildRequest, buildNotification, parseMessage } from "./protocol/rpc.js";
import { createStreamHelper, type StreamHelper } from "./protocol/streaming.js";
import { generateKeypair, type Keypair } from "./identity/keypair.js";
import { signEnvelope } from "./identity/signing.js";
import { createMemoryContext } from "./memory/context.js";
import type {
  AINMAgentConfig,
  MessageHandler,
  MessageContext,
  IncomingMessage,
  ToolDefinition,
  ToolHandler,
  ContentBlock,
} from "./protocol/types.js";

export class AINMAgent {
  private config: AINMAgentConfig;
  private transport: Transport;
  private messageHandler: MessageHandler | null = null;
  private tools: Map<string, { def: ToolDefinition; handler: ToolHandler }> = new Map();
  private keypair: Keypair | null = null;
  private pendingResponses: Map<number, {
    resolve: (v: Record<string, unknown>) => void;
    reject: (e: Error) => void;
  }> = new Map();
  private nextRpcId = 100;

  constructor(config: AINMAgentConfig, transport?: Transport) {
    this.config = config;
    this.transport = transport ?? new WebSocketTransport();
  }

  onMessage(handler: MessageHandler): void {
    this.messageHandler = handler;
  }

  registerTool(name: string, def: ToolDefinition, handler: ToolHandler): void {
    this.tools.set(name, { def, handler });
  }

  async connect(): Promise<void> {
    this.keypair = this.config.privateKey
      ? { privateKey: this.config.privateKey, publicKey: new Uint8Array() }
      : await generateKeypair();

    await this.transport.connect(this.config.server);

    this.transport.onMessage((data) => this.handleIncoming(data));
    this.transport.onClose((code) => this.handleDisconnect(code));

    await performHandshake(this.transport, {
      identity: this.config.identity,
      name: this.config.name,
      capabilities: this.config.capabilities ?? ["text"],
      skills: this.config.skills,
      keypair: this.keypair,
    });
  }

  async disconnect(): Promise<void> {
    this.transport.close();
  }

  private async handleIncoming(data: string): Promise<void> {
    const msg = parseMessage(data);

    if (msg.type === "response" || msg.type === "error") {
      const pending = this.pendingResponses.get(msg.id);
      if (pending) {
        this.pendingResponses.delete(msg.id);
        if (msg.type === "error") {
          pending.reject(new Error(`RPC error ${msg.code}: ${msg.message}`));
        } else {
          pending.resolve(msg.result);
        }
      }
      return;
    }

    if (msg.type === "notification" && msg.method === "ainm.message.deliver") {
      await this.handleMessageDelivery(msg.params);
    }
  }

  private async handleMessageDelivery(
    params: Record<string, unknown>
  ): Promise<void> {
    if (!this.messageHandler) return;

    const message = params.message as Record<string, unknown>;
    if (!message) return;

    const contentBlocks = (message.content as ContentBlock[]) ?? [];
    const textBlock = contentBlocks.find((b) => b.type === "text");
    const text = (textBlock?.text as string) ?? (textBlock?.fallback as string) ?? "";

    const threadId = message.thread_id as string;
    const from = message.from as string;

    const incoming: IncomingMessage = {
      id: message.id as string,
      from,
      threadId,
      timestamp: message.timestamp as string,
      text,
      contentBlocks,
      raw: message,
    };

    const sendFn = async (data: string) => this.transport.send(data);

    const ctx: MessageContext = {
      threadId,
      from,
      participants: [],
      stream: () => {
        const helper = createStreamHelper(sendFn, this.config.identity, threadId);
        helper.start();
        return { delta: (t: string) => { helper.delta(t); }, end: () => { helper.end(""); } };
      },
      memory: createMemoryContext(
        (method, params) => this.sendRpc(method, params),
        threadId,
        from
      ),
      sendMessage: async (content: ContentBlock[]) => {
        await this.sendMessageEnvelope(threadId, content);
      },
      emitToolStarted: (name, input) => {
        const notif = buildNotification("ainm.tool.call.started", {
          call_id: `${name}-${Date.now()}`,
          thread_id: threadId,
          name,
          input,
        });
        this.transport.send(JSON.stringify(notif));
      },
      emitToolProgress: (name, progress) => {
        const notif = buildNotification("ainm.tool.call.progress", {
          call_id: `${name}-${Date.now()}`,
          thread_id: threadId,
          name,
          progress,
        });
        this.transport.send(JSON.stringify(notif));
      },
      emitToolCompleted: (name, result) => {
        const notif = buildNotification("ainm.tool.call.completed", {
          call_id: `${name}-${Date.now()}`,
          thread_id: threadId,
          name,
          status: "success",
          result_summary: typeof result === "string" ? result : JSON.stringify(result),
        });
        this.transport.send(JSON.stringify(notif));
      },
    };

    const response = await this.messageHandler(incoming, ctx);

    if (typeof response === "string") {
      await this.sendMessageEnvelope(threadId, [
        {
          type: "text",
          version: "1.0",
          source: "agent",
          text: response,
          fallback: response,
        },
      ]);
    }
  }

  private async sendMessageEnvelope(
    threadId: string,
    content: ContentBlock[]
  ): Promise<void> {
    const envelope: Record<string, unknown> = {
      id: crypto.randomUUID(),
      from: this.config.identity,
      thread_id: threadId,
      timestamp: new Date().toISOString(),
      type: "message",
      content,
    };

    if (this.keypair) {
      const signed = await signEnvelope(this.keypair, envelope);
      Object.assign(envelope, signed);
    }

    const req = buildRequest("ainm.message.send", envelope);
    await this.transport.send(JSON.stringify(req));
  }

  private async sendRpc(
    method: string,
    params: Record<string, unknown>
  ): Promise<Record<string, unknown>> {
    const id = this.nextRpcId++;
    const req = { jsonrpc: "2.0" as const, id, method, params };
    await this.transport.send(JSON.stringify(req));

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(
        () => {
          this.pendingResponses.delete(id);
          reject(new Error(`RPC timeout for ${method}`));
        },
        30000
      );
      this.pendingResponses.set(id, {
        resolve: (v) => { clearTimeout(timeout); resolve(v); },
        reject: (e) => { clearTimeout(timeout); reject(e); },
      });
    });
  }

  private handleDisconnect(code: number): void {
    for (const [, pending] of this.pendingResponses) {
      pending.reject(new Error("Disconnected"));
    }
    this.pendingResponses.clear();
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
npx vitest run tests/agent.test.ts
```

Expected: PASS (all 3 tests).

- [ ] **Step 5: Run full test suite**

```bash
npx vitest run
```

Expected: All tests pass across all test files.

- [ ] **Step 6: Commit**

```bash
git add packages/connector/src/agent.ts packages/connector/tests/agent.test.ts packages/connector/src/index.ts
git commit -m "feat(connector): add AINMAgent entry point with message handling, streaming, memory, tools"
```

---

### Task 9: Build Verification and README

**Files:**
- Modify: `packages/connector/src/index.ts`
- Create: `packages/connector/README.md`

- [ ] **Step 1: Finalize index.ts exports**

Update `packages/connector/src/index.ts`:

```typescript
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
```

- [ ] **Step 2: Verify build succeeds**

```bash
npx tsc --noEmit
```

Expected: No errors.

- [ ] **Step 3: Create README**

Create `packages/connector/README.md`:

```markdown
# @ainm/connector

TypeScript SDK for connecting AI agents to the AINM messaging network.

## Quick Start

```typescript
import { AINMAgent } from "@ainm/connector";

const agent = new AINMAgent({
  server: "wss://my-server:8080/ws",
  identity: "my-agent@server.local",
  name: "My Agent",
  capabilities: ["text", "code"],
});

agent.onMessage(async (message, ctx) => {
  return `You said: ${message.text}`;
});

agent.connect();
```

## Streaming Responses

```typescript
agent.onMessage(async (message, ctx) => {
  const stream = ctx.stream();
  for (const word of message.text.split(" ")) {
    stream.delta(word + " ");
  }
  stream.end();
});
```

## Compound Memory

```typescript
agent.onMessage(async (message, ctx) => {
  const briefing = await ctx.memory.getBriefing();
  const graph = await ctx.memory.queryGraph("preferences");

  await ctx.memory.contribute({
    type: "semantic",
    category: "fact",
    label: "User prefers TypeScript",
    confidence: 0.9,
  });
});
```

## Tool Registration

```typescript
agent.registerTool(
  "search",
  {
    description: "Search documentation",
    parameters: { query: { type: "string" } },
  },
  async (params, ctx) => {
    ctx.emitToolStarted("search", params);
    const results = await searchDocs(params.query);
    ctx.emitToolCompleted("search", results);
    return results;
  }
);
```

## Protocol

The SDK implements the [AINM Protocol](https://ainm.dev) over WebSocket + JSON-RPC 2.0 with Ed25519 message signing.
```

- [ ] **Step 4: Run full test suite one final time**

```bash
npx vitest run && npx tsc --noEmit
```

Expected: All tests pass, type check passes.

- [ ] **Step 5: Commit**

```bash
git add packages/connector/
git commit -m "feat(connector): finalize @ainm/connector SDK with README and exports"
```
