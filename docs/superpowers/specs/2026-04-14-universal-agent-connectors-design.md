# Universal Agent Connectors — Design Spec

**Date:** 2026-04-14
**Status:** Approved
**Scope:** Phase A — Make AINM a universal agent client

## Vision

AINM is a universal agent client. Users connect to any agent runtime — Claude Code on their laptop, Codex in the cloud, a custom research agent, Ollama locally — from one app. Every conversation builds compound memory that any new agent can pull from. No lock-in to any provider.

The app is to AI agents what email is to messaging: an open, federated protocol where the client doesn't care who the provider is.

## Problem Statement

Today the AINM server has a scripted mock agent. The iOS app works end-to-end (messaging, streaming, tool visibility, compound memory), but users can't talk to real agents. The protocol is implemented; the last mile — connecting to actual agent runtimes — is missing.

## Architecture

```
┌──────────┐     ┌──────────────┐     ┌─────────────────┐     ┌─────────────────────┐
│  iOS App │────>│  AINM Server │<────│ Connector SDK   │<────│ Agent Runtime       │
│ (SwiftUI)│     │ (Rust/Axum)  │     │ (TS or Python)  │     │ (Claude Code, Codex,│
│          │<────│              │────>│                 │────>│  Ollama, custom)    │
└──────────┘     └──────────────┘     └─────────────────┘     └─────────────────────┘
   WebSocket       WebSocket             Native API
   JSON-RPC        JSON-RPC              (SDK, CLI, HTTP)
```

**Agents connect TO the server** via outbound WebSocket. This works through NAT/firewalls, matches the existing server architecture, and means no inbound ports on the user's machine.

The server is a **switchboard**, not a wrapper. It handles human connections, agent connections, thread routing, compound memory, and federation. It does NOT call LLM APIs directly.

### Connector SDK

A library that handles all AINM protocol plumbing. The developer implements one concern: bridging their agent runtime.

```
┌─────────────────────────────────────┐
│        Agent Implementation         │  Developer writes this
│   onMessage(), onToolResult(), ...  │
├─────────────────────────────────────┤
│         AINM Connector SDK          │  We build this
│  Identity, Streaming, Memory, RPC   │
├─────────────────────────────────────┤
│          Transport Layer            │  Pluggable
│   WebSocket │ Stdio │ HTTP/SSE      │
└─────────────────────────────────────┘
```

### Key Design Decisions

1. **Agents connect to server, not the other way around.** Outbound WebSocket from the agent's machine. No inbound ports, works through firewalls/NAT.

2. **Multi-agent per server.** A single AINM server hosts multiple agent connections simultaneously. Each registers with a unique identity and Agent Card. Users pick which agent(s) to include per thread.

3. **Protocol-first, multi-language SDKs.** The AINM WebSocket protocol is the contract. TypeScript SDK ships first (covers Claude Code, Codex, most runtimes), Python follows. Any language can build a connector because the protocol is documented.

4. **Two-tier SDK API.** Simple mode: 5 lines to connect. Advanced mode: streaming, tool events, memory queries, approval gates.

5. **Compound memory integration.** Connectors can pull the user's knowledge graph via `ctx.memory.getBriefing()` and inject it as system context. Agents can contribute back via `ctx.memory.contribute()`. Two-way memory flow.

6. **Reference connectors using our own SDK.** Dogfooding proves the SDK works.

## TypeScript Connector SDK (`@ainm/connector`)

### Simple Mode

```typescript
import { AINMAgent } from "@ainm/connector";

const agent = new AINMAgent({
  server: "wss://my-server.local:8080/ws",
  identity: "research-assistant",
  name: "Research Assistant",
  capabilities: ["text", "code"],
});

agent.onMessage(async (message, ctx) => {
  const response = await myLLM.generate(message.text);
  return response;
});

agent.connect();
```

### Advanced Mode

```typescript
const agent = new AINMAgent({
  server: "wss://my-server.local:8080/ws",
  identity: "claude-code-bridge",
  name: "Claude Code",
  capabilities: ["text", "code", "tool"],
  skills: [
    { name: "code-review", description: "Review code for bugs and quality" },
    { name: "refactor", description: "Refactor code for clarity" },
  ],
});

agent.onMessage(async (message, ctx) => {
  // Pull user's compound memory for context
  const briefing = await ctx.memory.getBriefing();

  // Stream response token by token
  const stream = ctx.stream();
  for await (const token of myRuntime.generate(message.text, briefing)) {
    stream.delta(token);
  }
  stream.end();

  // Contribute knowledge back to the graph
  await ctx.memory.contribute({
    type: "semantic",
    category: "fact",
    label: "Discussed project architecture",
    confidence: 0.8,
  });
});

// Declare tools with approval gates
agent.registerTool(
  "run_command",
  {
    description: "Execute a shell command",
    parameters: { command: { type: "string" } },
    needsApproval: (params) => params.command.includes("rm"),
  },
  async (params, ctx) => {
    ctx.emitToolStarted("run_command", params);
    const result = await runtime.exec(params.command);
    ctx.emitToolCompleted("run_command", { output: result });
    return result;
  }
);

agent.connect();
```

### SDK Internals

The SDK handles:

- **WebSocket lifecycle:** Connect, reconnect with exponential backoff, heartbeat.
- **Handshake:** `ainm.initialize` → capability negotiation → `ainm.initialized`.
- **Ed25519 identity:** Generate or load keypair, sign all outbound messages.
- **Agent Card generation:** Auto-generate from `capabilities`, `skills`, and `registerTool()` calls. Serve at well-known endpoint.
- **Streaming protocol:** `StreamStart` → `StreamDelta` (per token) → `StreamEnd` with collapsed signed message. The `ctx.stream()` helper manages the full lifecycle.
- **Tool events:** `ctx.emitToolStarted()`, `ctx.emitToolProgress()`, `ctx.emitToolCompleted()`. Maps to `ainm.tool.call.*` JSON-RPC notifications.
- **Compound memory:** `ctx.memory.getBriefing()` calls `ainm.memory.briefing`. `ctx.memory.queryGraph()` calls `ainm.memory.graph.query`. `ctx.memory.contribute()` calls `ainm.memory.graph.contribute`.
- **Content blocks:** Helpers for sending rich content: `ctx.sendForm()`, `ctx.sendTable()`, `ctx.sendCode()`, `ctx.sendApproval()`.
- **Thread awareness:** `ctx.threadId`, `ctx.participants`, `ctx.parentThreadId` for sub-thread support.
- **Transport abstraction:** `Transport` interface with `WebSocketTransport` (primary), `StdioTransport` (for CLI-based agents), `HttpSseTransport` (fallback).

### Package Structure

```
@ainm/connector/
├── src/
│   ├── agent.ts              # AINMAgent class — main entry point
│   ├── transport/
│   │   ├── interface.ts       # Transport trait
│   │   ├── websocket.ts       # WebSocket implementation
│   │   ├── stdio.ts           # Stdin/stdout for CLI agents
│   │   └── http-sse.ts        # HTTP + SSE fallback
│   ├── protocol/
│   │   ├── handshake.ts       # Initialize/initialized lifecycle
│   │   ├── rpc.ts             # JSON-RPC 2.0 framing
│   │   ├── streaming.ts       # Stream start/delta/end helpers
│   │   └── types.ts           # Message types, content blocks
│   ├── identity/
│   │   ├── keypair.ts         # Ed25519 key generation/loading
│   │   ├── signing.ts         # Message signing (JCS canonical)
│   │   └── agent-card.ts      # Auto-generate Agent Card
│   ├── memory/
│   │   ├── briefing.ts        # Pull briefings from server
│   │   ├── graph.ts           # Query/contribute to memory graph
│   │   └── context.ts         # MemoryContext helper for handlers
│   └── index.ts               # Public API exports
├── package.json
├── tsconfig.json
└── README.md
```

## Python Connector SDK (`ainm-connector`)

Same API shape, Pythonic conventions:

```python
from ainm_connector import AINMAgent

agent = AINMAgent(
    server="wss://my-server.local:8080/ws",
    identity="research-bot",
    name="Research Bot",
    capabilities=["text"],
)

@agent.on_message
async def handle(message, ctx):
    briefing = await ctx.memory.get_briefing()
    async with ctx.stream() as stream:
        async for token in my_llm.generate(message.text, briefing):
            stream.delta(token)
    return None  # stream already sent

agent.connect()
```

Package structure mirrors TypeScript. Uses `websockets` library for transport, `cryptography` for Ed25519.

## Reference Connectors

### Connector 1: Claude Code (highest value)

Bridges a running Claude Code session via the Agent SDK.

```
iOS App → AINM Server → Claude Code Connector → Claude Agent SDK → Claude Code session
                                                                    (codebase, tools, terminal)
```

**Integration path:** Uses Claude Code's Agent SDK `query()` function for message exchange. Streams tokens via `async for msg in client.receive_response()` mapped to AINM `StreamDelta` events. Forwards tool call events (file edits, bash commands, grep) as `ainm.tool.call.*` notifications.

**On connect:** Pulls user's compound memory briefing and injects as system context alongside the session's existing CLAUDE.md and codebase context.

**Runs as:** A small Node process alongside the Claude Code session. Started with `npx @ainm/connector-claude-code --server wss://my-server:8080/ws`.

### Connector 2: OpenAI-Compatible (broadest reach)

Covers OpenAI, Ollama, Together, Groq, any OpenAI-compatible API.

```
iOS App → AINM Server → OpenAI Connector → POST /v1/chat/completions (streaming)
```

**Integration path:** Configurable `base_url`, `model`, `api_key`. Maps AINM message history to OpenAI chat format. Streams SSE responses as `StreamDelta`. Maps OpenAI function calling to AINM tool events.

**Config-driven agents:** Each agent is a config entry (name, provider URL, model, system prompt, API key). Adding "My Local Llama" is adding a config file, not writing code.

**Runs as:** `npx @ainm/connector-openai --config ./agents.json --server wss://my-server:8080/ws`.

### Connector 3: MCP Bridge (ecosystem play)

Any MCP-compatible server becomes an AINM agent with zero code.

```
iOS App → AINM Server → MCP Bridge → MCP Server (stdio or HTTP)
```

**Integration path:** The bridge is an MCP client that connects to an MCP server and translates between protocols. MCP tools become AINM tools. MCP resources become AINM content blocks. MCP prompts become available via AINM message exchange.

**Runs as:** `npx @ainm/connector-mcp --server wss://my-server:8080/ws --mcp "npx -y @modelcontextprotocol/server-filesystem /path"`.

## Server Changes

### Multi-Agent Routing

**AgentRegistry** (mirrors existing HumanRegistry):

```rust
pub struct AgentRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<UnboundedSender<String>>>>>,
    agent_cards: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}
```

- Agents register on WebSocket connect (identity no longer requires "helper" prefix).
- Agent Card cached on registration, served at `/.well-known/ainm/{identity}/agent-card.json`.
- Thread routing: when a human sends a message in a thread, the server looks up which agents are participants and delivers to those specific agent connections.

### Agent Identity Detection

Replace the `from_addr.starts_with("helper")` heuristic with capability-based detection. During `ainm.initialize`, the `params.capabilities` object includes a `participant_type` field: `"human"` or `"agent"`. The server uses this to route to the appropriate handler. Existing clients that omit this field default to `"human"`.

### Thread Agent Assignment

When a human creates a thread, they specify which agent(s) to include (via the iOS Agent Discovery UI). The server records thread participants and routes messages accordingly. An agent can also be invited mid-thread.

### Briefing on Agent Join

When an agent joins a thread (first message routed to them), the server auto-generates a compound memory briefing and delivers it as an `ainm.memory.briefing` notification before the first message. The agent connector receives this and injects it as context.

### No Other Server Changes

The compound memory operations, streaming protocol, tool visibility, content blocks, federation — all already work. The server is ready for real agents.

## Deliverables and Sequence

| Phase | Deliverable | Effort | What it enables |
|-------|-------------|--------|-----------------|
| **A1** | TypeScript Connector SDK (`@ainm/connector`) | 1-2 weeks | Any developer can build an AINM agent |
| **A2** | Claude Code connector (`@ainm/connector-claude-code`) | 3-5 days | Message your Claude Code session from your phone |
| **A3** | OpenAI-compatible connector (`@ainm/connector-openai`) | 2-3 days | Connect GPT, Ollama, Together, Groq agents |
| **A4** | Multi-agent server routing (AgentRegistry) | 3-5 days | Multiple agents connected, user picks per thread |
| **A5** | Python Connector SDK (`ainm-connector`) | 1 week | Python ecosystem first-class support |
| **A6** | MCP Bridge connector (`@ainm/connector-mcp`) | 3-5 days | Any MCP server becomes an AINM agent |

**Critical path:** A1 → A2 produces the demo that sells the vision. A4 should happen in parallel once A1 is started.

## Research Insights Incorporated

### From Existing Projects

- **MCP SDK:** `connect(transport)` decoupling pattern adopted. Transport is pluggable interface.
- **Claude Agent SDK:** Two-tier API (simple `query()` vs advanced streaming) adopted.
- **Codex app-server:** Already speaks JSON-RPC over WebSocket. Nearly trivial to bridge.
- **A2A Bridge pattern:** AINM-as-MCP-server bridge enables zero-code agent connectivity.
- **Vercel open-agents:** Agent-sandbox separation, durable execution, stream recovery, tool approval gates.
- **GitHub Agent HQ:** Validates multi-agent-from-one-interface thesis commercially.

### From Academic/Industry Research

- **Graphiti/Mem0:** Two-way briefings (agents annotate/correct memory, not just consume).
- **Stanford Generative Agents:** Importance scoring on memories for briefing relevance ranking.
- **LACP paper:** Telecom-layered protocol model validates AINM's transport/identity/threading layers.

### Competitive Position

No existing project provides open federated messaging where humans and agents are equal participants with cryptographic identity, persistent compound memory, and rich interactive content blocks. AINM fills a gap that GitHub Agent HQ, Vercel Open Agents, BeeAI, and OpenAgents all leave open.

## Success Criteria

1. A user runs `npx @ainm/connector-claude-code --server wss://localhost:8080/ws` alongside their Claude Code session.
2. They open the iOS app, see "Claude Code" in their agent list, tap to start a thread.
3. They type "review my latest PR" — Claude Code responds with streaming analysis, file references, and tool call visibility.
4. The compound memory compiles facts from the conversation.
5. They switch to a different agent (GPT-4o via OpenAI connector). That agent pulls the briefing and already knows their tech stack and preferences.
6. Both agents are visible in the app simultaneously. The user messages whichever they need.

## Non-Goals (for Phase A)

- Visual graph rendering in iOS (polish, not blocking)
- Federation between multiple AINM servers (already works, not the focus)
- Agent-to-agent communication within threads (future phase)
- Web client (iOS-first)
- Billing/usage tracking
