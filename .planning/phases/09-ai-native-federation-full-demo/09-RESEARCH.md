# Phase 9: AI-Native Federation & Full Demo - Research

**Researched:** 2026-03-29
**Domain:** Rust/Axum federation relay, JSON-RPC method dispatch, cross-server event relay, Docker demo
**Confidence:** HIGH

## Summary

Phase 9 completes the AINM federation story by making the four AI-native features -- agent discovery, tool call transparency, memory propagation, and streaming -- work seamlessly across federation boundaries. The existing codebase already has all the building blocks: streaming events (streaming.rs), tool call events (agent.rs), memory operations (server.rs), agent card serving (server.rs), relay infrastructure (relay.rs/federation.rs), and a docker-compose two-server setup. The work is primarily integration: wiring existing local-only features to trigger relay when the thread is federated, adding two new endpoints/handlers (agent card resolution and context-push), and building a comprehensive demo script.

The architecture is straightforward because the CONTEXT.md decisions (D-04/D-05/D-06) mandate uniform relay handling -- streaming events, tool events, and messages all flow through the same `ainm.relay` method. The only new JSON-RPC method is `ainm.context-push` for memory propagation on join. The relay_handler already uses a match dispatch pattern (ainm.relay / ainm.backfill) that trivially extends.

**Primary recommendation:** Implement in three waves: (1) agent card resolution + streaming/tool event relay, (2) context-push for memory propagation, (3) comprehensive demo script exercising all four features end-to-end.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: Agent cards resolved via public HTTPS GET to `/.well-known/ainm/{agent-name}/agent-card.json`. No mTLS required.
- D-02: Per-agent URL path (not directory listing). Discovery starts by resolving @provider domain.
- D-03: Agent cards cached in-memory with TTL in existing RemoteServerCache.
- D-04: All event types (StreamStart/Delta/End, ToolCallStarted/Progress/Completed) relayed via existing `ainm.relay` method. Each event wrapped individually.
- D-05: Tool call events and streaming events use the same relay mechanism. Uniform handling.
- D-06: On receiving server, relayed events delivered through same broadcast::channel. REPL/client filters by event type.
- D-07: Origin server pushes active context frames to remote server immediately when remote agent joins federated thread.
- D-08: Only active context frames propagated (not conversation history or archival). Lightweight.
- D-09: New `ainm.context-push` JSON-RPC method on existing relay endpoint. Payload: thread_id + array of active context frames.
- D-10: Single comprehensive demo scenario exercising all four AI-native features in sequence.
- D-11: Reuses existing slash commands (/stream, /weather, /memory) that work in single-server mode.
- D-12: New `/discover helper@server-b.local` REPL command for remote agent card resolution.

### Claude's Discretion
- Internal relay payload format for streaming/tool events (as long as they use ainm.relay)
- TTL duration for agent card cache (5 min suggested but flexible)
- Active context frame selection logic on origin server
- Demo script structure and output formatting
- Order of relay_handler match arms for ainm.context-push

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AINF-01 | Agent Cards discoverable across federation boundaries | Dynamic agent-card endpoint on mTLS listener + HTTPS GET resolution from remote servers. Extend RemoteServerCache for TTL-based agent card caching. |
| AINF-02 | Tool call visibility events relayed across federated servers | Tool events (started/progress/completed) are already signed messages. Wire agent.rs to call relay_sender.enqueue() when thread is federated. |
| AINF-03 | Memory context propagated to agents on remote servers when joining federated thread | New ainm.context-push handler in relay_handler match dispatch. Origin server assembles context frame and sends on join. |
| AINF-04 | Streaming responses relayed across federation in real-time | Streaming events are notifications. Wire server.rs streaming passthrough to also relay when thread is federated. |
| DEMO-05 | Demo shows cross-server streaming | Comprehensive demo script exercising discover -> stream -> tool -> memory across two Docker containers. |
</phase_requirements>

## Architecture Patterns

### Current Codebase Architecture (key for this phase)

```
demo/src/
  main.rs          -- CLI, federation mode startup, two-listener architecture
  federation.rs    -- relay_handler (inbound), ThreadRegistry, FederationState
  relay.rs         -- RelaySender (outbound), persistent queue, backoff
  streaming.rs     -- StreamStart/Delta/End lifecycle (local WebSocket only)
  agent.rs         -- Tool call events, memory demo, bridge demo (local WS only)
  server.rs        -- Axum router, WS handler, federation-aware message routing
  identity.rs      -- RemoteServerCache, ServerIdentity, TOFU verification
  repl.rs          -- REPL with slash commands, incoming message rendering
  render.rs        -- Terminal rendering for all event types
  persistence.rs   -- redb backend for queues, dedup, TOFU pins
```

### Pattern 1: Federation-Aware Event Relay

**What:** The server.rs handle_agent_connection already checks if a thread is federated for regular messages. The same pattern extends to streaming and tool events.

**Current state:** In server.rs lines 618-621, streaming and tool events from the agent are forwarded to `to_human_tx` (local only):
```rust
"ainm.stream.start" | "ainm.stream.delta" | "ainm.stream.end"
| "ainm.presence.update" => {
    let _ = to_human_tx.send(text_str);
}
```

**Required change:** For federated threads, these events must ALSO be relayed via `relay_sender.enqueue()`. The thread_id is in the event params, so federation check is: extract thread_id -> parse_origin_domain -> if origin != server_id, relay to origin.

### Pattern 2: Relay Handler Match Dispatch

**What:** federation.rs relay_handler uses a match on `method` to dispatch:
```rust
match method {
    "ainm.relay" => {} // fall through to relay handling
    "ainm.backfill" => { return handle_backfill(...).await; }
    _ => { return error -32601 }
}
```

**Extension:** Add `"ainm.context-push"` arm before the wildcard.

### Pattern 3: Headless Agent Event Processing

**What:** In federation mode, the agent on server-b runs headlessly (main.rs line 488+), subscribing to `local_delivery_tx` broadcast channel. Currently it only handles `ainm.message.deliver`. For Phase 9, it also needs to:
1. Generate streaming responses (not just plain text echo)
2. Generate tool call events
3. Process context-push deliveries

**Key insight:** The headless agent currently only sends signed text messages back via relay. For streaming and tool calls, it needs to send those events via the same relay_sender. The headless agent already has access to `relay_sender` and `local_identity`.

### Pattern 4: Agent Card Dynamic Routing

**What:** server.rs currently has a hardcoded route:
```rust
.route("/.well-known/ainm/helper/agent-card.json", get(agent_card_handler))
```

**Required change:** Make this dynamic per D-01/D-02:
```rust
.route("/.well-known/ainm/{agent_name}/agent-card.json", get(agent_card_handler))
```
This needs to be on BOTH listeners (plain TCP for local and mTLS for federation). Per D-01, agent cards are public -- no mTLS required. But in the demo architecture, external servers only access the mTLS listener, so the route must exist there.

### Anti-Patterns to Avoid
- **Do NOT create a separate relay path for streaming events.** D-04/D-05 mandate they go through `ainm.relay` like regular messages. The relay_sender already handles JSON values generically.
- **Do NOT modify the relay_handler to inspect message content types.** The relay handler validates signatures and delivers -- it does not need to know if a message is a tool event or a stream delta. The broadcast channel delivers everything; the REPL client filters.
- **Do NOT block on context-push delivery.** The origin server should fire-and-forget the context push via relay_sender.enqueue(), same as regular messages.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TTL cache | Custom timestamp-tracking HashMap | `Instant`-based expiry check in `RemoteServerCache` | Simple: store `(value, Instant)`, check on get |
| HTTP client for agent card fetch | Raw TCP like repl.rs fetch_agent_card | `reqwest` (already a dependency via relay) | The mTLS reqwest client is already built; for public agent cards, use a plain reqwest client |
| Context frame assembly | Custom serialization | Reuse the `ainm.context.frame` format from server.rs memory handler | Lines 494-524 in server.rs already build context frames |

## Common Pitfalls

### Pitfall 1: Streaming Events Missing thread_id
**What goes wrong:** StreamStart/Delta/End notifications in streaming.rs include thread_id in StreamStart but NOT in Delta. Federation routing needs thread_id to decide relay.
**Why it happens:** The streaming spec optimizes deltas by only including stream_id and block_index.
**How to avoid:** Track active stream_id -> thread_id mapping in the server. When a StreamStart arrives with a thread_id, associate stream_id with that thread. Use that mapping for relay decisions on Delta/End events.
**Warning signs:** Relay sender receives a delta event but cannot determine destination.

### Pitfall 2: Headless Agent Cannot Stream
**What goes wrong:** The current headless agent (main.rs federation mode) sends plain signed messages via relay_sender.enqueue(). It has no WebSocket connection, so it cannot use streaming.rs::send_streamed_response() which requires a WsSender.
**Why it happens:** streaming.rs is tightly coupled to WebSocket -- it calls ws_sender.send() directly.
**How to avoid:** For federation mode, the headless agent should either: (a) emit streaming events as individual relay messages (StreamStart, then deltas, then StreamEnd with collapsed message), or (b) connect to local WebSocket like the legacy agent does. Option (a) is cleaner -- the agent constructs the same JSON payloads but enqueues them via relay_sender instead of WS.
**Warning signs:** Agent crashes or sends nothing when asked to stream in federation mode.

### Pitfall 3: Context-Push Before Remote Agent Has Keys
**What goes wrong:** Origin server sends ainm.context-push to remote server, but the remote server's relay_handler tries to verify signatures and fails because context-push is a server-to-server operation, not a participant message.
**Why it happens:** The relay_handler currently expects all relay payloads to have per-message signatures.
**How to avoid:** ainm.context-push is a new method on the relay endpoint -- it bypasses the ainm.relay message processing. It only needs server-level signature verification (Layer 2), not per-message signature verification (Layer 3). Handle it as a separate match arm, similar to ainm.backfill.
**Warning signs:** -32010 errors on context-push delivery.

### Pitfall 4: Docker Not Available on Dev Machine
**What goes wrong:** docker and docker-compose are not installed on this development machine.
**Why it happens:** macOS without Docker Desktop installed.
**How to avoid:** The demo script and docker-compose changes can be written and tested via `cargo build` + manual two-process launch locally. Docker testing deferred to CI or manual verification.
**Warning signs:** `docker: command not found`.

### Pitfall 5: Agent Card Route Conflict on mTLS Listener
**What goes wrong:** The mTLS Axum app in main.rs currently only has `/.well-known/ainm/server` and `/ainm/v1/relay`. Adding agent card routes requires the app to have access to ServerState (for agent_card data), but the mTLS app uses FederationState.
**Why it happens:** The two listeners serve different state types.
**How to avoid:** Either nest the agent card in FederationState, or add a separate handler on the mTLS listener that reads agent card from a shared Arc. Since the agent card is a static JSON value, pass it as `Arc<serde_json::Value>` into the mTLS router.

## Code Examples

### Relay streaming events from server.rs (federation-aware)
```rust
// In handle_agent_connection, replace the streaming passthrough:
"ainm.stream.start" | "ainm.stream.delta" | "ainm.stream.end" => {
    // Always deliver locally
    let _ = to_human_tx.send(text_str.clone());

    // Federation relay if thread is federated
    if let Some(ref fed) = state.federation {
        // Extract thread_id from params (StreamStart has it directly)
        let thread_id = parsed.get("params")
            .and_then(|p| p.get("thread_id"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let origin = federation::parse_origin_domain(thread_id).unwrap_or("");
        if !origin.is_empty() && origin != fed.server_id {
            // Wrap notification as relay payload
            fed.relay_sender.enqueue(origin, parsed.clone());
        }
    }
}
```

### Agent card caching with TTL in RemoteServerCache
```rust
pub struct AgentCardEntry {
    pub card: serde_json::Value,
    pub fetched_at: std::time::Instant,
    pub ttl: std::time::Duration,
}

impl AgentCardEntry {
    pub fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}
```

### ainm.context-push handler skeleton
```rust
"ainm.context-push" => {
    // Server-level signature verification only (Layer 2)
    // No per-message signatures needed -- this is server-to-server
    let params = match body.get("params") {
        Some(p) => p.clone(),
        None => return error(-32602, "Missing params"),
    };
    let thread_id = params.get("thread_id").and_then(|t| t.as_str());
    let frames = params.get("context_frames").and_then(|f| f.as_array());

    // Deliver context frames to local participants via broadcast
    let deliver = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "ainm.context.frame",
        "params": params,
    });
    let _ = state.local_delivery_tx.send(serde_json::to_string(&deliver).unwrap());

    return success(id, json!({"accepted": true}));
}
```

### Headless agent streaming via relay
```rust
// Instead of WebSocket streaming, build events and relay them
fn build_stream_events(identity: &Identity, thread_id: &str, text: &str)
    -> Vec<serde_json::Value>
{
    let stream_id = protocol::new_message_id();
    let message_id = protocol::new_message_id();
    let mut events = Vec::new();

    // StreamStart
    events.push(protocol::new_notification("ainm.stream.start", json!({
        "stream_id": stream_id,
        "message_id": message_id,
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "blocks": [{"type": "text", "version": "1.0", "source": "agent"}]
    })));

    // Split into deltas (simplified)
    for chunk in text.split_inclusive(' ').collect::<Vec<_>>().chunks(3) {
        let delta_text: String = chunk.iter().copied().collect();
        events.push(protocol::new_notification("ainm.stream.delta", json!({
            "stream_id": stream_id,
            "block_index": 0,
            "delta": delta_text
        })));
    }

    // StreamEnd with collapsed signed message
    // ... (sign and add collapsed message)
    events
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcoded agent-card route | Dynamic per-agent route with path param | Phase 9 | Enables cross-federation agent discovery |
| Local-only streaming/tool events | Federation-aware event relay | Phase 9 | Events visible across server boundaries |
| No memory propagation on join | ainm.context-push on federated join | Phase 9 | Agents get context immediately, no cold start |
| Echo-only headless agent | Full-featured headless agent (stream, tool, memory) | Phase 9 | Demo exercises all AI-native features |

## Open Questions

1. **Stream delta thread_id tracking**
   - What we know: StreamStart includes thread_id, but StreamDelta only has stream_id
   - What's unclear: Whether to add thread_id to all delta events or maintain server-side stream_id -> thread_id mapping
   - Recommendation: Add thread_id to delta/end events when relaying (enrichment). Simpler than maintaining a mapping. The spec allows additional fields.

2. **Agent card on plain HTTP vs mTLS for federation discovery**
   - What we know: D-01 says "public HTTPS GET, no mTLS required"
   - What's unclear: In the demo, remote servers only reach each other via mTLS. Should agent card route be on the mTLS listener (simpler) or a separate plain HTTPS listener (spec-correct)?
   - Recommendation: Put it on the mTLS listener for the demo. The mTLS client can still fetch it -- mTLS doesn't prevent serving public data. Add a comment noting production would use plain HTTPS.

3. **Headless agent command dispatch**
   - What we know: The headless agent currently only echoes text. For the demo, it needs to respond to "/stream", "/weather", "/memory" style triggers.
   - What's unclear: Exact trigger mechanism -- parse incoming message text or use a different signal?
   - Recommendation: Parse incoming message text for keywords (same as agent.rs handles), matching the single-server demo behavior per D-11.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo | Build server | Yes | 1.94.0 | -- |
| Docker | Demo containers | No | -- | Manual two-process local launch; Docker testing deferred to CI |
| Docker Compose | Two-server demo | No | -- | Same as above |
| reqwest | HTTP client for agent card fetch | Yes | (cargo dependency) | -- |
| redb | Persistent storage | Yes | (cargo dependency) | -- |

**Missing dependencies with no fallback:**
- None (Docker is convenient but not blocking)

**Missing dependencies with fallback:**
- Docker/Docker Compose: Build and test locally with two terminal processes (`cargo run -- --mode human ...` and `cargo run -- --mode agent ...`). Docker-compose.yml and demo script can be written without Docker installed.

## Project Constraints (from CLAUDE.md)

- Protocol-first: Spec must be updated for ainm.context-push before implementation
- Open standard: All changes must be open source
- Federation: No single point of control
- Transport: WebSocket primary, HTTP/SSE fallback, HTTP federation
- Security: Ed25519 signing non-negotiable -- context-push payloads need server-level signatures
- Wire format: JSON-RPC 2.0 for all protocol operations
- Spec format: Update transport.md, memory.md, agent-card.md as needed
- Schema validation: Update relevant JSON schemas

## Sources

### Primary (HIGH confidence)
- Existing codebase: demo/src/*.rs -- all source files read and analyzed
- spec/agent-card.md -- Agent Card discovery mechanism (Section 3)
- spec/memory.md -- Three-tier memory model and context frame format
- spec/transport.md -- Relay endpoint format, JSON-RPC methods

### Secondary (MEDIUM confidence)
- .planning/phases/09-ai-native-federation-full-demo/09-CONTEXT.md -- All implementation decisions
- .planning/REQUIREMENTS.md -- Requirement definitions (AINF-01 through DEMO-05)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - existing codebase with no new dependencies
- Architecture: HIGH - extending established patterns (relay, broadcast, match dispatch)
- Pitfalls: HIGH - identified from direct code analysis of current limitations
- Demo feasibility: MEDIUM - Docker not available locally, but local two-process testing is viable

**Research date:** 2026-03-29
**Valid until:** 2026-04-28 (stable -- extending existing patterns)
