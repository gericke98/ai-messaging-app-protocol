# Phase 06: Demo Feature Gaps & Code Cleanup - Research

**Researched:** 2026-03-29
**Domain:** Rust demo implementation (Axum server, REPL client, agent), HTTP caching, dead code elimination
**Confidence:** HIGH

## Summary

Phase 06 closes the remaining demo-level gaps from the v3.1 milestone audit: implement the `ainm.memory.evict` operation end-to-end, add ETag/If-None-Match HTTP caching to the Agent Card endpoint, remove dead `ainm.tool.call.*` dispatch paths from server and REPL, and eliminate all 3 cargo warnings (unused `resolve` method, unused `MessageEnvelope` struct with its `to_value`/`from_value` methods).

All changes are confined to `demo/src/` -- no spec files, schemas, or AsyncAPI definitions need modification. The protocol specification and JSON Schema already fully define `ainm.memory.evict` (memory.md Section 5.3, memory.schema.json `MemoryEvictParams`), and the Agent Card caching spec (agent-card.md Section 4) describes the exact ETag/If-None-Match behavior. The implementation work is straightforward: follow existing patterns established in phases 3-5, apply the spec-defined behaviors to the demo code, and remove dead paths.

**Primary recommendation:** Structure work into two plans -- one for the evict feature + Agent Card caching (functional changes), one for dead code removal + cargo warning fixes (cleanup changes). Both are small and could also be combined into a single plan if preferred for efficiency.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Add `/evict` REPL command that evicts a previously stored archival fact by key. Follow the same pattern as `/memory` -- user types `/evict`, demo sends `ainm.memory.evict` JSON-RPC request through the server, server responds with confirmation. Evict the first fact stored in the `/memory` scenario (reuse existing key).
- **D-02:** Memory evict response should print the evicted key and a confirmation message, matching the existing `/memory` output style.
- **D-03:** Use SHA-256 content hash of the serialized Agent Card JSON as the ETag value. No server-side state or version counter needed -- deterministic from content.
- **D-04:** Server handler checks `If-None-Match` request header against computed ETag. Return 304 Not Modified if match, 200 with full body + ETag header if no match.
- **D-05:** Remove `ainm.tool.call.started`, `ainm.tool.call.progress`, `ainm.tool.call.completed` match arms from the server.rs forwarding dispatch (lines ~412-414). Tool events reach the human via `ainm.message.send` wrapper -- the raw method dispatch is dead code.
- **D-06:** Evaluate whether repl.rs tool.call.* rendering arms (lines ~192-205) are actually reached. If tool events arrive as `ainm.message.send` envelopes with type field, these match arms for raw method names may also be dead. Remove if unreachable.
- **D-07:** Remove unused `resolve` method from identity module, or wire it into demo if trivial. Prefer removal (demo doesn't need runtime address resolution).
- **D-08:** Remove unused `MessageEnvelope` struct and its `to_value`/`from_value` associated methods from protocol.rs, OR wire them into the demo message construction. Prefer the approach that produces fewer changes -- if the struct serves no demo purpose, remove it.

### Claude's Discretion
- Exact error message text for evict failures
- Whether to add a brief REPL help update listing `/evict`
- Order of operations for dead code removal vs warning fixes (can be combined logically)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MEMO-01 | Protocol defines three memory tiers: active context, conversation memory, archival memory | The evict operation completes the demo coverage of archival memory CRUD (store, query, evict). Spec fully defines `ainm.memory.evict` at memory.md Section 5.3 with error codes -32051 (wrong tier), -32053 (missing params), -32054 (not found), -32055 (auth mismatch). Schema defines `MemoryEvictParams` in memory.schema.json. |
| DISC-05 | Agent Cards are cacheable with explicit versioning | Agent Card caching via ETag/If-None-Match is specified in agent-card.md Section 4. Server MUST return ETag on 200 responses, honor If-None-Match for 304. D-03 locks SHA-256 content hash as the ETag computation strategy. |
| SPEC-05 | Working demo proves the protocol end-to-end | Both evict and caching are protocol features that the demo must exercise to fully prove the spec. Removing dead code and warnings ensures the demo is clean and maintainable. |
</phase_requirements>

## Standard Stack

No new dependencies needed. All work uses existing crate capabilities.

### Core (Already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ring | 0.17.x | SHA-256 digest for ETag computation | Already used for Ed25519 signing. `ring::digest::digest(&SHA256, bytes)` provides the hash. No new dependency. |
| axum | 0.8.x | HTTP handler modifications for ETag/304 | Already the server framework. Axum extractors can read request headers. |
| serde_json | 1.x | Agent Card serialization for hashing | Already used throughout. Deterministic serialization via sorted keys for consistent ETag. |

### Not Needed
| Library | Why Not |
|---------|---------|
| sha2 crate | `ring::digest` already provides SHA-256. Adding sha2 would duplicate functionality. |
| http-cache-semantics | Overkill. We need exactly one ETag computation + one If-None-Match check. |
| reqwest | HTTP client. The demo uses raw TCP for Agent Card fetch (no new dependency). |

## Architecture Patterns

### Existing Demo Architecture (Unchanged)
```
demo/src/
  main.rs           # Entry point, identity/card setup, banner
  server.rs         # Axum server, WebSocket handler, Agent Card HTTP
  repl.rs           # Terminal REPL, human input, agent card fetch
  agent.rs          # Mock agent, scripted scenarios
  protocol.rs       # JSON-RPC constructors, types
  identity.rs       # Ed25519 identity, key generation, store
  signing.rs        # Message signing/verification
  streaming.rs      # Streaming helper
  render.rs         # Terminal rendering functions
```

### Pattern 1: Memory Evict (follows existing memory store/query pattern)

**What:** `/evict` REPL command sends `ainm.memory.evict` JSON-RPC request to the agent, which forwards to the server, which removes the fact and responds with confirmation.

**Flow:**
```
REPL (/evict) --> agent.rs (handle_evict) --> ainm.memory.evict request --> server.rs
  server: look up key in archival_store, remove, return { evicted: true }
  agent: receive response, send ainm.memory.evicted notification to human
  REPL: render eviction confirmation
```

**Key insight:** The evict flow follows the exact same 3-step pattern as `/memory`:
1. REPL sends message to agent via `ainm.message.send`
2. Agent sends `ainm.memory.evict` JSON-RPC request to server
3. Server processes, responds, agent forwards notification to REPL

### Pattern 2: Agent Card ETag Caching (HTTP conditional request)

**What:** The `agent_card_handler` in server.rs computes a SHA-256 hash of the serialized Agent Card JSON, returns it as an ETag header on 200 responses, and checks `If-None-Match` for 304 responses.

**Implementation approach:**
```rust
// In agent_card_handler:
// 1. Serialize agent_card to deterministic JSON
// 2. SHA-256 hash the bytes
// 3. Format as hex ETag string: "sha256:<hex>"
// 4. Check If-None-Match request header
// 5. If match: return 304 with ETag header, no body
// 6. If no match: return 200 with JSON body + ETag header
```

**Axum pattern for custom response with headers:**
```rust
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

// Extract If-None-Match from request headers
async fn agent_card_handler(
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Response {
    let body = serde_json::to_vec(&state.agent_card).unwrap();
    let hash = ring::digest::digest(&ring::digest::SHA256, &body);
    let etag = format!("\"{}\"", hex::encode(hash.as_ref()));
    // ... check If-None-Match, return 304 or 200
}
```

**Note on hex encoding:** The demo does not have a `hex` crate. Options:
1. Use `base64` (already in Cargo.toml) to encode the hash -- valid for ETag values.
2. Implement simple hex encoding inline (it's ~3 lines for a &[u8] -> String).
3. Use format!("{:02x}") in a loop.

Recommendation: Use the already-available `base64` crate for encoding the SHA-256 hash. ETags are opaque strings -- they don't need to be hex. `base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash.as_ref())` produces a shorter, valid ETag.

### Pattern 3: Dead Code Removal

**What:** Remove match arms and struct definitions that are never executed.

**server.rs dead code (lines 412-414):**
The agent sends tool events as `ainm.message.send` requests with `type: "tool.call.started"` etc. in the envelope. The server handles these via the `"ainm.message.send"` match arm (line 202), which verifies, wraps in `ainm.message.deliver`, and forwards to human. The separate match arms for `"ainm.tool.call.started"` | `"ainm.tool.call.progress"` | `"ainm.tool.call.completed"` at lines 412-414 would only fire if the agent sent these as raw JSON-RPC methods (not wrapped in `ainm.message.send`). The agent never does this -- all tool events are wrapped in message envelopes. **Confirmed dead code.**

**repl.rs dead code (lines 192-205):**
The REPL receives tool events as `ainm.message.deliver` notifications from the server. Inside the `ainm.message.deliver` handler (line 115), the code checks the message `type` field and routes to `render::render_tool_event` for `"tool.call.started"`, `"tool.call.progress"`, `"tool.call.completed"` (lines 128-134). The separate top-level match arms at lines 192-205 for `"ainm.tool.call.started"` etc. as raw JSON-RPC method names never fire because these methods never arrive as raw notifications -- they always arrive wrapped in `ainm.message.deliver`. **Confirmed dead code.**

### Anti-Patterns to Avoid
- **ETag without quotes:** RFC 7232 requires ETags to be wrapped in double quotes (e.g., `"abc123"`). Missing quotes will break client parsers.
- **Non-deterministic serialization for ETag:** If `serde_json::to_vec` produces different key orderings on different runs, the ETag would change even when content is identical. `serde_json::to_vec` produces deterministic output for `serde_json::Value` because the demo uses `serde_json::json!()` which creates ordered maps. However, to be safe, consider using the canonical form.
- **Forgetting to update /help:** If `/evict` is added but not listed in `/help`, users won't discover it.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SHA-256 hashing | Manual implementation | `ring::digest::digest(&SHA256, ...)` | Already in the dependency tree, audited crypto library |
| HTTP 304 response | Custom TCP response builder | Axum's `impl IntoResponse` + `StatusCode::NOT_MODIFIED` | Framework handles proper HTTP response formatting |
| ETag string formatting | Manual quoting | `format!("\"{}\"", encoded_hash)` | Simple but must include the RFC-required quotes |

## Common Pitfalls

### Pitfall 1: Evict Before Store
**What goes wrong:** User runs `/evict` before `/memory`, so there's no fact to evict. The server returns error -32054 (fact not found).
**Why it happens:** `/evict` uses the key `user_preference.demo` which is only stored during the `/memory` scenario.
**How to avoid:** The agent's `handle_evict` should send the evict request regardless -- if the fact doesn't exist, the server returns the appropriate error and the REPL displays an error message. This is correct protocol behavior. Optionally, print a hint: "Run /memory first to store a fact."
**Warning signs:** User reports "evict doesn't work" -- actually they just haven't stored anything yet.

### Pitfall 2: ETag Quoting
**What goes wrong:** Server returns `ETag: abc123` instead of `ETag: "abc123"`. Clients send `If-None-Match: "abc123"` (with quotes) but the comparison fails because the server's value has no quotes.
**Why it happens:** RFC 7232 Section 2.3 requires strong ETags to be enclosed in double quotes. Easy to forget.
**How to avoid:** Always format as `format!("\"{}\"", hash_string)` -- the quotes are part of the ETag value.
**Warning signs:** 304 never returned despite identical content.

### Pitfall 3: axum Response Types
**What goes wrong:** The current `agent_card_handler` returns `Json<serde_json::Value>`. To return either 200+body or 304+no-body, the return type needs to change to a more flexible type.
**Why it happens:** `Json<T>` always returns 200. Need `impl IntoResponse` or `Response` for conditional responses.
**How to avoid:** Change return type to `impl IntoResponse` or `Response`. Axum allows tuples like `(StatusCode, HeaderMap, Json<T>)` for 200, and `(StatusCode, HeaderMap)` for 304.
**Warning signs:** Compilation errors about return type mismatch.

### Pitfall 4: Notification Method Name for Evict
**What goes wrong:** Using an inconsistent notification method name for eviction results (e.g., `ainm.memory.evict.result` vs `ainm.memory.evicted`).
**Why it happens:** The store operation uses `ainm.memory.stored` as the human notification. Evict should follow the same pattern.
**How to avoid:** Use `ainm.memory.evicted` as the notification method name, consistent with `ainm.memory.stored`.
**Warning signs:** REPL doesn't render eviction confirmation.

### Pitfall 5: serde_json::Value Serialization Stability
**What goes wrong:** ETag changes across server restarts even though the Agent Card content is identical.
**Why it happens:** `serde_json::Value::Object` uses `Map<String, Value>` which preserves insertion order. The `serde_json::json!()` macro inserts keys in source order. As long as the same code builds the card, serialization is stable.
**How to avoid:** The agent card is built once in `main.rs` with `json!()` and passed to the server -- it never changes during runtime. The hash will be stable for the process lifetime. Across restarts, the same source code produces the same insertion order, so the hash is stable.
**Warning signs:** None -- this is a theoretical concern, not a practical one for the demo.

## Code Examples

### Memory Evict: Server Handler
```rust
// Add to server.rs handle_agent_connection match block:
"ainm.memory.evict" => {
    if let Some(params) = parsed.get("params") {
        let tier = params.get("tier").and_then(|t| t.as_str()).unwrap_or("");
        let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("").to_string();

        if tier != "archival" {
            if has_id {
                let err = protocol::new_error(
                    parsed.get("id").cloned().unwrap(),
                    -32051,
                    "Only archival tier supports eviction",
                );
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        } else {
            // Support eviction by key (D-01: reuse existing key)
            let key = params.get("key").and_then(|k| k.as_str()).unwrap_or("").to_string();
            let fact_id = params.get("fact_id").and_then(|f| f.as_str()).map(|s| s.to_string());

            let mut store = archival_store.lock().await;
            let evicted = if let Some(_fid) = fact_id {
                // fact_id-based eviction: find by fact_id in values
                let to_remove: Option<String> = store.iter()
                    .find(|(_, v)| v.get("fact_id").and_then(|f| f.as_str()) == Some(_fid.as_str()))
                    .map(|(k, _)| k.clone());
                if let Some(k) = to_remove { store.remove(&k); true } else { false }
            } else if !key.is_empty() {
                store.remove(&key).is_some()
            } else {
                // Error: neither fact_id nor key provided
                if has_id {
                    let err = protocol::new_error(
                        parsed.get("id").cloned().unwrap(),
                        -32053,
                        "Either fact_id or key must be provided",
                    );
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                }
                false
            };

            if evicted {
                if has_id {
                    let resp = protocol::new_response(
                        parsed.get("id").cloned().unwrap(),
                        serde_json::json!({ "evicted": true }),
                    );
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                }
                // Notify human
                let evict_notify = protocol::new_notification(
                    "ainm.memory.evicted",
                    serde_json::json!({ "thread_id": thread_id, "key": key, "status": "evicted" }),
                );
                let _ = to_human_tx.send(serde_json::to_string(&evict_notify).unwrap());
            } else if has_id && !key.is_empty() {
                let err = protocol::new_error(
                    parsed.get("id").cloned().unwrap(),
                    -32054,
                    "Fact not found",
                );
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
    }
}
```

### Agent Card Handler with ETag
```rust
// Replace agent_card_handler in server.rs:
use axum::http::{header, HeaderMap, StatusCode};

async fn agent_card_handler(
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    let body = serde_json::to_vec(&state.agent_card).unwrap();
    let hash = ring::digest::digest(&ring::digest::SHA256, &body);
    let etag_value = format!(
        "\"{}\"",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash.as_ref())
    );

    // Check If-None-Match
    if let Some(inm) = headers.get(header::IF_NONE_MATCH) {
        if let Ok(inm_str) = inm.to_str() {
            if inm_str == etag_value {
                return (
                    StatusCode::NOT_MODIFIED,
                    [(header::ETAG, etag_value)],
                ).into_response();
            }
        }
    }

    // Return 200 with body and ETag
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::ETAG, etag_value),
        ],
        body,
    ).into_response()
}
```

### REPL Agent Card Fetch with If-None-Match
```rust
// In fetch_agent_card (repl.rs), add If-None-Match support:
// The demo does a fresh fetch each time /card is typed.
// Since there's no local cache in the REPL, ETag support is on the
// server side only. The REPL can optionally send If-None-Match
// if it stores the last ETag, but for demo purposes this is
// not required -- the server returning ETag headers is sufficient
// to satisfy DISC-05.
```

### Dead Code Removal: server.rs
```rust
// REMOVE these lines from the agent connection match block:
//                     "ainm.tool.call.started"
//                     | "ainm.tool.call.progress"
//                     | "ainm.tool.call.completed"
// The remaining match arm for streaming + presence is:
                    "ainm.stream.start"
                    | "ainm.stream.delta"
                    | "ainm.stream.end"
                    | "ainm.presence.update" => {
                        let _ = to_human_tx.send(text_str);
                    }
```

### Dead Code Removal: repl.rs
```rust
// REMOVE these match arms (lines ~192-205):
//                     "ainm.tool.call.started" => { ... }
//                     "ainm.tool.call.progress" => { ... }
//                     "ainm.tool.call.completed" => { ... }
// Tool events arrive via ainm.message.deliver with type field,
// handled at lines ~128-134 inside the deliver handler.
```

### Cargo Warning Fixes
```rust
// identity.rs: Remove the resolve method (lines 91-93)
// The demo registers identities and looks up public keys via
// get_public_key_bytes(), but never calls resolve().

// protocol.rs: Remove MessageEnvelope struct and its impl block (lines 86-129)
// The demo constructs all messages as serde_json::Value via json!() macro.
// MessageEnvelope was likely an early abstraction that was superseded.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Message construction via typed structs | Dynamic `serde_json::json!()` construction | Phase 03 | MessageEnvelope struct became dead code |
| Raw tool event method forwarding | Tool events wrapped in `ainm.message.send` | Phase 04 conformance | Raw dispatch arms became dead code |

## Open Questions

1. **REPL help text for /evict**
   - What we know: D-01 specifies adding the command, discretion area says "Whether to add a brief REPL help update listing /evict"
   - Recommendation: Add `/evict` to both the `/help` output and the startup banner. Minimal effort, improves discoverability.

2. **REPL-side ETag caching**
   - What we know: The REPL's `fetch_agent_card` uses raw TCP and does not store any cache. D-04 only requires the server to return ETag and honor If-None-Match.
   - Recommendation: Server-side ETag is sufficient for DISC-05. Optionally enhance the REPL to store the ETag from the first `/card` fetch and send `If-None-Match` on subsequent fetches to demonstrate the 304 flow, but this is not required by the success criteria.

## Project Constraints (from CLAUDE.md)

- **Protocol-first:** Spec is already complete for evict and caching; this phase implements spec in demo only.
- **Technology stack:** All changes use existing Cargo.toml dependencies (ring, axum, serde_json, base64). No new crates needed.
- **Rust demo patterns:** Follow existing code patterns in server.rs (match on method, process, respond) and agent.rs (scripted scenarios with `handle_*` functions).

## Sources

### Primary (HIGH confidence)
- `spec/memory.md` Section 5.3 -- Full `ainm.memory.evict` specification with error codes
- `spec/agent-card.md` Section 4 -- ETag/If-None-Match caching requirements
- `schemas/memory.schema.json` `MemoryEvictParams` -- Schema definition for evict parameters
- `demo/src/server.rs` -- Direct code inspection confirming dead code paths (tool.call.* forwarding)
- `demo/src/repl.rs` -- Direct code inspection confirming dead code paths (tool.call.* rendering)
- `demo/src/agent.rs` -- Direct code inspection confirming tool events sent via ainm.message.send only
- `cargo build` output -- 3 warnings confirmed: resolve, MessageEnvelope, to_value/from_value

### Secondary (MEDIUM confidence)
- Axum documentation for response types and header extraction -- consistent with training data for axum 0.8.x

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies, all existing crate APIs verified by current Cargo.toml and working demo
- Architecture: HIGH - Follows patterns established in Phases 03-05, all code paths traced through direct inspection
- Pitfalls: HIGH - Based on direct code analysis and RFC 7232 caching requirements
- Dead code analysis: HIGH - Traced complete message flow from agent.rs through server.rs to repl.rs, confirmed tool events only travel via ainm.message.send wrapper

**Research date:** 2026-03-29
**Valid until:** Stable -- all findings based on local codebase inspection and protocol spec. No external API or version dependencies.
