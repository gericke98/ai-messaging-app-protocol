# Architecture Research: v3.3 Universal Agent Client Integration

**Domain:** AINM universal agent client — connector ecosystem integration with existing Rust server + SwiftUI iOS client
**Researched:** 2026-04-18
**Confidence:** HIGH for existing-architecture mapping (direct code read); MEDIUM for external integration points (MCP spec, OpenAI Responses API)
**Supersedes:** prior `ARCHITECTURE.md` covering v3.2 iOS reference client (v3.2 patterns are now live; this document focuses on v3.3 delta)

This document maps **how seven v3.3 concerns integrate with the existing architecture**. Each section uses explicit **NEW vs MODIFIED** tagging so the orchestrator can sequence work. Where current code already handles a concern, we say so rather than re-design it.

---

## 1. Existing-Architecture Snapshot (context, read-only)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              iOS Reference Client                            │
│  SwiftUI Views ──→ ViewModels ──→ Repositories ──→ GRDB + AINMClient         │
│                                                     │                        │
│                                                     │ URLSessionWebSocketTask│
│                                                     │ + CryptoKit (Ed25519)  │
└────────────────────────────────────────────────────┼────────────────────────┘
                                                      │  wss:// JSON-RPC 2.0
                                                      │  + RFC 8785 JCS sig
┌────────────────────────────────────────────────────┼────────────────────────┐
│                            AINM Rust Server (Axum)                            │
│                                                      ▼                        │
│   ws_upgrade_handler ──→ handshake (ainm.initialize/ainm.initialized)        │
│       │                           │                                           │
│       │                           ▼                                           │
│       │         participant_type in capabilities?                             │
│       │                ├── "agent"  ──→ handle_agent_connection               │
│       │                └── "human"  ──→ handle_human_connection               │
│       ▼                           │            │                              │
│   HumanRegistry                   │            │                              │
│   - connections: {user → [mpsc]}  │            │                              │
│   - thread_participants: {tid→[u]}│            │                              │
│                                   ▼            ▼                              │
│              AgentRegistry  to_agent_tx (broadcast)                           │
│              - connections: {id → [mpsc]}    ──▶ fan-out to all agents        │
│              - agent_cards: {id → JSON}                                       │
│                                                                               │
│   PostgreSQL (sqlx):  threads, messages, memory_nodes (user-scoped),          │
│                       memory_edges, archival_facts                            │
│                                                                               │
│   HTTP: /.well-known/ainm/{agent_name}/agent-card.json                        │
│         /health, /health/live                                                 │
└──────────────────────────────────────────────────────────────────────────────┘
                                      ▲
                                      │ outbound wss:// (NAT-friendly)
                                      │
┌─────────────────────────────────────┴────────────────────────────────────────┐
│                     @ainm/connector (TS SDK) — runs as separate Node process │
│   AINMAgent class                                                             │
│     ├── WebSocketTransport (ws)                                               │
│     ├── Keypair (@noble/ed25519 + @noble/hashes sha512)                       │
│     ├── performHandshake → signInit(identity|ISO-ts)                          │
│     ├── signEnvelope → canonicalize (npm package) + sign over canonical bytes │
│     ├── onMessage handler → MessageContext {stream, memory, tools, send}      │
│     └── registerTool → ainm.tool.invoke dispatch                              │
│                                                                               │
│   Wrappers:                                                                   │
│     @ainm/connector-claude-code  →  spawn claude -p, stream NDJSON → deltas   │
│     @ainm/connector-openai       →  fetch chat/completions SSE → deltas       │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Invariants (anchor these — do not break in v3.3):**

| Invariant | Where enforced | Consequence if broken |
|-----------|----------------|-----------------------|
| JSON-RPC 2.0 envelope on every WS frame | `buildRequest`/`buildNotification` (TS), `protocol::new_*` (Rust) | iOS client silently drops frames |
| Handshake sequence: `initialize` → response → `initialized` notification before any other method | `handle_ws_connection` loop (server.rs:483-507); `performHandshake` (TS) | Server returns `-32021` "Not initialized" |
| `participant_type: "agent"` in capabilities routes to `handle_agent_connection` | server.rs:509-522 | Connector is treated as human; agent card not registered; `to_agent_tx` broadcast not subscribed |
| RFC 8785 canonicalize-then-Ed25519-sign (no whitespace, sorted keys, signature field excluded) | `signEnvelope` (TS), `signing.rs` (Rust), `AINMSigning` (iOS) | Cross-language verification fails silently |
| Ed25519 signatures in base64url (no padding) | `base64urlEncode` (TS) | Rust verifier rejects (uses `URL_SAFE_NO_PAD`) |
| Memory graph is **user-scoped** (`user_addr`), not agent-scoped | `graph.rs` — every INSERT/SELECT filters on `user_addr` | Cross-agent memory sharing already supported; compound memory is a UX/policy layer on top |
| `to_agent_tx` is a **broadcast** channel (every agent sees every human message) | server.rs:989 `state.to_agent_tx.subscribe()` | Thread-scoped delivery is currently best-effort via client-side `thread_id` filtering; see Concern 5 |

**Key observations that drive v3.3 architecture:**

1. The server **already** distinguishes agent vs human connections via `participant_type`. Connector SDK already sends `participant_type: "agent"` (handshake.ts:36). The AgentRegistry is **live** and stores the card derived from `ainm.initialize.capabilities`. This is the basis for iOS live Agent Discovery (concern 5).
2. The server **does not** currently authenticate connectors beyond the signed `ainm.initialize` payload (`signature = sign(identity|timestamp)`). The `identity_store` lookup is only used for *message* verification (`handle_agent_connection` at server.rs:1030-1036), not for gating the handshake itself. This is the hole concern 4 closes.
3. Memory is **already** compound-memory-ready at the storage layer — the `memory_nodes` table is keyed on `user_addr`, not agent identity. What's missing is the *scoping policy* (which agent can read which user's memory) and *per-user consent*.
4. Fan-out from human to agents is currently a flat `broadcast::Sender<String>` (server.rs:210). Every agent sees every human message. Agents filter by `thread_id` client-side. This is fine for a handful of agents per server but must be tightened for multi-tenant production (and is relevant to concern 4's trust model).

---

## 2. Concern 1 — Python Connector SDK Integration (A5)

### Goal
`@ainm/py-connector` (working name) reaches parity with `@ainm/connector`: same handshake, same signing, same message envelopes, same memory/tool/stream callbacks. A Python connector can be swapped for a TypeScript connector with **zero server-side change**.

### Cross-language Compatibility Surface

**This is the critical table. Each row is a potential drift point.**

| Surface | TypeScript SDK (current) | Python SDK (required) | Drift Risk |
|---------|--------------------------|------------------------|-----------|
| WS framing | text frames, one JSON-RPC message per frame | text frames, one JSON-RPC message per frame (`websockets` library, `websocket-client`, or `aiohttp`) | LOW — both mature WS libs |
| JSON-RPC envelope keys | `{jsonrpc: "2.0", id, method, params}` | same | LOW |
| Handshake payload | `{protocolVersion, identity, signature, capabilities: {streaming, subThreads, contentTypes, participant_type: "agent", toolVisibility}}` | same | MEDIUM — `participant_type` is easy to forget; see Pitfall 1 |
| `ainm.initialize` signature | `Ed25519(UTF8(f"{identity}\|{timestamp_iso8601}"))`, base64url no-pad | same | HIGH — see "Signing Protocol Normative Pin" below |
| Envelope canonicalization | `canonicalize` npm package (pure JS, RFC 8785) | `rfc8785` PyPI package (trailofbits, pure Python) | MEDIUM — test vector suite mandatory |
| Envelope signing | `signEnvelope`: delete `signature` → canonicalize → `ed25519.signAsync(utf8(canonical), priv)` → base64url no-pad | same: pop `"signature"` → `rfc8785.dumps(env)` → `nacl.signing.SigningKey(priv).sign(canonical).signature` → base64.urlsafe_b64encode without `=` | HIGH — ordering mistakes silent |
| Key generation | `@noble/ed25519` `utils.randomPrivateKey()` (32 random bytes) | `nacl.signing.SigningKey.generate()` (32 random bytes) | LOW |
| Public key encoding | Raw 32-byte → base64url no-pad | same | LOW |
| Stream protocol | `ainm.stream.start` + `ainm.stream.delta` + `ainm.stream.end{collapsed_message}` | same | LOW |
| Tool protocol | `ainm.tool.invoke` in → `ainm.tool.result`/`ainm.tool.error` out; emit `started`/`progress`/`completed` | same | LOW |
| Memory RPC | `ainm.memory.briefing`, `ainm.memory.graph.query`, `ainm.memory.graph.contribute` | same | LOW |

### Recommended Python Stack

| Dependency | Version (pin floor) | Rationale |
|------------|---------------------|-----------|
| `PyNaCl` | 1.5+ | Ed25519 via libsodium. ~10-20× faster than pure-Python `ed25519`. Maintained by PyCA. Matches security posture of TS `@noble/ed25519` (audited). |
| `rfc8785` | 0.1+ | Pure-Python RFC 8785 JCS by Trail of Bits (security-focused shop). No deps. |
| `websockets` | 12+ | asyncio-native. Well-maintained. Cleaner than `websocket-client` for our async model. |
| `httpx` | 0.27+ | For HTTP calls during bridge scenarios (Agent Card fetch, etc.). |
| `pydantic` | 2+ | Envelope/content-block type models. Matches TS `types.ts` shapes. |
| `anyio` | 4+ | Backend-agnostic async primitives; lets the SDK work under asyncio or trio. |

Python version floor: **3.11+** (for `asyncio.TaskGroup`, improved error messages).

### Drift-Prevention Architecture (MANDATORY)

Two SDKs means two implementations of signing, canonicalization, and framing. Drift here causes silent signature verification failures. Mitigations, in priority order:

1. **NEW** `tests/cross-language/` suite at repo root. Known-good inputs (envelope JSON + private key) with expected canonical bytes and expected base64url signatures. Both SDKs run against the same vectors in CI. Authoritative vectors are produced by the **Rust server** (not by either SDK) via a new `cargo run -p ainm-demo --bin gen-vectors` utility.
2. **NEW** Shared schema package: `schemas/connector-wire/*.schema.json` — the handshake capabilities envelope, the stream event envelope, and the tool event envelope as formal JSON Schema 2020-12. Both SDKs validate outbound frames in development mode. This catches the "forgot `participant_type`" class of bug at runtime.
3. **NEW** `docs/SDK-PARITY.md` — a checklist of surfaces (the table above). Every PR to either SDK that changes wire shape must tick the parity checklist or be rejected.
4. **NEW** E2E harness `tests/connector-e2e/` — spins up the Rust server, one TS connector, one Python connector, and the iOS simulator. Sends a message from iOS, asserts both connectors receive-and-respond identically.

### Proposed Python Project Layout

```
packages/py-connector/
├── pyproject.toml              # build config (hatch or uv)
├── ainm_connector/
│   ├── __init__.py             # re-export AINMAgent, Transport, types
│   ├── agent.py                # AINMAgent class (mirrors agent.ts)
│   ├── identity/
│   │   ├── keypair.py          # generate_keypair, base64url_encode
│   │   └── signing.py          # sign_init, sign_envelope, verify_envelope
│   ├── protocol/
│   │   ├── rpc.py              # build_request, build_notification, parse_message
│   │   ├── handshake.py        # perform_handshake
│   │   ├── streaming.py        # StreamHelper
│   │   └── types.py            # Pydantic models for envelope/blocks
│   ├── memory/
│   │   └── context.py          # MemoryContext factory
│   └── transport/
│       ├── interface.py        # abstract Transport protocol
│       └── websocket.py        # websockets-backed transport
├── examples/
│   └── minimal_agent.py        # 5-line hello-world like the TS README
└── tests/
    ├── test_signing.py         # cross-language vector verification
    ├── test_handshake.py       # mocked transport
    └── test_e2e.py             # spins up Rust server, asserts round-trip
```

**Rationale:** mirror the TS SDK's folder structure exactly. This makes drift visually obvious in code review (a file in one tree and not the other is a red flag).

### Server-Side Changes

**Zero.** If the wire protocol is identical, the server cannot distinguish a Python connector from a TypeScript one. This is the point.

---

## 3. Concern 2 — MCP Bridge Connector Architecture (A6)

### Goal
Any MCP server (filesystem, GitHub, slack, custom) becomes an AINM thread participant. Users can message the MCP bridge and invoke MCP tools; tool calls render as first-class AINM tool visibility events on iOS.

### Architectural Unit: One Bridge Process per MCP Server

**Decision: ONE BRIDGE PROCESS PER MCP SERVER** (not one bridge multiplexing many MCP servers).

**Rationale:**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| One bridge, many MCP servers | Fewer processes, unified directory | Tool-name collisions across MCP servers, one crash kills all bridges, config sprawl, mixed identity model (whose agent card do we publish?) | Reject |
| One bridge per MCP server | Clear 1:1 mapping MCP server ↔ AINM identity, isolated failure domain, each MCP server has its own signing key, each publishes a distinct Agent Card | More processes (fine — they're cheap Node procs) | **Pick this** |

The AINM identity model is already email-like (`name@provider`). One MCP server maps cleanly to one identity, e.g. `fs-mcp@localhost`, `github-mcp@localhost`. Users `@mention` exactly the capability they want. A "meta-bridge" can come later if demand exists.

**Multi-MCP user experience:** a user runs several bridge processes (each wrapping one MCP server). The iOS Agent Discovery surface (concern 5) lists them all; user picks which to invite to a thread. This keeps the mental model consistent with Claude Code vs GPT bridges.

### Control Flow

```
                                                        ┌────────────┐
                                                        │ MCP Server │
                                                        │ (stdio OR  │
                                                        │  HTTP)     │
                                                        └─────┬──────┘
                                                              │ JSON-RPC
                                                              │ (MCP wire)
                                                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          MCP Bridge Process (Node)                       │
│                                                                          │
│  @modelcontextprotocol/sdk  ←→  McpClient (stdio or StreamableHTTP)      │
│            │                         │                                   │
│            │                         │  tools/list, tools/call,          │
│            │                         │  resources/list, prompts/list     │
│            │                         ▼                                   │
│            │               MCPCapabilityCache                            │
│            │               - tools: {name, description, inputSchema}[]   │
│            │               - resources: {uri, mimeType}[]                │
│            │                                                             │
│            ▼                                                             │
│    AINMAgent (from @ainm/connector)                                      │
│      ├── onMessage(handler):                                             │
│      │      parse user intent → select MCP tool → tools/call → stream    │
│      │      progress back as ainm.tool.* events                          │
│      └── Agent Card published with skills synthesized from MCP tools     │
│                                                                          │
└────────────────────────────────────────────────────────────────────────┬┘
                                                                          │
                                                              wss:// JSON-RPC
                                                              (AINM wire)
                                                                          ▼
                                                              ┌─────────────┐
                                                              │ AINM Server │
                                                              └─────────────┘
```

### Control Flow — Tool Invocation Path

```
[iOS user types "read README.md"]
        │
        ▼  ainm.message.send (signed envelope)
┌───────────────────────────────┐
│ AINM Server                    │
│ - verify signature             │
│ - broadcast via to_agent_tx    │
└───────────────┬───────────────┘
                │ ainm.message.deliver
                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ MCP Bridge Process                                                        │
│                                                                            │
│ 1. AINMAgent.onMessage receives message.text = "read README.md"            │
│                                                                            │
│ 2. Tool-selection strategy:                                                │
│    (a) LLM-assisted: ask a local/API model "which MCP tool + args"         │
│    (b) Explicit: user @mentions the tool, e.g. "@fs-mcp:read_file README"  │
│    (c) Natural-language heuristic: match user text to MCP tool descriptions│
│    → v3.3 ships (b) + (c). (a) deferred to v3.4.                           │
│                                                                            │
│ 3. ctx.emitToolStarted("read_file", {path: "README.md"})                   │
│    ──▶ server receives ainm.tool.started                                   │
│    ──▶ iOS renders "read_file" indicator (existing Phase 13 UI)            │
│                                                                            │
│ 4. mcpClient.callTool({name: "read_file", arguments: {path: ...}})         │
│    ──▶ stdin JSON-RPC to MCP server                                        │
│    ◀── stdout JSON-RPC response (may include error; may include resources) │
│                                                                            │
│ 5. ctx.emitToolCompleted("read_file", {result: fileContents})              │
│    ──▶ server receives ainm.tool.completed                                 │
│    ──▶ iOS renders completion                                              │
│                                                                            │
│ 6. ctx.stream().delta(fileContents) + ctx.stream().end()                   │
│    ──▶ iOS renders streamed response                                       │
└──────────────────────────────────────────────────────────────────────────┘
```

### Bridge Configuration

**NEW:** `packages/connector-mcp/config-schema.json` defines bridge configuration. CLI args and env vars map to this schema.

```
ainm-mcp-bridge \
  --server wss://localhost:8080/ws \
  --identity fs-mcp@localhost \
  --name "Filesystem (MCP)" \
  --mcp-transport stdio \
  --mcp-command "npx @modelcontextprotocol/server-filesystem /data" \
  --mcp-timeout-ms 30000
```

For Streamable HTTP MCP servers:

```
ainm-mcp-bridge \
  --server wss://localhost:8080/ws \
  --identity github-mcp@localhost \
  --name "GitHub (MCP)" \
  --mcp-transport http \
  --mcp-url https://mcp.example.com/sse \
  --mcp-auth-bearer $GITHUB_MCP_TOKEN
```

### MCP → AINM Agent Card Synthesis

On bridge start:

1. Bridge connects to MCP server, calls `tools/list`, `resources/list`, `prompts/list`.
2. Bridge builds AINM Agent Card capabilities:
   - Each MCP tool becomes an AINM **skill** with `name`, `description`, `input_schema` (MCP already ships JSON Schema).
   - `capabilities.toolVisibility = true`.
   - `contentTypes` defaults to `["text"]` plus any content types the MCP server's resources expose.
3. Bridge sends the synthesized card via `ainm.initialize.capabilities`. Server's `handle_agent_connection` stores it in `AgentRegistry.agent_cards`.

### Risks

| Risk | Mitigation |
|------|------------|
| MCP tool name collisions if bridge multiplexes servers (rejected above — single-server per bridge) | Architecture decision sidesteps this |
| MCP stdio process dies | Bridge supervises child; on exit, agent re-registers with "degraded" capability or disconnects entirely |
| MCP tool call blocks indefinitely | `--mcp-timeout-ms` (default 30s); timeout emits `ainm.tool.error` |
| MCP returns large resources (e.g. 50 MB file) | Bridge truncates to configurable limit (default 1 MB) and includes a warning; full content available via MCP resources URI if iOS adds resource fetching later |
| MCP `elicitation/create` (interactive clarification) — MCP 2025-06-18 feature | v3.3 scope: decline elicitation requests with a polite error. Full bridging to AINM ApprovalBlock is a v3.4 follow-up. |
| Tool-selection quality (no LLM in the loop) | Explicit `@tool-name: args` syntax + naive keyword match for v3.3; document that LLM-assisted selection is v3.4 |

---

## 4. Concern 3 — OpenAI Responses API Integration

### Current State

`@ainm/connector-openai` uses `chat/completions` in **stateless** mode: every AINM message becomes a one-turn request with a system prompt. No conversation history is sent beyond the current message. This is broken — the LLM cannot see what was said earlier in the AINM thread. Works-for-hello, fails-for-anything-real.

### Target: Responses API with Thread Mapping

The Responses API (GA since 2025) supports stateful conversations via `store: true` + `previous_response_id`. Caching gains (40-80%) and reasoning-model support (GPT-5) make this the default path for new work.

### Mapping AINM Threads ↔ OpenAI Responses

**Decision: 1:1 mapping AINM thread_id ↔ OpenAI response chain.**

The bridge maintains a local map `{ainm_thread_id: {last_response_id, store_enabled}}`. First user message in a thread → `POST /v1/responses` with no `previous_response_id`. Subsequent messages in the same thread → include the last `response.id`. On connector restart the map rebuilds by scanning AINM message history (bounded — see below).

| AINM event | Responses API action |
|------------|----------------------|
| User sends first message in thread T | `POST /v1/responses` with `store: true`, no `previous_response_id`. Save `response.id` keyed on T. |
| User sends Nth message in thread T | `POST /v1/responses` with `previous_response_id = last(T)`. Update last(T). |
| User creates sub-thread of T | **NEW decision:** sub-thread gets its own OpenAI response chain. AINM sub-thread starts fresh, `previous_response_id` null. (Alternative: inherit parent chain. Rejected — AINM sub-threads are semantically "fork from here for a tangent", not "continuation".) |
| Connector restart | On first message in thread T after restart, scan AINM message history for T's last OpenAI response hint (stored by bridge in `ainm.memory.graph.contribute` with tag `openai:response_id`). If absent, start fresh chain. |
| User archives/evicts a message | No-op in Responses API (OpenAI retains state for 30 days). Document this as a known gap. |

**Many-to-1 rejected.** A single OpenAI response chain covering multiple AINM threads blows the "thread as isolated context" invariant and defeats sub-threading.

### Server-Side Tool Execution (OpenAI-Hosted Tools)

The Responses API offers built-in tools: web search, code interpreter, file search, computer use. Tool execution runs on OpenAI servers; we see the tool call + result in the response stream, not control over invocation.

**This is a surprise for AINM tool visibility** — AINM's contract is "every tool call in a thread is visible to every participant." With OpenAI hosting the tool, the connector cannot naturally emit `started` before the tool runs (we only learn about it post-hoc in the streamed response) nor intervene for approval.

**Proposed handling:**

1. Responses API streams back typed events: `response.output_item.added`, `response.function_call_arguments.delta`, `response.file_search.started`, `response.file_search.completed`, etc.
2. Connector maps these to AINM tool events:
   - `response.web_search.started` → `ainm.tool.started` with `tool: "openai:web_search"`
   - `response.web_search.completed` → `ainm.tool.completed` with result summary
   - `response.output_text.delta` → `ainm.stream.delta`
3. Prefix all OpenAI-hosted tool names with `openai:` so iOS can disambiguate from connector-hosted tools. This keeps the invariant "all tool calls visible to the thread" honest — users see `openai:web_search` run.
4. **Document the limitation:** AINM's ApprovalBlock gate cannot intercept OpenAI-hosted tools (they execute on OpenAI infra). For user-approval-required flows, the user must use a connector that hosts tools locally (Claude Code, MCP bridge).

### Migration Strategy

1. Keep `chat/completions` code path under `--api-mode chat` (default for Ollama, Together, Groq, and OpenAI-compatible servers that don't implement Responses).
2. Add `--api-mode responses` (default when `--base-url` is `api.openai.com`). CLI auto-detection: if base URL matches `api.openai.com`, default to `responses`.
3. For `responses` mode, use official `openai` npm SDK (has first-class Responses API support) rather than raw `fetch`.

### Compound Memory Interaction

`ctx.memory.getBriefing()` currently returns a blob prepended to the system prompt. With stateful Responses API this is wasteful — the briefing is re-sent every turn.

**NEW behavior:** briefing sent only on the **first** message of a thread (when `previous_response_id` is null). Subsequent turns rely on OpenAI's stored context. When briefing content changes (new compound memory facts committed), the bridge forces a fresh chain on next message and re-injects the updated briefing.

---

## 5. Concern 4 — Connector Auth/Identity Architecture

### Current Trust Model (the hole)

```
connector: signInit("claude-code@localhost", "2026-04-18T12:00:00Z")
           → base64url(Ed25519 sig)
           → ainm.initialize {identity, signature, capabilities}

server:    handle_ws_connection → reads init_req.identity ("claude-code@localhost")
           → sends init response immediately
           → **does NOT verify the signature against any known public key**
           → only when agent later sends ainm.message.send does signing.rs:verify_signature run
             against identity_store.get_public_key_bytes(from) — and that store is
             seeded only with bootstrap identities (see identity.rs).
```

**Impact:** any process that can reach the WS port can claim any identity. Signed envelopes afterward only work if the server happens to have the public key for that identity on file — which is a soft fence, not a hard auth gate. For v3.3 production paths (MCP bridges, third-party connectors, OpenAI bridge with API keys), we need a real trust anchor.

### Proposed Model: Server-Managed Connector Allowlist with First-Run TOFU

Trust anchor lives in a **NEW** `connector_keys` table keyed by `(user_addr, agent_identity) → public_key_b64`. The user (via iOS app or out-of-band CLI) enrolls a connector's public key before first use.

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Enrollment Flow (first run)                  │
│                                                                      │
│ Connector process:                                                   │
│   1. Generate Ed25519 keypair (if no key in $XDG_CONFIG_HOME)        │
│   2. Derive pubkey, print:                                           │
│        "Connector public key: <base64url>"                           │
│        "Enroll via: ainm-cli enroll --agent claude-code@localhost \  │
│                       --user santiago@localhost \                    │
│                       --pubkey <base64url>"                          │
│                                                                      │
│ User (on iOS or via CLI):                                            │
│   3. Invokes enrollment RPC (signed by user's human key):            │
│      ainm.connector.enroll {agent_identity, pubkey, allow_scopes}    │
│                                                                      │
│ Server:                                                              │
│   4. Verifies the request is signed by a known human identity        │
│   5. Inserts into connector_keys table                               │
│   6. Acks                                                            │
│                                                                      │
│ Connector (next restart):                                            │
│   7. ainm.initialize now has a known-good key on file                │
│   8. Handshake signature is verified server-side before accepting    │
└─────────────────────────────────────────────────────────────────────┘
```

### MODIFIED: `handle_ws_connection`

```
Before (today):
  read init_req → send init response → wait for initialized → route

After (v3.3):
  read init_req → look up expected pubkey for init_req.identity
                  ├── found:
                  │     verify init signature with that pubkey
                  │     ├── valid → send init response
                  │     └── invalid → close(-32022 "Invalid signature")
                  └── not found:
                        ├── if server is in "TOFU" mode (dev default):
                        │     accept, persist pubkey as trust-on-first-use,
                        │     warn in logs
                        └── if server is in "strict" mode (production default):
                              close(-32023 "Unknown agent identity")
  → wait for initialized → route
```

### Relationship Between Connector Identity Key and Agent Signing Key

**Decision: they are the SAME key.**

Why: the connector's identity (the thing the handshake proves) and the message-signing identity (the thing each envelope proves) should be the same principal. Splitting them adds a certificate layer we don't need, and the user's mental model is "this connector speaks as this agent" — one key per agent identity.

Corollary: the `connector_keys` table **is** the agent's pubkey source of truth. `identity_store.get_public_key_bytes` in message verification should consult `connector_keys` for agent identities (today it reads from a bootstrap file).

### Auth for Connector → Upstream Runtime (API keys, OAuth)

This is orthogonal to AINM auth, but v3.3 needs to document it:

| Runtime | Auth | Where it lives | Documented in v3.3 |
|---------|------|----------------|---------------------|
| Claude Code CLI | Anthropic console OAuth (via `claude login`) OR `ANTHROPIC_API_KEY` | `~/.claude/` (OAuth) or env var | yes — both paths supported today; doc clarifies tradeoffs |
| OpenAI | `OPENAI_API_KEY` env var (existing) | connector process env | yes |
| OpenAI-compatible (Ollama, Together, Groq) | `--api-key` or env (may be empty for local) | connector process env | yes |
| MCP server (HTTP transport) | `--mcp-auth-bearer`, `--mcp-oauth` (deferred to v3.4) | connector process env | partial — bearer only for v3.3 |
| MCP server (stdio) | environment of the child process | passed through via `--mcp-env` | yes |

**Key architectural point:** AINM never sees upstream credentials. They stay in the connector process. This is the correct trust boundary — the AINM server is a transport, not a credential broker.

### Error Codes (additions to transport.md)

| Code | Meaning |
|------|---------|
| -32022 | Invalid handshake signature |
| -32023 | Unknown agent identity (strict mode, no `connector_keys` row) |
| -32024 | Pubkey mismatch (enrolled key differs from handshake claim) |

---

## 6. Concern 5 — iOS Live Agent Discovery

### Current Behavior

iOS `AgentCardService` (from Phase 13) fetches `GET /.well-known/ainm/{name}/agent-card.json` over HTTP with ETag caching. This is pull-only. It returns whatever the server has configured, independent of whether the agent is actually connected.

### Target: Live Discovery Reflecting Connected Agents

Iosapp should surface which agents are **currently connected** (from `AgentRegistry`), not just which have a card on file. Push updates when agents connect/disconnect.

### Option Analysis

| Option | Fit with existing model | Verdict |
|--------|-------------------------|---------|
| (a) Poll `/.well-known/ainm/directory` every N seconds | Works but spammy, high battery cost on iOS, stale by up to N seconds | Reject for primary path; useful as fallback |
| (b) JSON-RPC notification `ainm.agents.updated` pushed via HumanRegistry | Uses existing WS channel; fits HumanRegistry.send_to_all; low code impact | **Pick this** |
| (c) Dedicated pub/sub channel (Redis, NATS) | Overkill for single-server deployment; doesn't interop with our "single Axum process" architecture | Reject |

### Spec Change Required

**NEW method `ainm.directory.list`** (request) — returns currently connected agents' cards:

```
→ {method: "ainm.directory.list", params: {}}
← {result: {agents: [{identity, name, capabilities, connected_at, skills}]}}
```

**NEW notification `ainm.directory.updated`** (server → human clients) — emitted on agent connect/disconnect:

```
← {method: "ainm.directory.updated",
   params: {event: "connected"|"disconnected", agent: {identity, name, ...}}}
```

**NEW HTTP endpoint `GET /.well-known/ainm/directory`** — JSON list of Agent Cards. Used for:
- Unauthenticated discovery (federation, docs, UI previews)
- Cold-start when iOS connects (fetch once, then listen for updates)

### Server Integration Points

1. **MODIFIED** `AgentRegistry::register`: after inserting, broadcast `ainm.directory.updated{event: "connected"}` to all human connections via `HumanRegistry.send_to_all`.
2. **MODIFIED** `AgentRegistry::cleanup`: when last connection for an identity drops, broadcast `event: "disconnected"`.
3. **NEW** `AgentRegistry::list_cards() -> Vec<AgentCard>`: returns all currently connected agents' cards (public method).
4. **NEW** handler for `ainm.directory.list` JSON-RPC method in `handle_human_connection`.
5. **NEW** HTTP route `GET /.well-known/ainm/directory` returning `list_cards()` as JSON array.

### iOS Integration Points

1. **MODIFIED** `AgentDiscoveryView`: on appear, call `AINMClient.request("ainm.directory.list")` instead of hitting HTTP. HTTP `/.well-known/ainm/directory` is kept as fallback for cold-start and for web/federation uses.
2. **MODIFIED** `AINMClient`: add `onDirectoryUpdated` callback. When `ainm.directory.updated` arrives, invoke the callback on MainActor; AgentCard repository upserts and UI reacts via GRDB observation.
3. **MODIFIED** `AgentCardRepository`: gains a `connected_at` column; UI filters "show only connected" toggle.

### Backward Compatibility

The existing HTTP Agent Card endpoint stays (federation needs it; some tests depend on it). The new directory endpoint is additive. iOS clients on the old build continue to work; they just don't get live updates.

---

## 7. Concern 6 — Compound Memory Groundwork

### Current State

Memory graph is **already user-scoped** (confirmed by reading `demo/src/graph.rs`: every INSERT/SELECT filters on `user_addr`). The existing RPCs:

- `ainm.memory.briefing` — builds a blob from the user's graph
- `ainm.memory.graph.query` — searches user's nodes + edges
- `ainm.memory.graph.contribute` — adds a node/edge
- `ainm.memory.store`/`ainm.memory.query`/`ainm.memory.evict` — archival facts (per-user)

**Gap:** the scoping policy is implicit ("the agent is trusted because it connected"). For v3.3 we need the *consent* layer, even if we don't ship full cross-agent aggregation (that's v3.4).

### v3.3 Scope (groundwork)

The v3.3 deliverable is **positioning for v3.4**, not full compound memory. Concretely:

1. **MODIFIED** memory briefing: already returns a user-scoped blob. Document the contract: `ctx.memory.getBriefing()` returns **the user's compound memory**, not the agent's private memory. Claude Code connector sees the same briefing the OpenAI connector sees, if both are talking to the same user.
2. **NEW** consent column on `connector_keys` table: `memory_scopes TEXT[]` — `["read", "write"]`. Default `["read", "write"]` for TOFU enrollment; iOS settings UI can revoke.
3. **MODIFIED** server-side memory RPC gating: before executing `ainm.memory.graph.query` or `contribute` for an agent, check `connector_keys.memory_scopes`. Return `-32025 "Memory access denied"` if scope is missing.
4. **NEW** `ainm.memory.consent` RPC for the user's iOS app to grant/revoke per-agent scopes. Grants/revokes signed by the user's key (same identity pattern as connector enrollment).
5. **DEFERRED to v3.4**: cross-agent memory aggregation, export, visualizations, compaction across agents.

### Data Model Additions

```
-- NEW table (v3.3)
CREATE TABLE connector_keys (
  user_addr TEXT NOT NULL,
  agent_identity TEXT NOT NULL,
  public_key BYTEA NOT NULL,
  memory_scopes TEXT[] NOT NULL DEFAULT ARRAY['read','write'],
  enrolled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen_at TIMESTAMPTZ,
  PRIMARY KEY (user_addr, agent_identity)
);

-- MODIFIED: memory_nodes/edges already have user_addr column; no schema change
```

### Architectural Invariant to Preserve

**Memory is user-owned.** A connector writes *on behalf of the user* (with explicit user consent). A connector reading memory is reading *the user's data*, not the agent's data. This framing matters: it's what makes cross-agent compound memory work.

---

## 8. Concern 7 — v3.2 Tech Debt Integration

### Scope

11 iOS HUMAN-UAT items (Phases 10 + 11) + 2 XCUITest flakes (Phase 14):
- Phase 10: live handshake, cross-platform crypto round-trip, bidirectional signing, Keychain persistence, CI simulator
- Phase 11: offline read access, real-time list updates, agent identity badges, multi-block rendering
- Phase 14: deep-link missing-message toast, sub-thread reply-count badge

### Integration Decision: Dedicated Phase 15 (Standalone)

**Do this first, as its own phase, before connector work.**

Rationale:
- All 13 items are iOS-side. Connector work is Rust + TypeScript + Python — different code, different reviewers, different risk domain.
- Phase 10/11 items are largely cross-platform crypto verification. Those tests **catch exactly the signing drift risks** that Python SDK introduces. Fixing them first gives us trustworthy iOS ↔ Rust ↔ TS cross-verification; when Python is added, the same test scaffolding extends to Python.
- XCUITest flakes are timing races. Fixing them now avoids poisoning v3.3's UAT runs (concern 5 adds new XCUITests for live directory).
- Small phase (1-2 weeks), entirely iOS, keeps the milestone's opening cadence brisk and confidence high.

**Reject interleaving.** Spreading iOS cleanups across phases means every new iOS UI phase has to re-derisk the same crypto plumbing.

---

## 9. Build Order

### Dependency Graph

```
                    ┌────────────────────────────────────────┐
                    │ Phase 15: v3.2 iOS tech debt (13 items)│
                    │ — independent, unblocks cross-lang     │
                    │   crypto confidence                    │
                    └────────────────┬───────────────────────┘
                                     │
                                     ▼
           ┌─────────────────────────────────────────────────┐
           │ Phase 16: Connector auth + A1–A4 ratification   │
           │ — connector_keys table, signature-verified      │
           │   handshake, TOFU mode, enrollment RPC,         │
           │   polish existing TS SDK + ClaudeCode + OpenAI  │
           │   BLOCKING for Phase 17 (Python wants the       │
           │   hardened auth contract to match)              │
           └────────────────┬──────────────────┬─────────────┘
                            │                  │
              ┌─────────────▼───────────┐  ┌───▼───────────────────┐
              │ Phase 17: Python SDK    │  │ Phase 18: OpenAI       │
              │ — parity with TS SDK,   │  │   Responses API        │
              │   cross-lang test vecs  │  │ — stateful thread map, │
              │   and E2E harness       │  │   OpenAI-hosted tools  │
              │                         │  │                        │
              │ Can parallelize with 18 │  │ Can parallelize with 17│
              └─────────────┬───────────┘  └───┬───────────────────┘
                            │                  │
                            ▼                  ▼
           ┌─────────────────────────────────────────────────┐
           │ Phase 19: MCP Bridge connector                  │
           │ — depends on connector auth (Phase 16) for      │
           │   TOFU enrollment, does not depend on Python    │
           │   (bridge is TS). Ships one-bridge-per-MCP      │
           │   server model.                                 │
           └────────────────┬────────────────────────────────┘
                            │
                            ▼
           ┌─────────────────────────────────────────────────┐
           │ Phase 20: iOS live Agent Discovery + compound   │
           │   memory groundwork (consent + scoping)         │
           │ — depends on Phase 16 (connector_keys table     │
           │   powers memory_scopes). UI touches AgentCard   │
           │   repo and new memory consent sheet.            │
           └─────────────────────────────────────────────────┘
```

### Proposed Phase Ordering

| # | Phase | Est. size | Depends on | Can parallelize with |
|---|-------|-----------|------------|-----------------------|
| 15 | iOS tech debt cleanup (13 items) | S | — | — |
| 16 | Connector auth + A1–A4 ratification | M | 15 | — |
| 17 | Python Connector SDK (A5) | M | 16 | 18 |
| 18 | OpenAI Responses API + connector hardening | S | 16 | 17 |
| 19 | MCP Bridge connector (A6) | M | 16 | — (best sequenced after 17/18 so parity + auth stories are stable) |
| 20 | iOS live Agent Discovery + compound memory groundwork | M | 16 | — |

### Rationale for Ordering

- **15 first** — closes confidence gaps in the shipped v3.2 surface before we add more surface. It's also the smallest phase and gives the milestone a quick confidence win.
- **16 before everything else** — connector auth is the trust anchor. Shipping Python SDK or MCP Bridge before the auth gate gives us two places to fix the same class of vulnerability later. Also hardens the TS SDK so Python has a stable target to mirror.
- **17 || 18** — Python SDK and OpenAI Responses are independent. Different engineers can own them in parallel. They share only the auth contract from 16.
- **19 after 17/18** — the MCP bridge is a *consumer* of the SDK. It benefits from stabilization work done in 17/18 (better error handling, clearer stream semantics). Also lets us dogfood the bridge with Python tools too (MCP + Python SDK pairing).
- **20 last** — live discovery and memory consent both touch the iOS UI deeply. Doing this after 16-19 means the AgentRegistry is already producing richer directory data (more connectors), so the UI has real content to render. Memory consent is also best validated with multiple concurrent connectors on the same user.

---

## 10. Integration Summary Table

| New feature | Server changes | Connector SDK changes | iOS changes | Spec changes |
|-------------|----------------|-----------------------|-------------|--------------|
| Python SDK (A5) | none | NEW `packages/py-connector/` tree | none | NEW `docs/SDK-PARITY.md`, cross-lang test vectors |
| MCP Bridge (A6) | none | NEW `packages/connector-mcp/` using `@ainm/connector` + `@modelcontextprotocol/sdk` | none | none (MCP is external); add bridge config schema |
| OpenAI Responses | none | MODIFIED `packages/connector-openai/` (Responses client, thread map) | none | document `openai:*` tool-name prefix convention |
| Connector auth | NEW `connector_keys` table, MODIFIED handshake, NEW enrollment RPC | MODIFIED SDK to read/write key from `$XDG_CONFIG_HOME/ainm/{identity}.key` and emit enrollment banner on first run | NEW enrollment UX (QR or paste pubkey), signed enrollment RPC call | NEW error codes -32022/-32023/-32024/-32025; NEW RPC `ainm.connector.enroll` |
| iOS live directory | NEW directory RPCs, MODIFIED AgentRegistry (broadcast on conn/disc), NEW HTTP /directory | none | MODIFIED AgentDiscoveryView to use RPC + subscribe to updates | NEW `ainm.directory.list` + `ainm.directory.updated` |
| Compound memory groundwork | MODIFIED memory RPCs to gate on `memory_scopes` | MODIFIED SDK memory context to handle `-32025` errors | NEW memory consent sheet (per-agent scopes) | NEW `ainm.memory.consent` RPC; document user-scoped-memory contract |
| v3.2 tech debt | none | none | FIXED Phase 10/11/14 items | none |

---

## 11. Anti-Patterns to Avoid

### Anti-Pattern 1: Re-implementing signing in each SDK from scratch

**What people do:** clone the TS SDK's signing logic into Python by eye.
**Why it's wrong:** RFC 8785 has subtle rules (number serialization follows ECMAScript 6, escaping follows JSON spec but strings are re-serialized, null handling, etc.). Byte-level drift is silent — signatures just fail to verify.
**Do this instead:** (a) both SDKs use mature RFC 8785 libraries (`canonicalize` / `rfc8785`). (b) Cross-language test vectors generated by the Rust server are canonical; both SDKs verify against them in CI. (c) Any time the signing logic is touched in one SDK, the parity checklist forces a PR to the other.

### Anti-Pattern 2: One connector multiplexing all MCP servers

**What people do:** write a single "meta-bridge" that holds connections to many MCP servers and exposes them all as one AINM agent.
**Why it's wrong:** (a) tool name collisions ("read_file" exists in half the MCP ecosystem), (b) a single AINM identity has to represent many capability sets — confusing agent card, (c) failure of one MCP server degrades all, (d) isolation between MCP servers is lost.
**Do this instead:** one bridge process per MCP server. One AINM identity per MCP server. Users `@mention` or invite the specific capability. Cheap: Node processes are free; memory cost is < 50 MB each.

### Anti-Pattern 3: Treating connector keys and message-signing keys as separate

**What people do:** issue a "connector auth token" separate from the agent's Ed25519 identity key, creating a certificate layer.
**Why it's wrong:** the user's mental model is "this connector speaks as this agent." Two keys means two rotation flows, two enrollment steps, two breakage modes. Also the AINM protocol already signs every message with the agent's key — adding a different key for handshake means the handshake and the messages prove different things.
**Do this instead:** one Ed25519 keypair per agent identity. Used for handshake signature *and* message envelope signatures. `connector_keys` is the pubkey truth.

### Anti-Pattern 4: iOS polling for agent list

**What people do:** iOS polls `/.well-known/ainm/directory` every 10 seconds.
**Why it's wrong:** battery drain, stale UI, still doesn't handle "agent disconnected 2s ago", doesn't integrate with the WS channel that's *already open*.
**Do this instead:** `ainm.directory.list` for cold-start, `ainm.directory.updated` notification for live updates, piggyback on the existing HumanRegistry broadcast channel.

### Anti-Pattern 5: Stateful OpenAI bridge with no thread mapping

**What people do:** store a single `previous_response_id` globally in the connector.
**Why it's wrong:** multiple AINM threads collapse into one OpenAI response chain. Sub-threads break. Context bleeds across conversations.
**Do this instead:** map `thread_id → last_response_id`. Each AINM thread is its own OpenAI chain. Sub-threads start fresh (treat AINM's "fork from here" semantics literally).

### Anti-Pattern 6: Server-side allowlist that blocks dev-loop

**What people do:** require pre-enrolled pubkeys for every connector, always.
**Why it's wrong:** a developer spinning up a new connector has to run an enrollment dance before the first hello-world. Friction kills adoption.
**Do this instead:** two modes — `--auth-mode tofu` (default in dev, warn on first-seen, persist) vs `--auth-mode strict` (default in prod, reject unknown). Document how to flip per deployment.

---

## 12. Open Questions (flag for later phases)

1. **MCP elicitation handling** — the 2025-06-18 MCP spec added `elicitation/create` for interactive clarification. v3.3 declines these. When do we bridge to AINM ApprovalBlock? Probably v3.4 when we have clearer semantics for agent ↔ agent (connector as intermediary).
2. **OpenAI-hosted tool approval gates** — AINM ApprovalBlock cannot gate OpenAI-side tools. Do we document this as a known limitation or do we route through a proxy that can veto the tool before sending to OpenAI? Defer to v3.4.
3. **Multi-user server tenancy** — today `to_agent_tx` is a single broadcast. Multi-user production probably wants per-user agent subscriptions. Not v3.3 scope, but concern 4's `connector_keys` table has `user_addr` as part of the PK, so we're not painting ourselves into a corner.
4. **Federation × connector auth** — how does a connector enrolled on server A authenticate against server B in a federated context? Probably via the existing federation trust model (mutual TLS between servers) — the user-agent trust stays local. Needs explicit doc in v3.3 federation notes.
5. **Compound memory export format** — v3.4 ships aggregation. The wire shape should be decided in v3.3 so v3.4 doesn't have to migrate data.
6. **Rate limiting on memory RPCs** — a misbehaving connector could flood `ainm.memory.graph.contribute`. Not a v3.3 blocker but worth a token-bucket in Phase 16.

---

## Sources

- **Existing code (HIGH confidence):**
  - `demo/src/server.rs` (HumanRegistry, AgentRegistry, handshake)
  - `demo/src/graph.rs` (user-scoped memory)
  - `demo/src/signing.rs` (Rust verify path)
  - `packages/connector/src/` (TS SDK — current reference)
  - `packages/connector-claude-code/src/claude-code-agent.ts`
  - `packages/connector-openai/src/openai-agent.ts`
- **AINM spec docs:** `docs/transport.md`, `docs/agent-card.md`, `docs/compound-memory.md`, `docs/security.md`, `docs/tool-visibility.md`
- **External (MEDIUM confidence, verified 2026-04):**
  - [MCP 2026 Roadmap — Model Context Protocol Blog](https://blog.modelcontextprotocol.io/posts/2026-mcp-roadmap/)
  - [MCP Transport Future — Model Context Protocol Blog (2025-12-19)](https://blog.modelcontextprotocol.io/posts/2025-12-19-mcp-transport-future/)
  - [MCP Bridge paper (arXiv 2504.08999)](https://arxiv.org/html/2504.08999v2)
  - [OpenAI: Migrate to the Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses)
  - [OpenAI: Responses vs Chat Completions](https://platform.openai.com/docs/guides/responses-vs-chat-completions)
  - [OpenAI: Conversation state](https://developers.openai.com/api/docs/guides/conversation-state)
  - [rfc8785 (PyPI) — trailofbits](https://pypi.org/project/rfc8785/)
  - [PyNaCl (PyPI)](https://pypi.org/project/PyNaCl/)
  - [RFC 8785: JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)

---

*Architecture research for: v3.3 Universal Agent Client integration*
*Researched: 2026-04-18*
