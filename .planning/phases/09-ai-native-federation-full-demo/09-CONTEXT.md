# Phase 9: AI-Native Federation & Full Demo - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning

<domain>
## Phase Boundary

AINM's unique AI-native features — agent discovery, tool call transparency, memory propagation, and streaming — work seamlessly across federation boundaries. A comprehensive demo proves all four features end-to-end across two federated servers.

Requirements: AINF-01, AINF-02, AINF-03, AINF-04, DEMO-05

</domain>

<decisions>
## Implementation Decisions

### Cross-Federation Agent Card Resolution (AINF-01)
- **D-01:** Agent cards resolved via public HTTPS GET to the agent's @provider domain at `/.well-known/ainm/{agent-name}/agent-card.json`. No mTLS required — agent cards are public discovery info, consistent with A2A pattern.
- **D-02:** Per-agent URL path (not a directory listing). Discovery starts by resolving the @provider domain, then fetching the named agent's card by name.
- **D-03:** Resolved agent cards cached in-memory with TTL (e.g., 5 min) in the existing `RemoteServerCache` in `identity.rs`. Consistent with existing server identity caching pattern.

### Event Relay Strategy (AINF-02, AINF-04)
- **D-04:** All event types — StreamStart, StreamDelta, StreamEnd, ToolCallStarted, ToolCallProgress, ToolCallCompleted — relayed via the existing `ainm.relay` method. Each event wrapped individually in a relay payload. Same path, same validation, same queue as regular messages.
- **D-05:** Tool call events and streaming events use the same relay mechanism. The relay doesn't need to know what kind of event it's carrying — uniform handling.
- **D-06:** On the receiving server, relayed streaming/tool events delivered through the same `broadcast::channel` as regular relayed messages. REPL/client filters by event type for rendering. No new plumbing.

### Memory Context on Federated Join (AINF-03)
- **D-07:** Origin server pushes active context frames to the remote server immediately when a remote agent joins a federated thread. Agent is ready from first message — no cold start.
- **D-08:** Only active context frames are propagated (not conversation history or archival). Lightweight, immediately useful. Conversation history available via existing `ainm.backfill` if needed.
- **D-09:** New `ainm.context-push` JSON-RPC method on the existing relay endpoint. Payload contains `thread_id` + array of active context frames. Sent once on join, through existing mTLS relay infrastructure.

### Demo Scenario (DEMO-05)
- **D-10:** Single comprehensive demo scenario exercising all four AI-native features in sequence: (1) resolve remote agent card via /discover, (2) start cross-federation thread, (3) agent streams response across federation, (4) agent does tool call with visible events, (5) memory context delivered on join.
- **D-11:** Reuses existing slash commands (/stream, /weather, /memory) that work in single-server mode — now they cross federation. Consistent with Phase 7/8 demo style.
- **D-12:** New `/discover helper@server-b.local` REPL command that resolves the remote agent's card and displays its capabilities. Explicit demonstration of AINF-01.

### Claude's Discretion
- Internal relay payload format for streaming/tool events (as long as they use ainm.relay)
- TTL duration for agent card cache (5 min suggested but flexible)
- Active context frame selection logic on origin server (which frames to push)
- Demo script structure and output formatting
- Order of relay_handler match arms for ainm.context-push

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `spec/transport.md` — Defines relay endpoint format (Section 5.1-5.4), `.well-known/ainm/server` response format, relay request/response JSON-RPC, 3-layer validation rules. Extend for `ainm.context-push` method.
- `spec/streaming.md` — Streaming lifecycle (StreamStart/Delta/End) and event format
- `spec/interactive.md` — Tool call visibility events (ToolCallStarted/Progress/Completed)
- `spec/memory.md` — Three-tier memory model (active context frames, conversation, archival)
- `spec/agent-card.md` — Agent Card format and discovery mechanism
- `spec/identity.md` — Identity resolution model, `name@provider` format, public key distribution
- `spec/security.md` — Ed25519 signing, JCS canonicalization, signature verification

### JSON Schemas
- `schemas/transport.schema.json` — Relay payload schemas (extend for context-push)
- `schemas/streaming.schema.json` — Streaming event schemas
- `schemas/interactive.schema.json` — Tool call event schemas
- `schemas/memory.schema.json` — Memory operation schemas
- `schemas/agent-card.schema.json` — Agent Card schema

### AsyncAPI
- `asyncapi.yaml` — Transport binding definitions (extend for context-push operation)

### Existing Implementation (key files to modify)
- `demo/src/federation.rs` — Inbound relay handler with match dispatch for ainm.relay and ainm.backfill. Add ainm.context-push handler and streaming/tool event forwarding to broadcast channel.
- `demo/src/relay.rs` — Outbound relay sender. Needs to relay streaming/tool events from local agents to remote servers.
- `demo/src/streaming.rs` — StreamStart/Delta/End generation. Currently local-only — needs to trigger relay for federated threads.
- `demo/src/agent.rs` — Tool call events and memory demo. Events need relay for federated threads.
- `demo/src/identity.rs` — RemoteServerCache. Extend for agent card caching with TTL.
- `demo/src/server.rs` — Axum router. Add per-agent .well-known endpoint for agent card resolution.
- `demo/src/repl.rs` — Interactive REPL. Add /discover command for remote agent card resolution.
- `demo/src/render.rs` — Message rendering. May need updates for rendering relayed streaming/tool events.

### Project Context
- `.planning/REQUIREMENTS.md` — Full requirement definitions (AINF-01 through DEMO-05 for Phase 9)
- `.planning/ROADMAP.md` — Phase success criteria and dependency chain
- `.planning/phases/07-federation-relay-core/07-CONTEXT.md` — Phase 7 decisions (demo model, thread routing, TLS setup)
- `.planning/phases/08-security-hardening-reliability-backfill/08-CONTEXT.md` — Phase 8 decisions (replay protection, backfill, persistent queue)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `streaming.rs`: Full StreamStart/Delta/End lifecycle — extend to trigger relay for federated threads
- `agent.rs`: ToolCallStarted/Progress/Completed events — extend to trigger relay for federated threads
- `agent.rs::handle_memory()`: Memory store/query/evict — reuse for demonstrating context frame creation
- `server.rs`: Agent card endpoint at `.well-known/ainm/helper/agent-card.json` — make path dynamic for per-agent resolution
- `federation.rs::relay_handler()`: Match dispatch for ainm.relay and ainm.backfill — add ainm.context-push arm
- `relay.rs::RelaySender`: Outbound relay with persistent queue — reuse as-is for streaming/tool events
- `identity.rs::RemoteServerCache`: Remote server identity caching — extend for agent card caching with TTL
- `repl.rs`: Interactive REPL with slash commands — add /discover command

### Established Patterns
- Match dispatch in relay_handler for method routing (ainm.relay → ainm.backfill → ainm.context-push)
- broadcast::channel for inter-component message passing (relayed messages → WebSocket clients)
- JSON-RPC 2.0 framing for all protocol operations
- Ed25519 signing via ring with JCS canonicalization
- Slash commands in REPL (/stream, /weather, /memory) for demo features
- Docker Compose two-server setup with pre-generated certificates

### Integration Points
- `relay_handler`: Add ainm.context-push dispatch and streaming/tool event forwarding
- `RelaySender`: Called from streaming.rs and agent.rs when thread is federated
- `ServerState`: May need to expose RelaySender handle to streaming/agent modules
- `ThreadRegistry`: Check if thread is federated to decide whether to relay events
- Docker Compose: Comprehensive demo script exercising all four AI-native features

</code_context>

<specifics>
## Specific Ideas

- The /discover command should display the agent card in a human-readable format: name, description, capabilities, supported content types
- Streaming across federation should feel indistinguishable from local streaming to the human user — same REPL rendering, just the source is remote
- The demo scenario tells a story: discover what the agent can do → talk to it → see it stream → see it use tools → see memory propagate — showing AINM's unique value proposition as a federation protocol
- Thread ID format {domain}:{uuidv7} already tells any server whether a thread is federated (domain doesn't match local) — use this for relay decisions

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 09-ai-native-federation-full-demo*
*Context gathered: 2026-03-29*
