# Technology Stack: v3.3 Universal Agent Client

**Project:** AI-Native Messaging Protocol — v3.3 Universal Agent Client
**Researched:** 2026-04-18
**Scope:** NEW stack additions for v3.3. Does NOT re-research existing Rust server, iOS client, or TS `@ainm/connector` SDK (all shipped in v3.1/v3.2).
**Confidence:** HIGH for Python/MCP/OpenAI libraries (verified via PyPI, npm, official docs, April 2026). MEDIUM for iOS live-update primitive (no spec primitive exists yet — must be added). MEDIUM for connector auth pattern (industry precedents clear; specific design is a decision for the roadmap, not a library pick).

---

## Existing Stack (DO NOT CHANGE)

These are shipped and must remain wire-compatible with everything new:

| Layer | Technology | Locked Because |
|-------|-----------|-----------------|
| Server | Rust + Axum 0.8 / Tokio 1.49 / ring 0.17 / rustls 0.23 / sqlx 0.8 | Shipped v3.1, HumanRegistry/AgentRegistry live |
| iOS client | Swift 6.1 / SwiftUI / GRDB 7.10 / CryptoKit Curve25519 / URLSessionWebSocketTask | Shipped v3.2 |
| TS SDK | `@ainm/connector` (Node 20+, `@noble/ed25519` 2.2, `@noble/hashes` 2.2, `canonicalize` 2.0, `ws` 8.18) | Shipped A1; Python SDK MUST match this wire format exactly |
| Wire protocol | JSON-RPC 2.0 over WebSocket, Ed25519 signatures over RFC 8785 JCS-canonicalized envelopes, base64url-encoded signatures | Non-negotiable — every new connector must match |
| Handshake | `ainm.initialize` → `ainm.initialized`, `signInit` payload = `${identity}|${timestamp}` signed with identity keypair | See `packages/connector/src/protocol/handshake.ts` |

Any divergence across languages in JCS canonicalization, base64url encoding, or signature payload construction will silently break message verification. All new SDKs must produce byte-identical signatures to the TS SDK for the same envelope.

---

## 1. Python Connector SDK (A5)

**Goal:** Feature parity with `@ainm/connector` TypeScript SDK. Target Python 3.12+ (dropping 3.10/3.11 simplifies type hints and lets us use `tomllib`, `TypeAliasType`, PEP 695 generics).

### Core Libraries

| Library | Version | Purpose | Why This One |
|---------|---------|---------|--------------|
| `websockets` | 16.0.x | WebSocket client over asyncio | The canonical asyncio WebSocket library. Pure asyncio (no aiohttp dependency). Simple `async with connect(url) as ws` API. Same developer community as CPython asyncio. Handles RFC 6455 correctly including ping/pong, close codes, backpressure. Used by FastAPI's test client. |
| `rfc8785` | 0.1.4 | RFC 8785 JCS canonicalization | Pure-Python, zero dependencies, by Trail of Bits. This is the only thing that MUST match the TS `canonicalize` package byte-for-byte — Trail of Bits maintains a strict RFC 8785 conformance test suite. Supports Python 3.8+, works on 3.12. |
| `cryptography` | 44.x (LTS) | Ed25519 signing/verification | PyCA `cryptography` (not PyNaCl). Rationale below. Provides `Ed25519PrivateKey` / `Ed25519PublicKey` with `sign(data)` / `verify(sig, data)`. Hardware-accelerated via OpenSSL. Wheels published for Python 3.12, 3.13, 3.14. Regularly audited; used by the entire Python cryptographic ecosystem (pip, cryptography signatures, TLS). |
| `httpx` | 0.28.x | HTTP client for Agent Card fetching / health checks / OpenAI calls | Async-first, HTTP/2 support, modern. Same API as `requests` but `async`/`await` native. Pairs naturally with `websockets`. |
| `anyio` | 4.x | Structured concurrency + cross-runtime compat | Used internally by `httpx` and most modern async Python. Gives us task groups, timeouts, cancel scopes. Keeps us compatible with Trio users who may embed the SDK. |
| `pydantic` | 2.9+ | Message/envelope typing + validation | Runtime validation of incoming JSON against protocol schemas. Generates JSON Schema from models (we can round-trip against the canonical AINM schemas). Fast (Rust core). Developer ergonomics match `@noble` + manual TS types. |
| `fastapi` + `uvicorn` | FastAPI 0.115.x / uvicorn 0.41.x | HTTP server for Agent Card serving | Connector needs to serve `/.well-known/agent.json` for discovery + potentially health endpoints. FastAPI is the de-facto async web framework; uvicorn is the standard ASGI runner. Overkill for one endpoint but gives us OpenAPI, CORS, and graceful shutdown for free. |

**Why PyCA `cryptography` over PyNaCl for Ed25519:**

PyNaCl is faster for pure signing (libsodium C implementation), but:
1. `cryptography` is already a transitive dep of almost every Python project (via requests, httpx, urllib3 in TLS paths) — adding PyNaCl means two crypto libraries in the venv
2. `cryptography` follows the modern Ed25519 API shape (`Ed25519PrivateKey.from_private_bytes(32)`) that matches Apple CryptoKit and Rust `ring` conceptually — easier to mental-map across the three reference implementations
3. `cryptography` wheels ship for free-threaded Python 3.13t+ which matters for high-concurrency connector scenarios
4. Performance difference (~2-3x) is irrelevant: AINM signs one envelope per message, not millions/sec. Not on a hot path.

**Why `websockets` over `aiohttp`:**

`aiohttp` bundles an HTTP server + client + WebSocket client. We don't want the HTTP server (FastAPI covers that). For a connector SDK, `websockets` is smaller, narrower, and API-stable. `aiohttp` performance advantage (~2x) doesn't matter — a connector handles 1-10 messages/sec from its paired agent, not 100K.

### Matching the TypeScript SDK Shape

The TS SDK exposes `AINMAgent` with `.onMessage()`, `.registerTool()`, `.connect()`, context includes `.stream()`, `.memory`, `.sendMessage()`, `.emitToolStarted/Progress/Completed()`. The Python SDK must mirror this:

```python
from ainm_connector import AINMAgent, IncomingMessage, MessageContext

agent = AINMAgent(
    server="ws://localhost:8080/ws",
    identity="claude@localhost",
    name="Claude",
    capabilities=["text", "code"],
)

@agent.on_message
async def handle(msg: IncomingMessage, ctx: MessageContext) -> str | None:
    async with ctx.stream() as stream:
        async for delta in llm_response():
            await stream.delta(delta)
    return None

await agent.connect()
await agent.run_forever()
```

Decorator style (`@agent.on_message`) is idiomatic Python; callback style (`agent.on_message(fn)`) also supported to stay close to TS.

### Installation

```bash
# Python 3.12+ required
pip install ainm-connector  # Package will be published to PyPI as part of A5

# Pinned in pyproject.toml:
# websockets = "^16.0"
# rfc8785 = "^0.1.4"
# cryptography = "^44.0"
# httpx = "^0.28"
# anyio = "^4.0"
# pydantic = "^2.9"
# fastapi = "^0.115"
# uvicorn = "^0.41"
```

**Sources:**
- [websockets 16.0 documentation](https://websockets.readthedocs.io/en/stable/) — HIGH
- [rfc8785 PyPI 0.1.4](https://pypi.org/project/rfc8785/) — HIGH (verified April 2026)
- [PyCA cryptography](https://cryptography.io/en/latest/hazmat/primitives/asymmetric/ed25519/) — HIGH
- [httpx async docs](https://www.python-httpx.org/async/) — HIGH

---

## 2. MCP Bridge Connector (A6)

**Goal:** A connector binary that takes an MCP server URL/stdio command as config, connects to it as an MCP client, and exposes the MCP server's tools/resources/prompts as AINM tool calls in threads.

### Core Libraries

| Library | Version | Language | Purpose | Why |
|---------|---------|----------|---------|-----|
| `@modelcontextprotocol/sdk` | 1.29.0 | TypeScript/Node | MCP client for the bridge connector | Official SDK (modelcontextprotocol/typescript-sdk), 42K+ dependents on npm as of April 2026. Provides `Client` class with `.connect(transport)`, `.listTools()`, `.callTool()`, `.listResources()`, `.listPrompts()`. Multiple transports: `StdioClientTransport`, `SSEClientTransport`, `StreamableHTTPClientTransport`. |
| `mcp` | 1.27.0 | Python | Alternative MCP client (Python SDK) | Official SDK (modelcontextprotocol/python-sdk). Released April 2, 2026. Requires Python 3.10+. Provides `ClientSession` + `streamablehttp_client` / `stdio_client`. Use this only if we decide to ship an all-Python MCP bridge; otherwise the TS version suffices. |

**Implementation choice: build the MCP bridge in TypeScript** (colocated with `@ainm/connector-claude-code` and `@ainm/connector-openai`). Reasons:
1. All three existing connectors are TypeScript — single build/test toolchain
2. The TS MCP SDK is more mature (1.29 vs 1.27) and has 10x the community usage
3. MCP servers are often JS/Node tools; keeping the bridge in the same runtime is ergonomically symmetric
4. Python MCP SDK becomes available to ALL A5 users automatically — they can build custom MCP-aware Python connectors without using our bridge

### MCP Capability Discovery Flow

Canonical MCP discovery after `initialize`:

```typescript
await client.connect(transport);               // MCP initialize handshake
const tools = await client.listTools();        // { tools: [{ name, description, inputSchema }, ...] }
const resources = await client.listResources(); // { resources: [{ uri, name, mimeType }, ...] }
const prompts = await client.listPrompts();     // { prompts: [{ name, description, arguments }, ...] }
```

Each MCP tool (`{name, description, inputSchema}`) maps 1:1 to an AINM tool registration via `agent.registerTool(name, definition, handler)`. The handler proxies to `client.callTool({ name, arguments })` and emits `ainm.tool.started` / `ainm.tool.completed` notifications.

### Transports We Must Support

| MCP Transport | When Used | Bridge Support |
|---------------|-----------|----------------|
| stdio | Local MCP servers launched as subprocesses (e.g., `npx @modelcontextprotocol/server-filesystem`) | REQUIRED — most common MCP deployment |
| Streamable HTTP (`StreamableHTTPClientTransport`) | Remote MCP servers, production deployments. Recommended transport per MCP spec | REQUIRED — this is the future-proof transport |
| SSE (`SSEClientTransport`) | Legacy remote MCP servers | NICE-TO-HAVE — adding ~20 LOC; skip if out of scope |

### Streaming Tool Results

MCP does not have granular token-by-token streaming for tool calls the way LLM APIs do. A tool call returns a `CallToolResult` with `content` blocks (text/image/resource) when complete. The bridge should:

1. Emit `ainm.tool.started` immediately with input args
2. Emit `ainm.tool.progress` at a coarse interval (e.g., every 2s) for long-running tools, with no progress value (MCP doesn't expose this unless the tool itself streams via a resource subscription)
3. Emit `ainm.tool.completed` with the final result when `callTool` resolves

For MCP servers that stream via `resources/subscribe`, the bridge can forward those as `ainm.stream.delta` events — out of scope for initial A6.

### Installation

```bash
# In packages/connector-mcp/ (new package):
npm install @modelcontextprotocol/sdk @ainm/connector
```

**Sources:**
- [MCP Python SDK on PyPI v1.27.0](https://pypi.org/project/mcp/) — HIGH (verified April 2026)
- [@modelcontextprotocol/sdk v1.29.0](https://www.npmjs.com/package/@modelcontextprotocol/sdk) — HIGH (verified April 2026)
- [MCP TypeScript SDK GitHub](https://github.com/modelcontextprotocol/typescript-sdk) — HIGH
- [MCP streamable HTTP transport](https://blog.cloudflare.com/streamable-http-mcp-servers-python/) — MEDIUM

---

## 3. OpenAI Responses API Upgrade

**Goal:** Replace the current `chat/completions` implementation in `packages/connector-openai/` with the Responses API. This unlocks real tool calling and the recommended 2026 path.

**Verification:** The Responses API is the official current recommended endpoint (April 2026, not a beta). The Assistants API has been formally deprecated with shutdown on 2026-08-26 — confirmed in OpenAI's migration guide. Chat Completions remains supported but Responses is what all new OpenAI features ship into first (remote MCP servers, tool search, reasoning-preserved tool calls on o3/o4-mini, WebSocket mode).

### Core Libraries

| Library | Current Version (April 2026) | Purpose | Why |
|---------|-----|---------|-----|
| `openai` (Node) | 6.34.0 | OpenAI Node SDK | Version 6.x has first-class `client.responses.create()` API. Verified latest on npm April 19, 2026. Provides typed streaming events via typed `for await` iteration. Minimum Node 18, we're on 20+. |
| `openai` (Python) | ≥1.54.x | OpenAI Python SDK | If we ship a Python OpenAI connector under A5, same SDK. Both Node and Python SDKs have identical Responses API surface. |

### Wire Shape (New vs Old)

**Old (chat/completions):**
```typescript
POST /chat/completions
{
  model, stream: true,
  messages: [{role: "system", content}, {role: "user", content}]
}
// SSE stream of choices[0].delta.content chunks; custom SSE parsing
```

**New (Responses):**
```typescript
const stream = await client.responses.create({
  model: "gpt-5.2",
  input: [{role: "user", content: [{type: "input_text", text: msg.text}]}],
  tools: [/* function + remote MCP + hosted web_search + code_interpreter */],
  stream: true,
});
for await (const event of stream) {
  switch (event.type) {
    case "response.created":        // Response started
    case "response.output_text.delta":  // Text delta → ctx.stream().delta(event.delta)
    case "response.function_call_arguments.delta":  // Tool args streaming in
    case "response.function_call_arguments.done":   // Tool call ready to execute
    case "response.completed":      // Full response done → ctx.stream().end()
    case "error":
  }
}
```

### Tool Calls: Client-Side vs Hosted

Responses API supports BOTH:

| Tool Type | Execution | Relevant for AINM |
|-----------|-----------|-------------------|
| `function` | Client-side — AINM connector catches `function_call_arguments.done`, executes tool (e.g., forwards to AINM tool registry), POSTs result back via `client.responses.create` with `previous_response_id` + `function_call_output` | YES — this is how AINM-registered tools work |
| `web_search_preview` | Hosted by OpenAI — model calls, no client work | Useful; forward results as content blocks to thread |
| `code_interpreter` | Hosted by OpenAI | Nice-to-have; gated behind org approval |
| `mcp` | Hosted — OpenAI calls a remote MCP server URL directly (the "New tools in Responses API" feature) | INTERESTING for compound memory: we could let OpenAI models directly call the AINM server's memory endpoints via MCP, reducing connector complexity |

### Threading / State

Responses API has its own conversation state primitive: `previous_response_id` chains responses into a thread server-side, no local history management needed. This is optional — we can still pass the full `input` array ourselves. For AINM we SHOULD pass the full thread context from the AINM thread (so conversations can migrate between agent providers) rather than relying on OpenAI-held state.

**Recommendation for AINM OpenAI connector:**
- Use `client.responses.create({stream: true, input: [...threadHistory]})` every turn
- Do NOT use `previous_response_id` (would lock thread state to OpenAI, contra the universal-client vision)
- Map `response.output_text.delta` events → `ctx.stream().delta()`
- Map `function_call_arguments.done` events → `ctx.emitToolStarted` + dispatch to tool handler + `ctx.emitToolCompleted`, then submit the tool output as a new `responses.create` with `function_call_output` input items

### Installation

```bash
# In packages/connector-openai/
npm install openai@^6.34.0
# Drop custom fetch+SSE parsing; let the SDK own streaming
```

**Sources:**
- [Migrate to the Responses API — OpenAI](https://developers.openai.com/api/docs/guides/migrate-to-responses) — HIGH
- [Streaming API responses](https://developers.openai.com/api/docs/guides/streaming-responses) — HIGH
- [Responses streaming events reference](https://developers.openai.com/api/reference/resources/responses/streaming-events) — HIGH
- [openai npm 6.34.0](https://www.npmjs.com/package/openai) — HIGH (verified April 19, 2026)
- [Assistants API deprecation](https://platform.openai.com/docs/assistants/migration) — HIGH (shutdown 2026-08-26)
- [New tools in Responses API (MCP, tool search)](https://openai.com/index/new-tools-and-features-in-the-responses-api/) — HIGH

---

## 4. Connector Auth / Identity Hardening

**Goal:** Move beyond "trust-by-connection". A connector claiming to be `claude@acme.example` must prove it holds the private key for that identity.

### Current State

The SDK ALREADY does `signInit` during handshake: on `ainm.initialize`, the connector sends `${identity}|${timestamp}` signed with its Ed25519 private key. See `packages/connector/src/identity/signing.ts` and `packages/connector/src/protocol/handshake.ts`. The problem is the server currently accepts any signature or accepts unknown identities — there is no server-side pubkey registry binding.

### Recommended Pattern: Server-Side Signed-Nonce Handshake with Pre-Registered Identity

This is a proven pattern (Matrix appservices, Fediverse, Bluesky, SSH host keys). NO new library needed — just extends existing `ring` + `@noble/ed25519` crypto already in use.

**Flow:**

1. **Identity registration (out-of-band, one-time):** Operator registers an agent identity on the server: `claude@acme.example` bound to Ed25519 public key `ed25519:BASE64URL...`. Stored in AgentRegistry's identity table.
2. **Handshake challenge:** On `ainm.initialize`, server responds with `nonce: <random 32 bytes base64url>` + `server_timestamp` in the result (the existing `HandshakeResult.nonce` field in `handshake.ts` is already a placeholder for this).
3. **Handshake proof:** Connector signs `${identity}|${server_nonce}|${server_timestamp}` with private key, sends in a follow-up `ainm.authenticate` RPC.
4. **Server verifies:** Server looks up registered pubkey for identity, verifies signature, binds the WebSocket connection to that identity. All subsequent messages must be signed by the same keypair (we already do per-message signing).

**Why signed-nonce, not JWT / OAuth / mTLS:**

| Pattern | Why Not for v3.3 |
|---------|-------------------|
| JWT bearer tokens | Bearer theft risk. Short-lived JWTs require refresh infra. Most importantly: we ALREADY have Ed25519 keypairs as the identity primitive — adding JWT is redundant auth. |
| OAuth2 device flow | Great for user-facing agents (ChatGPT authenticating to external APIs) but overkill for machine-to-server auth where the machine has a keypair. Needed only if we want human-operator flows (e.g., Claude Code user auth). |
| mTLS | Strongest but hardest: client cert provisioning, Let's Encrypt doesn't do client certs, cert rotation infrastructure. Heavy operational cost. Worth it for future server-to-server federation (already noted in PITFALLS-federation.md). |
| Pre-shared API keys | Symmetric secrets = distribute-and-protect problem. We ruled this out in v3.1 by choosing Ed25519. Don't walk it back. |

**Industry precedents:**

- **Matrix appservices:** Use `hs_token` (homeserver→appservice) + `as_token` (appservice→homeserver) static bearer tokens. Works but is pre-modern; we can do better with Ed25519 since iOS CryptoKit + Rust ring + TS @noble are all deployed.
- **Slack Socket Mode:** App token (`xapp-...`) + bot token (`xoxb-...`) — static bearers. Simple but subject to leakage. Slack's model is what we're trying to improve on.
- **Fediverse/ActivityPub HTTP Signatures:** RFC 9421 — sign a digest of the request with the sender's private key; receiver fetches sender's public key via WebFinger. Directly analogous to AINM's design. Our per-message signing is already doing this at WebSocket-message granularity.
- **Bluesky AT Protocol:** DID keys + signed requests. Same philosophy.

**For Claude Code specifically (OAuth-vs-API-key):** The upstream Claude Code CLI authenticates the *human user* to Anthropic via OAuth2 device flow. The AINM Claude Code connector authenticates the *agent identity* to the AINM server via Ed25519. These are orthogonal. Document both in the A6 hardening plan.

### Libraries Needed

| Library | Where | Purpose |
|---------|-------|---------|
| `ring` 0.17 | Server (existing) | Ed25519 verification against registered pubkeys |
| `@noble/ed25519` 2.2 (existing) / `cryptography` 44 (Python) | Connector SDKs | Signing the nonce |
| None new | — | This is a protocol change + a Rust server change, not a new dependency |

### Nonce/Replay Defenses

- Nonces must be 256 bits (32 bytes) from `ring::rand::SystemRandom` or equivalent
- Server must remember used nonces per identity with TTL matching `server_timestamp` skew window (recommend 5 min)
- `server_timestamp` must be checked for ±5 min clock skew — reject old proofs
- WebSocket connection binding prevents connection-ID reuse across sessions

**Sources:**
- [Matrix Application Service API](https://spec.matrix.org/v1.14/application-service-api/) — HIGH
- [API Authentication 2026: OAuth/JWT/API Keys/mTLS](https://www.elysiate.com/blog/api-authentication-oauth-jwt-api-keys-compared) — MEDIUM
- [AI Agent Authentication draft-klrc-aiagent-auth](https://dev.to/kanywst/ai-agent-authentication-authorization-deep-dive-reading-draft-klrc-aiagent-auth-00-5d1) — MEDIUM
- [HTTP Signatures RFC 9421](https://datatracker.ietf.org/doc/rfc9421/) — HIGH

---

## 5. iOS Live Agent Discovery

**Goal:** The iOS Agent Discovery view currently shows a snapshot. It should subscribe to AgentRegistry changes so agents appear/disappear in real time.

### Server-Side: New Protocol Primitive Needed

**Finding:** There is NO existing `ainm.agents.updated` or `ainm.registry.subscribe` notification in the current spec (v3.1/v3.2). This is a spec-level addition required by v3.3 P19 (iOS wiring work).

**Recommended primitive (to add to the protocol):**

```
// Client → Server (JSON-RPC request)
method: "ainm.agents.subscribe"
params: { scope?: "all" | "thread" | ["thread_id_1", ...] }
→ response: { subscription_id, agents: [AgentCard, ...] }  // initial snapshot

// Server → Client (JSON-RPC notification)
method: "ainm.agents.updated"
params: {
  subscription_id,
  change: "connected" | "disconnected" | "updated",
  agent: AgentCard,  // full card for connected/updated; minimal {identity} for disconnected
}

// Client → Server
method: "ainm.agents.unsubscribe"
params: { subscription_id }
```

This mirrors the existing `ainm.thread.subscribe` / `ainm.stream.delta` patterns so implementers see a consistent shape.

**Server implementation:** Rust — AgentRegistry already fires internal events on agent connect/disconnect (see `project_server_architecture.md` memory note). Need to:
1. Add a `broadcast::Sender<AgentRegistryEvent>` alongside the registry
2. On subscribe, spawn a task that forwards events filtered by scope to the client's WebSocket
3. On WebSocket close, drop the subscription

No new crates needed — `tokio::sync::broadcast` is already in the dep graph.

### iOS Client-Side: No New Libraries

The existing iOS stack (SwiftUI `@Observable`, `URLSessionWebSocketTask`, `AsyncStream`, GRDB) covers this. Pattern:

```swift
@Observable final class AgentRegistryStore {
    private(set) var agents: [AgentCard] = []
    private var subscriptionId: String?

    func start(client: AINMClient) async throws {
        let result = try await client.request("ainm.agents.subscribe", params: ["scope": "all"])
        self.subscriptionId = result["subscription_id"] as? String
        self.agents = decode([AgentCard].self, result["agents"])

        // AINMClient exposes a notification stream we already use for message delivery
        for await notif in client.notifications(method: "ainm.agents.updated") {
            handle(notif)
        }
    }

    private func handle(_ notif: ServerNotification) {
        switch notif.change {
        case "connected", "updated": upsert(notif.agent)
        case "disconnected": remove(notif.agent.identity)
        }
    }
}
```

The SwiftUI view uses `@State` on `AgentRegistryStore` and the list auto-updates because `@Observable` + the view's `onAppear` handle the subscription lifecycle.

**Caveats:**
- Reconnect handling: on WebSocket close, the store must re-subscribe on reconnect (AINMClient already auto-reconnects per v3.2)
- Initial snapshot vs. delta race: we use request/response for snapshot and then stream deltas from that instant forward — no reliable "include deltas since X" semantics needed

### Libraries Needed

None new. Already on iOS:
- SwiftUI `@Observable` (iOS 17+, shipped v3.2)
- `URLSessionWebSocketTask` (shipped v3.2)
- `AsyncStream` (shipped v3.2)

Already on server:
- `tokio::sync::broadcast` (already in Cargo.lock)

**Sources:**
- [SwiftUI Real-Time Data Architecture with WebSockets](https://dev.to/sebastienlato/swiftui-real-time-data-architecture-websockets-streams-consistency-310n) — MEDIUM
- [Real-time with WebSockets and Swift Concurrency](https://medium.com/@thomsmed/real-time-with-websockets-and-swift-concurrency-8b44a8808d0d) — MEDIUM
- AINM `project_server_architecture.md` (memory) — HIGH for internal state

---

## 6. Compound Memory Groundwork

**Goal:** Connector SDK ergonomics for pushing memory context back to the server. Existing spec has `ainm.memory.store` / `ainm.memory.query` / `ainm.memory.evict`; the question is the SDK API shape and what to learn from Letta/Mem0.

### API Shape: Explicit + Helper, Not Auto-Extract

**Recommendation: two-tier API, no LLM-powered auto-extraction in v3.3.**

**Tier 1 — Explicit store (all languages, matches TS existing):**

```typescript
// TS
await ctx.memory.store({
  kind: "fact" | "preference" | "decision",
  content: "User prefers Python over TypeScript for data science",
  subject: "user:alice@acme",
  confidence: 0.9,
  source_message_id: msg.id,
});

const briefing = await ctx.memory.getBriefing({ subject });  // hydrate fresh agent context
const results = await ctx.memory.query({ subject, kind: "preference", limit: 10 });
```

```python
# Python (A5)
await ctx.memory.store(
    kind="fact",
    content="User prefers Python over TypeScript for data science",
    subject="user:alice@acme",
    confidence=0.9,
    source_message_id=msg.id,
)

briefing = await ctx.memory.get_briefing(subject=subject)
results = await ctx.memory.query(subject=subject, kind="preference", limit=10)
```

**Tier 2 — Framework helper (optional import for agents that do structured extraction):**

```typescript
import { extractFacts } from "@ainm/connector/memory-helpers";
// Thin wrapper: prompts an LLM with a fact-extraction system prompt, returns {facts: Array<{kind, content, confidence}>}
// The CONNECTOR AUTHOR opts in, SDK doesn't silently call an LLM
const facts = await extractFacts({ messages: threadHistory, llm: openaiClient });
await Promise.all(facts.map(f => ctx.memory.store(f)));
```

### What NOT to Copy from Mem0 / Letta

| Their Pattern | Why Not for AINM v3.3 |
|---------------|-----------------------|
| Mem0's `add()` auto-extracts via background LLM | Couples SDK to an LLM provider. Makes memory latency + cost opaque. Hides the fact that facts are being stored, breaking the tool-call-transparency principle (users should SEE memory writes in the thread). |
| Letta's "agents self-edit memory via tools" | That's exactly what AINM tool-call-transparency + explicit `memory.store` already achieves. Agents CAN register a "remember this" tool that users see fire in the UI. This is a BETTER architecture than Letta's implicit self-editing because it's auditable in the thread. |
| Mem0's LLM-powered deduplication | Belongs on the SERVER, not the SDK. Can be added in v3.4 compound memory aggregation. |
| Letta's virtual-memory paging (RAM/disk metaphor) | Already covered by AINM's existing 3-tier memory (active/conversation/archival). Don't re-invent. |

### What to Borrow

- **Letta's three-tier model (active/conversation/archival)**: already in AINM spec — keep
- **Mem0's subject-centric query model** (`memory.query(user_id=...)`): already in AINM as `subject:` field — keep
- **Both frameworks' rejection of "dump the whole conversation into RAG"**: reinforced; AINM memory is structured facts, not chunks

### Libraries Needed

**None new.** The memory API is plain JSON-RPC over the existing WebSocket. The SDK adds helpers in user-space.

For the optional `extractFacts` helper (Tier 2), we'd use whatever LLM the connector author is already using (the Claude Code connector has Claude access, the OpenAI connector has OpenAI access) — no new dep.

**Sources:**
- [Mem0 Add Memory docs](https://docs.mem0.ai/core-concepts/memory-operations/add) — HIGH
- [Letta Agent Memory concepts](https://docs.letta.com/concepts/memgpt/) — HIGH
- [Mem0 vs Letta 2026 comparison](https://vectorize.io/articles/mem0-vs-letta) — MEDIUM
- AINM v3.1 `memory.md` spec — HIGH (internal, already shipped)

---

## Consolidated Installation

```bash
# Python Connector SDK (A5) — new package `ainm-connector` on PyPI
pip install websockets==16.0.* rfc8785==0.1.* cryptography==44.0.* httpx==0.28.* \
            anyio==4.* pydantic==2.9.* fastapi==0.115.* uvicorn==0.41.*

# TS MCP Bridge Connector (A6) — new package packages/connector-mcp
npm install @modelcontextprotocol/sdk@^1.29.0 @ainm/connector

# OpenAI Responses API upgrade (existing packages/connector-openai)
npm install openai@^6.34.0  # replaces custom fetch+SSE code

# Connector auth hardening — NO new deps (extends existing Ed25519 crypto)
# iOS live Agent Discovery — NO new deps (extends existing client + server)
# Compound memory groundwork — NO new deps (protocol-level, SDK ergonomic wrappers)
```

---

## Version Compatibility Matrix

| Package | Version | Compatible With | Notes |
|---------|---------|-----------------|-------|
| `websockets` 16.0 | Python 3.12+ | `anyio` 4.x, `cryptography` 44.x | Pure asyncio, no aiohttp dep |
| `rfc8785` 0.1.4 | Python 3.8+ | — | MUST produce byte-identical output to TS `canonicalize` 2.0 — verify with cross-language conformance test in A5 |
| `cryptography` 44 | Python 3.10+ | OpenSSL 3.x | LTS series. Free-threaded Python 3.13t wheels available |
| `pydantic` 2.9 | Python 3.9+ | `fastapi` ≥0.110 | v2 Rust core; v1 is EOL |
| `@modelcontextprotocol/sdk` 1.29 | Node 18+ | `@ainm/connector` 0.1+ | MCP v2 is anticipated Q1 2026 but v1.x remains stable/recommended |
| `openai` (Node) 6.34 | Node 18+ | — | `chat/completions` still works; `responses.create` is the recommended path |
| `openai` (Python) 1.54+ | Python 3.8+ | — | Same surface as Node SDK |

---

## What NOT to Add

| Avoid | Reason | Use Instead |
|-------|--------|-------------|
| `aiohttp` | Bundles HTTP server + client + WebSocket; redundant with `fastapi` + `websockets` + `httpx` split | Narrow libraries by responsibility |
| `PyNaCl` | Second crypto lib in venv; benchmark advantage irrelevant at AINM message rates | `cryptography` 44 |
| `requests` | Sync-only; the SDK is async end-to-end | `httpx` 0.28 |
| OpenAI Assistants API | Deprecated; shutdown 2026-08-26 | Responses API |
| `openai-streams` npm package | Unofficial; the official SDK now handles streams natively | `openai` 6.x built-in streaming |
| Custom JWT session tokens | Duplicates the Ed25519 keypair we already use for identity | Signed-nonce handshake (section 4) |
| mTLS for connector-to-server | Certificate lifecycle cost not justified for v3.3; reserve for v3.x federation work | Signed-nonce handshake |
| Mem0 or Letta as a direct dependency | Couples SDK to external memory service; AINM memory IS the product, not a wrapper | Native `ainm.memory.*` RPCs (already shipped) |
| LangChain / LangGraph integrations in the core SDK | Heavy, versionful, opinionated | Keep SDK framework-agnostic; any LangChain-AINM adapter is a separate package |
| `socket.io` (either side) | Non-standard over-the-wire framing; we already have JSON-RPC over plain WebSocket | Plain `ws` / `websockets` |
| Protobuf / gRPC for connector↔server | Already rejected in v3.1 STACK for same reasons — debuggability, browser-future-proofing | JSON over WebSocket |
| SwiftData for storing AgentRegistry snapshot | Already rejected in v3.2 for messages; same reasons apply | GRDB (already shipped) |
| A second Python WebSocket lib (e.g., `picows`) | Speed advantage irrelevant at 1-10 msg/s; adds C build step | `websockets` 16 pure-Python |

---

## Stack Patterns by Variant

**If a connector author wants sync-only Python:**
- NOT supported. The SDK is async end-to-end (WebSocket demands it). They can wrap with `asyncio.run()` at the entry point.

**If a connector author wants to bypass the SDK:**
- Fully supported — the wire protocol is documented JSON-RPC 2.0 + Ed25519 + JCS. They can implement in any language. The SDK is sugar, not a gatekeeper.

**If a connector needs to host multiple agent identities (multi-tenant):**
- Instantiate multiple `AINMAgent` objects, one per identity, each with its own keypair. The WebSocket is per-agent.

**If a connector needs to connect to many AINM servers (federation):**
- Out of scope for v3.3. Federation is for server-to-server. Clients/connectors pair 1:1 with their home server.

---

## Sources (Consolidated)

**HIGH confidence (official/primary sources, verified April 2026):**
- [MCP Python SDK 1.27.0 on PyPI](https://pypi.org/project/mcp/)
- [@modelcontextprotocol/sdk 1.29.0 on npm](https://www.npmjs.com/package/@modelcontextprotocol/sdk)
- [MCP TypeScript SDK GitHub](https://github.com/modelcontextprotocol/typescript-sdk)
- [MCP Python SDK GitHub](https://github.com/modelcontextprotocol/python-sdk)
- [openai npm 6.34.0](https://www.npmjs.com/package/openai)
- [OpenAI Responses API migration guide](https://developers.openai.com/api/docs/guides/migrate-to-responses)
- [OpenAI streaming responses guide](https://developers.openai.com/api/docs/guides/streaming-responses)
- [OpenAI responses streaming events reference](https://developers.openai.com/api/reference/resources/responses/streaming-events)
- [OpenAI Assistants deprecation](https://platform.openai.com/docs/assistants/migration)
- [websockets 16.0 documentation](https://websockets.readthedocs.io/en/stable/)
- [rfc8785 0.1.4 on PyPI](https://pypi.org/project/rfc8785/)
- [rfc8785 Trail of Bits implementation](https://github.com/trailofbits/rfc8785.py)
- [PyCA cryptography Ed25519 docs](https://cryptography.io/en/latest/hazmat/primitives/asymmetric/ed25519/)
- [httpx async documentation](https://www.python-httpx.org/async/)
- [Matrix Application Service API spec](https://spec.matrix.org/v1.14/application-service-api/)
- [RFC 9421 HTTP Message Signatures](https://datatracker.ietf.org/doc/rfc9421/)

**MEDIUM confidence (community / secondary sources, cross-referenced):**
- [MCP streamable HTTP (Cloudflare blog)](https://blog.cloudflare.com/streamable-http-mcp-servers-python/)
- [OpenAI Responses API new tools announcement](https://openai.com/index/new-tools-and-features-in-the-responses-api/)
- [Mem0 vs Letta 2026 comparison](https://vectorize.io/articles/mem0-vs-letta)
- [Mem0 Add Memory docs](https://docs.mem0.ai/core-concepts/memory-operations/add)
- [Letta Agent Memory (MemGPT concepts)](https://docs.letta.com/concepts/memgpt/)
- [API auth 2026 — OAuth/JWT/API/mTLS](https://www.elysiate.com/blog/api-authentication-oauth-jwt-api-keys-compared)
- [mTLS vs JWT in 2026](https://medium.com/@anandjeyaseelan10/mtls-vs-jwt-what-every-enterprise-developer-should-know-in-2026-c0b42cfb8a66)
- [AI Agent Authentication draft-klrc](https://dev.to/kanywst/ai-agent-authentication-authorization-deep-dive-reading-draft-klrc-aiagent-auth-00-5d1)
- [JWTs for AI Agents (Security Boulevard)](https://securityboulevard.com/2025/11/jwts-for-ai-agents-authenticating-non-human-identities/)
- [SwiftUI + WebSocket real-time data](https://dev.to/sebastienlato/swiftui-real-time-data-architecture-websockets-streams-consistency-310n)
- [Real-time WebSockets + Swift Concurrency](https://medium.com/@thomsmed/real-time-with-websockets-and-swift-concurrency-8b44a8808d0d)
- [FastAPI deployment guide 2026](https://www.zestminds.com/blog/fastapi-deployment-guide/)
- [uvicorn 0.41 on PyPI](https://pypi.org/project/uvicorn/)

---

*Stack research for: v3.3 Universal Agent Client milestone*
*Researched: 2026-04-18*
*Overall confidence: HIGH for library versions and patterns; MEDIUM for protocol-level additions (live Agent Discovery primitive, signed-nonce handshake) which are DESIGN decisions feeding into roadmap phases, not library choices*
