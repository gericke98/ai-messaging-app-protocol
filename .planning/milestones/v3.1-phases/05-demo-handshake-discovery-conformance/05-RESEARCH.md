# Phase 5: Demo Handshake & Discovery Conformance - Research

**Researched:** 2026-03-28
**Domain:** Rust demo conformance -- WebSocket handshake lifecycle and Agent Card HTTP endpoint
**Confidence:** HIGH

## Summary

Phase 5 closes two integration gaps (INT-01, INT-02) and one broken flow (WebSocket lifecycle) identified in the v3.1 milestone audit. All three issues are demo-only -- the spec prose, JSON Schema, and AsyncAPI definitions are already correct. The work is purely in the Rust demo binary (`demo/src/`) and `website/docs/demo.md`.

The changes are surgically scoped: (1) both demo clients (repl.rs, agent.rs) must send an `ainm.initialized` JSON-RPC notification after receiving the init response; (2) the server (server.rs) must track handshake state per connection and reject any method with error `-32021` before `ainm.initialized` is received; (3) the Agent Card HTTP route must change from `/.well-known/ainm/agent-card/helper` to `/.well-known/ainm/helper/agent-card.json`; and (4) `demo.md` must correct the hardcoded `127.0.0.1:9090` reference to reflect the actual random-port binding behavior.

**Primary recommendation:** Implement in 2 plans: Plan 1 handles the `ainm.initialized` handshake lifecycle (clients + server enforcement), Plan 2 handles Agent Card URL conformance + demo.md fix. Both plans are code-only changes to existing Rust files with no new dependencies.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TRNS-01 | Protocol defines WebSocket transport binding for client-to-server real-time communication | Handshake lifecycle is part of WebSocket transport; `ainm.initialized` completes the connection lifecycle defined in transport.md Section 4.2 |
| TRNS-04 | Protocol uses JSON-RPC 2.0 as the message framing format | `ainm.initialized` is a JSON-RPC 2.0 notification (no `id`); `-32021` is a JSON-RPC error response |
| DISC-01 | Agents publish capability cards (Agent Cards) as JSON at a well-known endpoint | Agent Card URL must follow `/.well-known/ainm/{name}/agent-card.json` pattern per agent-card.md Section 3.2 |
| DISC-02 | Agent Cards are discoverable by resolving the `@provider` portion of an agent's identity | Identity record's `agent_card_url` field must point to the spec-correct URL path |
| SPEC-05 | Working demo proves the protocol end-to-end | Demo must implement the full handshake lifecycle and serve Agent Card at the correct URL to be conformant |
</phase_requirements>

## Architecture Patterns

### Current Demo Architecture (Unchanged)

```
main.rs
  |-- Generates identities (human + agent)
  |-- Builds IdentityStore + Agent Card JSON
  |-- Starts Axum server (server.rs) on 127.0.0.1:0
  |-- Spawns agent client (agent.rs) via tokio-tungstenite
  |-- Runs human REPL (repl.rs) via tokio-tungstenite
```

All three components connect over WebSocket on the same random port. The server routes messages between human and agent via `broadcast::channel`. There is no external HTTP client library -- the REPL uses raw TCP for the `/card` HTTP GET.

### Pattern 1: Handshake State Machine (Server-Side)

**What:** The server must track whether each WebSocket connection has completed the `ainm.initialized` handshake before allowing any other method calls.

**Current state:** `handle_ws_connection()` in server.rs receives the `ainm.initialize` request at line 88, sends the response at line 107-129, and immediately transitions to `handle_agent_connection()` or `handle_human_connection()`. There is no handshake state tracking and no enforcement point.

**Required change:** After sending the init response, the server must wait for the `ainm.initialized` notification before dispatching to the connection handler. Any non-`ainm.initialized` message received during this wait must get a `-32021` error response.

**Implementation approach:**
```rust
// After sending init_response (line 129), add:

// Wait for ainm.initialized notification
loop {
    match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
                if method == "ainm.initialized" {
                    tracing::info!("Handshake complete for: {}", from_addr);
                    break;
                }
                // Any other method before initialized -> -32021
                if let Some(req_id) = parsed.get("id").cloned() {
                    let err = protocol::new_error(req_id, -32021, "Not initialized");
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Text(
                        serde_json::to_string(&err).unwrap().into(),
                    )).await;
                }
                // Notifications without id: silently drop (per JSON-RPC spec, no error for notifications)
            }
        }
        _ => return, // Connection closed or binary frame
    }
}
```

**Why this approach:** The server already has a sequential read pattern (read init request, send response, then dispatch). Adding a second read loop between init-response and dispatch is the minimal change. The `-32021` enforcement naturally happens inside this loop. Once `ainm.initialized` is received, execution falls through to the existing `handle_agent_connection`/`handle_human_connection` functions -- no changes needed in those functions.

### Pattern 2: Client Sends `ainm.initialized` Notification

**What:** Both `repl.rs` and `agent.rs` must send an `ainm.initialized` JSON-RPC notification immediately after receiving the init response.

**Current state:** Both clients send `ainm.initialize` (request), receive the response, log "connected and initialized", and proceed to normal operation. They never send the `ainm.initialized` notification.

**Implementation approach:**
```rust
// In both repl.rs and agent.rs, after the "Wait for initialized response" block:

// Send ainm.initialized confirmation notification
let initialized_notif = protocol::new_notification("ainm.initialized", serde_json::json!({}));
let initialized_text = serde_json::to_string(&initialized_notif)?;
ws_sender.send(Message::Text(initialized_text.into())).await?;
```

**Note on notification payload:** The spec shows `ainm.initialized` as `{"jsonrpc": "2.0", "method": "ainm.initialized"}` with no params. However, `protocol::new_notification()` always sets `params: Some(...)` with a `skip_serializing_if = "Option::is_none"` on the field. Passing `serde_json::json!({})` is acceptable (empty object). Alternatively, the code could pass `serde_json::json!(null)` and let `skip_serializing_if` omit it, but the `new_notification` function does `Some(params)`, so null would serialize. The cleanest approach: either use empty object `{}` (harmless, spec doesn't forbid extra empty params) or add a `new_notification_no_params()` helper. The simpler path is to just use `serde_json::json!({})` -- it matches the spirit of the spec.

**Even simpler:** Modify `new_notification` to accept `Option<serde_json::Value>` or add a separate constructor. But given the demo scope, passing empty params is pragmatically fine.

### Pattern 3: Agent Card URL Path Correction

**What:** The Agent Card HTTP endpoint must change from `/.well-known/ainm/agent-card/helper` (current, segments inverted) to `/.well-known/ainm/helper/agent-card.json` (spec-correct per agent-card.md Section 3.2).

**Affected locations (4 total):**

1. **server.rs line 54:** Route registration
   ```rust
   // Current: "/.well-known/ainm/agent-card/helper"
   // Correct: "/.well-known/ainm/helper/agent-card.json"
   ```

2. **main.rs line 42:** Initial identity store registration (before server starts, uses placeholder address)
   ```rust
   // Current: "http://127.0.0.1:0/.well-known/ainm/agent-card/helper"
   // Correct: "http://127.0.0.1:0/.well-known/ainm/helper/agent-card.json"
   ```

3. **main.rs line 115:** Updated identity store registration (after server binds, uses real address)
   ```rust
   // Current: format!("http://{}/.well-known/ainm/agent-card/helper", server_addr)
   // Correct: format!("http://{}/.well-known/ainm/helper/agent-card.json", server_addr)
   ```

4. **repl.rs line 290:** HTTP fetch URL in `/card` command handler
   ```rust
   // Current: format!("http://{}/.well-known/ainm/agent-card/helper", server_addr)
   // Correct: format!("http://{}/.well-known/ainm/helper/agent-card.json", server_addr)
   ```

**No schema or spec changes needed.** The spec already defines the correct pattern. This is purely a demo code fix.

### Pattern 4: demo.md Port Documentation Fix

**What:** `website/docs/demo.md` line 53 says the server runs on `127.0.0.1:9090` but the demo actually binds to `127.0.0.1:0` (OS-assigned random port).

**Fix:** Replace "Axum WebSocket server on `127.0.0.1:9090`" with "Axum WebSocket server on `127.0.0.1:0` (random available port, printed at startup)".

### Anti-Patterns to Avoid

- **Adding session state to ServerState:** Do NOT add a global `initialized: HashMap<String, bool>` to `ServerState`. The handshake is per-connection and should be enforced inline within `handle_ws_connection()` before dispatching. Adding global state creates synchronization complexity for zero benefit.

- **Using `Option<params>` gymnastics for ainm.initialized:** Do NOT spend time making `new_notification` accept optional params. The spec shows no params but does not forbid empty params. Use `serde_json::json!({})` and move on. The demo is not a conformance test suite.

- **Changing the Axum route to use a path parameter:** Do NOT use `.route("/.well-known/ainm/:name/agent-card.json", ...)`. The demo has exactly one agent. A literal path `/.well-known/ainm/helper/agent-card.json` is simpler and sufficient.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON-RPC notification | Custom struct | `protocol::new_notification()` | Already exists, handles jsonrpc/method/params |
| JSON-RPC error | Manual JSON | `protocol::new_error()` | Already exists, handles code/message fields |
| WebSocket message sending | Raw frame construction | `Message::Text(...)` via futures SinkExt | Already the pattern used throughout the demo |

**Key insight:** Every building block needed for this phase already exists in the demo codebase. No new Cargo dependencies, no new modules, no new types. This is purely wiring existing primitives into the correct sequence.

## Common Pitfalls

### Pitfall 1: Agent Connects Before Server Is Ready for Initialized

**What goes wrong:** The agent client (agent.rs) connects, sends `ainm.initialize`, receives response, then sends `ainm.initialized`. But the server's `handle_ws_connection` has already dispatched to `handle_agent_connection` before the initialized notification arrives, meaning the notification goes to the method dispatch loop instead of the handshake loop.

**Why it happens:** The current code structure dispatches immediately after init response. If the server code is refactored incorrectly, the `ainm.initialized` notification could end up in the wrong handler.

**How to avoid:** The handshake enforcement loop MUST be added between the init response send (line 129) and the is_agent dispatch (line 134) in server.rs. The dispatch must only happen after the loop breaks on receiving `ainm.initialized`.

**Warning signs:** The agent logs "connected and initialized" but server never logs "handshake complete". Or messages start working even though ainm.initialized was never sent.

### Pitfall 2: Timing Race Between REPL and Agent

**What goes wrong:** In main.rs, the agent spawns (line 147), sleeps 200ms, then the REPL starts (line 157). Both connect via WebSocket. If the handshake enforcement adds latency (waiting for ainm.initialized), the 200ms sleep might not be enough, causing the REPL to try connecting before the agent has finished its handshake.

**Why it happens:** The server handles connections sequentially in the current design (agent connects first, then human). Adding a mandatory wait for `ainm.initialized` on the agent connection could block the server's ability to accept the human connection if they are handled on the same task.

**How to avoid:** Verify that the server spawns each WebSocket connection handler as a separate tokio task (it does -- `ws.on_upgrade()` spawns a new task per connection). This means the agent and human connections are independent. The 200ms sleep should be sufficient since the `ainm.initialized` round-trip is < 1ms on localhost. If paranoid, increase the sleep to 500ms, but it should not be needed.

**Warning signs:** REPL hangs on startup without showing the prompt.

### Pitfall 3: Notification vs Request Confusion for ainm.initialized

**What goes wrong:** `ainm.initialized` is a notification (no `id` field). If a client accidentally sends it as a request (with an `id`), the server might not recognize it. Conversely, if the server tries to send a response to a notification, it violates JSON-RPC.

**Why it happens:** The protocol module has both `new_request()` and `new_notification()`. Easy to pick the wrong one.

**How to avoid:** Use `protocol::new_notification("ainm.initialized", ...)`. Verify the serialized JSON has no `id` field. On the server side, check for `method == "ainm.initialized"` regardless of whether `id` is present (be liberal in what you accept).

**Warning signs:** Server logs show an unrecognized method or the handshake loop never completes.

### Pitfall 4: demo.md Shows Wrong Agent Card URL After Fix

**What goes wrong:** demo.md might have the old Agent Card URL path mentioned somewhere, or the REPL help text might reference it.

**How to avoid:** After changing the URL path, grep the entire repo for `agent-card/helper` to ensure no stale references remain. Check main.rs banner, repl.rs /help output, and demo.md.

## Code Examples

### Example 1: Sending ainm.initialized notification (client side)

```rust
// Source: spec/transport.md Section 4.3, JSON-RPC notification format
let initialized = protocol::new_notification("ainm.initialized", serde_json::json!({}));
let text = serde_json::to_string(&initialized)?;
ws_sender.send(Message::Text(text.into())).await?;
```

Serializes to:
```json
{"jsonrpc":"2.0","method":"ainm.initialized","params":{}}
```

### Example 2: Server -32021 error response

```rust
// Source: spec/transport.md Section 7, error code -32021
let err = protocol::new_error(req_id, -32021, "Not initialized");
let err_text = serde_json::to_string(&err).unwrap();
sender.send(Message::Text(err_text.into())).await;
```

Serializes to:
```json
{"jsonrpc":"2.0","id":1,"result":null,"error":{"code":-32021,"message":"Not initialized"}}
```

### Example 3: Corrected Agent Card route

```rust
// Source: spec/agent-card.md Section 3.2
// Pattern: /.well-known/ainm/{name}/agent-card.json
.route(
    "/.well-known/ainm/helper/agent-card.json",
    get(agent_card_handler),
)
```

## State of the Art

| Old Approach (Current Demo) | Correct Approach (Spec) | Impact |
|-----|-----|-----|
| Skip `ainm.initialized` | Send notification after init response | Handshake lifecycle incomplete |
| No `-32021` enforcement | Server rejects pre-handshake methods | Security gap (demo only) |
| `/.well-known/ainm/agent-card/helper` | `/.well-known/ainm/helper/agent-card.json` | Third-party clients would fail |
| `demo.md` says port 9090 | Demo uses port 0 (random) | Documentation mismatch |

## Open Questions

None. This phase is fully specified by the existing spec documents (transport.md Section 4.2-4.3, agent-card.md Section 3.2) and the audit findings (INT-01, INT-02). All changes are mechanical fixes to make the demo match the spec.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Demo build | Yes | 1.94.0 | -- |
| cargo | Demo build | Yes | 1.94.0 | -- |

No new external dependencies. The demo already builds cleanly (3 warnings, pre-existing). No additional crates needed.

## Project Constraints (from CLAUDE.md)

- **Protocol-first**: Spec is already correct; this phase only fixes demo conformance
- **GSD Workflow Enforcement**: Work must go through GSD commands
- **Wire format**: JSON-RPC 2.0 over WebSocket (no changes)
- **Crypto**: Ed25519 via ring crate (no changes)
- **Demo pattern**: Fully scripted, no API key, works offline (no changes)
- **camelCase enforcement**: All JSON field names must match JSON Schema defs (Phase 04 decision)
- **MCP-style lifecycle**: initialize/initialized/shutdown pattern (Phase 01 decision)

## Sources

### Primary (HIGH confidence)
- `spec/transport.md` Section 4.2-4.3 -- handshake lifecycle, `ainm.initialized` notification format, `-32021` error code
- `spec/agent-card.md` Section 3.2 -- Agent Card URL pattern `/.well-known/ainm/{name}/agent-card.json`
- `schemas/transport.schema.json` -- InitializeParams, InitializeResult definitions
- `asyncapi.yaml` -- session channel with InitializedNotification message
- `.planning/v3.1-MILESTONE-AUDIT.md` -- INT-01, INT-02, WebSocket lifecycle flow findings

### Secondary (HIGH confidence -- direct code inspection)
- `demo/src/server.rs` -- current handle_ws_connection flow, agent_card route at line 54
- `demo/src/repl.rs` -- current init handshake at lines 20-42, /card fetch at line 290
- `demo/src/agent.rs` -- current init handshake at lines 29-51
- `demo/src/main.rs` -- identity registration with agent_card_url at lines 42, 115
- `demo/src/protocol.rs` -- new_notification, new_error constructors
- `website/docs/demo.md` -- port 9090 documentation error at line 53

## Metadata

**Confidence breakdown:**
- Handshake lifecycle: HIGH -- spec is explicit, code locations identified, no ambiguity
- Agent Card URL: HIGH -- spec pattern is clear, all 4 code locations mapped
- demo.md fix: HIGH -- trivial text change, verified current binding at server.rs line 59
- Error code -32021: HIGH -- defined in transport.md Section 7, transport.schema.json

**Research date:** 2026-03-28
**Valid until:** Indefinite (spec is stable, demo code changes are mechanical)
