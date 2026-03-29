# Phase 06: Demo Feature Gaps & Code Cleanup - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Close remaining demo tech debt: implement memory evict operation, add Agent Card HTTP caching, eliminate all cargo warnings, and remove dead code paths. All items are concrete fixes from the v3.1 milestone audit — no new features.

</domain>

<decisions>
## Implementation Decisions

### Memory Evict
- **D-01:** Add `/evict` REPL command that evicts a previously stored archival fact by key. Follow the same pattern as `/memory` — user types `/evict`, demo sends `ainm.memory.evict` JSON-RPC request through the server, server responds with confirmation. Evict the first fact stored in the `/memory` scenario (reuse existing key).
- **D-02:** Memory evict response should print the evicted key and a confirmation message, matching the existing `/memory` output style.

### Agent Card Caching
- **D-03:** Use SHA-256 content hash of the serialized Agent Card JSON as the ETag value. No server-side state or version counter needed — deterministic from content.
- **D-04:** Server handler checks `If-None-Match` request header against computed ETag. Return 304 Not Modified if match, 200 with full body + ETag header if no match.

### Dead Code Removal
- **D-05:** Remove `ainm.tool.call.started`, `ainm.tool.call.progress`, `ainm.tool.call.completed` match arms from the server.rs forwarding dispatch (lines ~412-414). Tool events reach the human via `ainm.message.send` wrapper — the raw method dispatch is dead code.
- **D-06:** Evaluate whether repl.rs tool.call.* rendering arms (lines ~192-205) are actually reached. If tool events arrive as `ainm.message.send` envelopes with type field, these match arms for raw method names may also be dead. Remove if unreachable.

### Cargo Warnings
- **D-07:** Remove unused `resolve` method from identity module, or wire it into demo if trivial. Prefer removal (demo doesn't need runtime address resolution).
- **D-08:** Remove unused `MessageEnvelope` struct and its `to_value`/`from_value` associated methods from protocol.rs, OR wire them into the demo message construction. Prefer the approach that produces fewer changes — if the struct serves no demo purpose, remove it.

### Claude's Discretion
- Exact error message text for evict failures
- Whether to add a brief REPL help update listing `/evict`
- Order of operations for dead code removal vs warning fixes (can be combined logically)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specs
- `spec/memory.md` — Defines ainm.memory.evict method, request/response format, error codes
- `spec/agent-card.md` §3.2 — Agent Card HTTP discovery, caching requirements (ETag, If-None-Match)
- `spec/transport.md` §7 — Error code registry (for evict error codes)

### Schemas
- `schemas/memory.schema.json` — JSON Schema for memory operations including evict
- `schemas/agent-card.schema.json` — Agent Card schema (for ETag content serialization)

### Demo Source
- `demo/src/server.rs` — Server dispatch, Agent Card HTTP handler, tool.call forwarding
- `demo/src/repl.rs` — REPL commands, tool event rendering
- `demo/src/agent.rs` — Agent memory scenario, tool event construction
- `demo/src/protocol.rs` — MessageEnvelope struct, identity types
- `demo/src/identity.rs` — Identity.resolve method (unused warning source)

### Audit Reference
- `.planning/v3.1-MILESTONE-AUDIT.md` — Tech debt items driving this phase

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `/memory` scenario in agent.rs: Full store + query + context-push cycle — evict follows same JSON-RPC pattern
- `serve_agent_card` handler in server.rs: Currently returns 200 with JSON body — add ETag header and If-None-Match check
- `protocol::new_request` / `protocol::new_error`: Used for all JSON-RPC request/response pairs

### Established Patterns
- REPL commands: Match on user input string, send JSON-RPC request, display response
- Server dispatch: Match on method name, route to handler or forward to connection
- Memory operations: Routed through server as JSON-RPC (not direct agent-to-agent)

### Integration Points
- REPL input handler (repl.rs): Add `/evict` alongside existing `/memory`, `/card`, `/bridge`
- Agent response handler (agent.rs): Add `ainm.memory.evict` method handler
- Server forwarding (server.rs): Memory methods already forwarded to agent — evict should follow same path

### Cargo Warnings (3 current)
- `identity.rs:91` — `fn resolve` never used
- `protocol.rs:87` — `struct MessageEnvelope` never constructed
- `protocol.rs:101,114` — `to_value` / `from_value` never used

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard approaches per spec definitions. Follow existing demo patterns for consistency.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 06-demo-feature-gaps-cleanup*
*Context gathered: 2026-03-29*
