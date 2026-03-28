# Phase 4: Protocol Conformance Polish - Research

**Researched:** 2026-03-28
**Domain:** AsyncAPI/JSON Schema conformance, Rust demo alignment, protocol artifact consistency
**Confidence:** HIGH

## Summary

Phase 4 is a pure conformance-and-polish phase. All 57 v1 requirements are satisfied at the specification level. The tech debt is concentrated in 12 active findings from the v3.1 milestone audit: 2 AsyncAPI definition errors, 1 JSON Schema enum gap, 8 demo-to-schema field/behavior mismatches, and 1 undocumented field. No new protocol design is needed -- every fix has a clear "source of truth" in the spec or schema files.

The codebase is well-structured for surgical edits. The demo is 8 Rust source files (~1,500 lines total) using `serde_json::json!()` inline JSON construction, making field name fixes straightforward find-and-replace operations. The AsyncAPI YAML and JSON Schema files are canonical and well-organized -- schema $defs map 1:1 to spec sections.

**Primary recommendation:** Fix AsyncAPI + schema issues first (Plan 1), then fix demo code (Plan 2). Plan 1 changes are upstream of Plan 2 -- the demo must conform to the corrected schemas.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TRNS-01 | WebSocket transport binding for client-to-server | FINDING-03 + FINDING-05: demo init uses snake_case `protocol_version` and omits signature; transport.schema.json requires `protocolVersion` and `signature` |
| SECU-04 | Trust annotations on content blocks | FINDING-05: demo ainm.initialize omits `signature` field; server omits `nonce` in response. Schema requires both. |
| STRM-01 | Token-by-token streaming | FINDING-12: demo sends undocumented `sequence` field in StreamDelta. Schema has no `sequence` property. |
| STRM-03 | Streaming collapse into permanent events | FINDING-02: AsyncAPI stream.error references `StreamingErrorCodes` (catalog) instead of an event schema. |
| CONT-03 | Interactive block types (ButtonBlock, FormBlock, etc.) | FINDING-06: demo ApprovalBlock uses `prompt`/`option_id` instead of `title`/`value`. Schema requires `title`/`value`. |
| CONT-04 | Declarative component model | FINDING-04: demo uses `interactive: true` capability; schema uses `contentTypes` array. FINDING-06: field names wrong. |
| THRD-02 | Sub-thread creation | FINDING-01: AsyncAPI uses `ainm.sub_thread.create`; spec uses `ainm.thread.create` with `parent_thread_id`. |
| THRD-04 | Agent sub-thread creation | FINDING-01: same method name issue. |
| DISC-02 | Agent Card discoverable via @provider | FINDING-07 + FINDING-08: Agent Card missing `parallelBlocks` (uses `parallelStreaming`), missing `threading` and `memory` sub-objects. |
| TOOL-01 | ToolCallStarted event | FINDING-09: `MessageEnvelope.type` enum missing `tool.call.started`. |
| TOOL-02 | ToolCallProgress event | FINDING-09: enum missing `tool.call.progress`. |
| TOOL-03 | ToolCallCompleted event | FINDING-09: enum missing `tool.call.completed`. |
| MEMO-01 | Three memory tiers | FINDING-10: demo has no memory scenario. Need `/memory` command. |
| MEMO-02 | Active context | FINDING-10: demo lacks memory demonstration. |
| MEMO-03 | Conversation memory queryable | FINDING-10: demo lacks memory demonstration. |
| MEMO-04 | Archival memory | FINDING-10: demo lacks memory demonstration. |
| MEMO-05 | Structured context payloads | FINDING-10: demo lacks memory demonstration. |
| COMP-01 | MCP tool call mapping | FINDING-11: demo has no bridge scenario. Need `/bridge` command. |
| COMP-02 | MCP server as agent participant | FINDING-11: demo has no bridge scenario. |
| COMP-03 | A2A task lifecycle mapping | FINDING-11: demo has no bridge scenario. |
| COMP-04 | A2A task updates as thread messages | FINDING-11: demo has no bridge scenario. |
| SPEC-02 | All message types have JSON Schema | FINDING-02 + FINDING-09: StreamErrorNotification needs proper schema; MessageEnvelope.type enum incomplete. |
| SPEC-03 | AsyncAPI v3.1 transport bindings | FINDING-01 + FINDING-02: AsyncAPI has wrong method name and wrong schema ref. |
| SPEC-05 | Working demo proves protocol E2E | FINDING-03 through FINDING-12: demo field names, auth, Agent Card, memory, bridge scenarios all need fixes. |
</phase_requirements>

## Finding-by-Finding Analysis

This section details every finding, the exact source of truth, and the precise changes needed.

### FINDING-01 (Critical): AsyncAPI sub_thread method name

**Source of truth:** `spec/threading.md` Section 4.2 states: "Sub-thread creation uses the same `ainm.thread.create` method as root thread creation, with the addition of `parent_thread_id` and `parent_message_id` fields." The spec is unambiguous -- there is NO `ainm.sub_thread.create` method.

**Current state in `asyncapi.yaml`:**
- Line 86-88: channel message refs `subThreadCreateRequest` / `subThreadCreateResponse`
- Line 317-332: operation `sendSubThreadCreate` with `ainm.sub_thread.create`
- Line 662-675: component messages `SubThreadCreateRequest` (name: `ainm.sub_thread.create`) and `SubThreadCreateResponse`
- The payload refs `schemas/threading.schema.json#/$defs/SubThreadCreateParams`

**What `transport.schema.json` already has:** `ThreadCreateParams` includes `parent_thread_id` and `parent_message_id` as OPTIONAL fields. Sub-threading is just a `ainm.thread.create` with those fields populated.

**Fix:**
1. Remove the separate `subThreadCreateRequest` / `subThreadCreateResponse` messages from the threading channel
2. Remove the `sendSubThreadCreate` operation
3. Remove the `SubThreadCreateRequest` and `SubThreadCreateResponse` component messages
4. Update the `sendThreadCreate` operation description to mention sub-thread creation via optional `parent_thread_id`
5. Update the `ThreadCreateRequest` component message description to note it handles both root and sub-thread creation
6. The `threading.schema.json#/$defs/SubThreadCreateParams` can remain as documentation but should NOT be referenced from AsyncAPI as a separate method

**Confidence:** HIGH -- spec text is explicit.

### FINDING-02 (High): AsyncAPI stream.error schema reference

**Source of truth:** `spec/streaming.md` Section 6 defines `ainm.stream.error` parameters: `stream_id`, `code`, `message`, `partial_preserved`. The `StreamingErrorCodes` $def in `streaming.schema.json` is a catalog of error codes, NOT an event schema.

**Current state:**
- `asyncapi.yaml` line 739: `$ref: "schemas/streaming.schema.json#/$defs/StreamingErrorCodes"` -- this is wrong
- `streaming.schema.json` has `StreamingErrorCodes` as a lookup table (codes object), not an event payload

**Fix:**
1. Add a new `StreamErrorNotification` $def to `streaming.schema.json` with:
   - `stream_id` (UUIDv7, required)
   - `code` (integer, required, range -32040 to -32049)
   - `message` (string, required)
   - `partial_preserved` (boolean, required)
2. Update `asyncapi.yaml` StreamErrorNotification payload to reference `streaming.schema.json#/$defs/StreamErrorNotification`

**Confidence:** HIGH -- spec Section 6 has the exact field table.

### FINDING-03 (High): Demo `protocol_version` vs `protocolVersion`

**Source of truth:** `transport.schema.json#/$defs/InitializeParams` requires `protocolVersion` (camelCase). `spec/transport.md` line 245 shows `protocolVersion`.

**Current state:**
- `demo/src/agent.rs` line 34: `"protocol_version": "0.1.0"` (snake_case)
- `demo/src/repl.rs` line 25: `"protocol_version": "0.1.0"` (snake_case)
- `demo/src/server.rs` line 106: `"protocol_version": "0.1.0"` (snake_case)

**Fix:** Replace `protocol_version` with `protocolVersion` in all three files.

**Confidence:** HIGH -- direct field name comparison.

### FINDING-04 (High): Demo `interactive` capability vs `contentTypes`

**Source of truth:** `transport.schema.json#/$defs/ClientCapabilities` defines `contentTypes` (array of strings) and `toolVisibility` (boolean). There is NO `interactive` capability.

**Current state:**
- `demo/src/agent.rs` line 37: `"interactive": true`
- `demo/src/repl.rs` line 28: `"interactive": true`
- `demo/src/server.rs` line 109: `"interactive": true`
- `demo/src/agent.rs` line 38: `"tool_visibility": true` (should be `toolVisibility`)
- `demo/src/repl.rs` line 29: `"tool_visibility": true` (should be `toolVisibility`)
- `demo/src/server.rs` line 110: `"tool_visibility": true` (should be `toolVisibility`)

**Fix:** Replace `"interactive": true` with `"contentTypes": ["text", "code", "button", "form", "table", "approval", "action"]` and fix `"tool_visibility"` to `"toolVisibility"`.

**Confidence:** HIGH -- schema is explicit.

### FINDING-05 (High): Demo init omits signature and nonce

**Source of truth:** `transport.schema.json#/$defs/InitializeParams` requires `signature` (Ed25519Signature). `transport.schema.json#/$defs/InitializeResult` requires `nonce`.

**Current state in demo:**
- `agent.rs` and `repl.rs` ainm.initialize params: no `signature` field
- `server.rs` ainm.initialize response: no `nonce` field, no `serverInfo` object

**Fix:**
1. In agent.rs and repl.rs: generate a signature over the init nonce (or a timestamp-based init token) using the identity's Ed25519 key pair
2. In server.rs: add `nonce` and `serverInfo` to the initialize response, change structure to match `InitializeResult` schema

**Implementation note:** The signing infrastructure already exists (`signing::sign_envelope`). For init, we need a simpler sign operation -- sign a predictable string (e.g., the identity address + timestamp) and include as `signature`. The server generates a random nonce and returns it.

**Confidence:** HIGH -- schema required fields are clear.

### FINDING-06 (Medium): Demo ApprovalBlock field names

**Source of truth:** `interactive.schema.json#/$defs/ApprovalBlock` requires `title` (not `prompt`). `interactive.schema.json#/$defs/ApprovalOption` requires `value` (not `option_id`).

**Current state:**
- `demo/src/agent.rs` line 212: `"prompt": "Deploy v0.1.0 to production?"` -- should be `"title"`
- `demo/src/agent.rs` line 214-215: `"option_id": "approve"` / `"option_id": "reject"` -- should be `"value"`
- `demo/src/render.rs` line 131: reads `block.get("prompt")` -- should read `"title"`

**Fix:** Update agent.rs ApprovalBlock JSON to use `title` and `value`. Update render.rs to read `"title"` instead of `"prompt"`.

**Confidence:** HIGH -- direct field name comparison.

### FINDING-07 (Medium): Demo Agent Card `parallelStreaming` vs `parallelBlocks`

**Source of truth:** `agent-card.schema.json#/$defs/AINMStreamingCapability` requires `parallelBlocks` (boolean) and `preferredChunkSize` (string).

**Current state:**
- `demo/src/main.rs` line 67: `"parallelStreaming": false` -- should be `"parallelBlocks"`
- Missing `preferredChunkSize` field (required by schema)

**Fix:** Rename `parallelStreaming` to `parallelBlocks` and add `"preferredChunkSize": "token"`.

**Confidence:** HIGH -- schema field name and required fields are explicit.

### FINDING-08 (Medium): Demo Agent Card missing threading and memory

**Source of truth:** `agent-card.schema.json#/$defs/AINMExtensions` requires `streaming`, `threading`, and `memory` sub-objects (all required).

**Current state in `main.rs` Agent Card:**
- Has `ainm.streaming` (with wrong field name, see FINDING-07)
- Missing `ainm.threading` entirely
- Missing `ainm.memory` entirely
- Missing `ainm.protocolVersion` (required by schema)
- Missing top-level `protocolVersion` (required by AgentCard schema)

**Fix:** Add full `ainm` object with all required sub-objects:
```json
"ainm": {
    "protocolVersion": "0.1.0",
    "address": "helper@demo.ainm.local",
    "type": "agent",
    "contentTypes": ["text", "code", "button", "form", "approval"],
    "streaming": {
        "supported": true,
        "parallelBlocks": false,
        "preferredChunkSize": "token"
    },
    "threading": {
        "subThreads": false,
        "maxConcurrentThreads": 1
    },
    "memory": {
        "archivalSupported": true,
        "contextFramePreferred": true
    },
    "tools": [...]
}
```
Also add top-level `protocolVersion` to the Agent Card.

**Confidence:** HIGH -- schema required fields enumerated.

### FINDING-09 (Medium): MessageEnvelope.type enum incomplete

**Source of truth:** The demo already uses `tool.call.started`, `tool.call.progress`, `tool.call.completed`, and `action` as message types. These are valid per the spec but missing from `common.schema.json#/$defs/MessageEnvelope` type enum.

**Current enum:** `["message", "thread.create", "thread.join", "thread.leave", "presence.update"]`

**Missing values:** `tool.call.started`, `tool.call.progress`, `tool.call.completed`, `action`

**Fix:** Add the 4 missing values to the enum array.

**Confidence:** HIGH -- the types are used by the demo and defined in tool-visibility.schema.json and interactive.schema.json.

### FINDING-10 (Medium): Demo missing memory scenario

**Source of truth:** `memory.schema.json` defines `MemoryStoreParams`, `MemoryQueryParams`, `ContextFrame`. `spec/memory.md` Section 5 defines the JSON-RPC methods.

**Current state:** Demo has NO memory-related code. The agent does not handle `/memory` commands.

**Fix:** Add a `/memory` demo scenario that:
1. Agent stores a fact via `ainm.memory.store` (key: `user_preference.demo`, value: `"Prefers streaming responses"`)
2. Agent queries archival memory via `ainm.memory.query` (tier: `archival`, key prefix match)
3. Server delivers a `ainm.context.frame` notification with a ContextFrame payload

This requires:
- New `handle_memory()` function in `agent.rs`
- Server-side handler for `ainm.memory.store` and `ainm.memory.query` in `server.rs`
- REPL command `/memory` added to `repl.rs`
- Renderer for context frame output in `render.rs`

**Implementation approach:** The demo server uses in-memory broadcast channels. Memory operations can use a simple `HashMap<String, ArchivalFact>` behind a `Mutex` in ServerState. The context frame delivery can be a hardcoded example showing the 4-section structure.

**Confidence:** HIGH -- schemas and spec methods are well-defined.

### FINDING-11 (Medium): Demo missing bridge scenario

**Source of truth:** `compatibility.schema.json` defines `MCPBridgeConfig`, `MCPToolMapping`, `A2ATaskMapping`. `spec/compatibility.md` defines bridge architecture.

**Current state:** Demo has NO bridge-related code.

**Fix:** Add a `/bridge` demo scenario that shows:
1. **MCP bridge:** Simulated MCP tool call via bridge proxy. Agent demonstrates how an MCP tool (e.g., `file_read`) would be exposed through the bridge. Shows:
   - Bridge Agent Card with `ainm.bridgeType: "mcp"` and `ainm.bridgeTarget`
   - Tool call routed through bridge -> MCP server -> result back to thread
2. **A2A bridge:** Simulated A2A task delegation. Agent demonstrates how an A2A task maps to a sub-thread. Shows:
   - A2A task `submitted` -> `ainm.thread.create` (sub-thread)
   - A2A task `working` -> `ainm.presence.update` (processing)
   - A2A task `completed` -> result message in sub-thread

This requires:
- New `handle_bridge()` function in `agent.rs` that sends simulated bridge messages
- No server changes needed -- the bridge scenario uses existing message types
- REPL command `/bridge` added to `repl.rs`
- Render support for bridge-specific output in `render.rs`

**Implementation approach:** The bridge scenario is purely demonstrative -- it uses existing message types (tool events, presence updates, thread creates) with bridge-specific metadata. No actual MCP or A2A connections needed.

**Confidence:** HIGH -- schemas define the data structures, demo just needs to produce conforming JSON.

### FINDING-12 (Low): Demo sends undocumented `sequence` field

**Source of truth:** `streaming.schema.json#/$defs/StreamDelta` has 4 properties: `stream_id`, `block_index`, `delta`, `language`. There is NO `sequence` property.

**Current state:**
- `demo/src/streaming.rs` lines 73, 100: includes `"sequence": sequence` in delta payloads
- The spec does not mention sequence numbers for deltas

**Decision needed:** Either:
- (a) Add `sequence` to StreamDelta schema (useful for ordering, but not in spec)
- (b) Remove `sequence` from demo

**Recommendation:** Option (b) -- remove from demo. The spec explicitly does not include sequence numbers in deltas. Stream ordering is guaranteed by the WebSocket transport (TCP ordered delivery). Adding it to the schema would be a spec change beyond polish scope.

**Confidence:** HIGH -- the schema is explicit about what properties StreamDelta has.

## Architecture Patterns

### File Change Map

```
Plan 1: AsyncAPI + Schema fixes
  asyncapi.yaml                 -- FINDING-01 (sub_thread method), FINDING-02 (stream.error ref)
  schemas/streaming.schema.json -- FINDING-02 (add StreamErrorNotification $def)
  schemas/common.schema.json    -- FINDING-09 (MessageEnvelope.type enum)
  schemas/streaming.schema.json -- FINDING-12 (add sequence to StreamDelta OR leave as-is)
  website/static/asyncapi.yaml  -- mirror of asyncapi.yaml
  website/static/schemas/*      -- mirrors of schemas/

Plan 2: Demo conformance
  demo/src/agent.rs    -- FINDING-03, -04, -06 (field names), FINDING-10 (memory), FINDING-11 (bridge)
  demo/src/repl.rs     -- FINDING-03, -04 (field names), FINDING-10 (memory cmd), FINDING-11 (bridge cmd)
  demo/src/server.rs   -- FINDING-03, -04, -05 (init response), FINDING-10 (memory handlers)
  demo/src/main.rs     -- FINDING-07, -08 (Agent Card)
  demo/src/render.rs   -- FINDING-06 (ApprovalBlock), FINDING-10 (memory render), FINDING-11 (bridge render)
  demo/src/streaming.rs -- FINDING-12 (remove sequence field)
```

### Schema-to-Demo Conformance Pattern

For every demo JSON payload, there is a corresponding JSON Schema $def that is the source of truth:

| Demo Context | Schema Source of Truth |
|---|---|
| ainm.initialize params | `transport.schema.json#/$defs/InitializeParams` |
| ainm.initialize response | `transport.schema.json#/$defs/InitializeResult` |
| Agent Card | `agent-card.schema.json#/$defs/AgentCard` |
| ApprovalBlock | `interactive.schema.json#/$defs/ApprovalBlock` |
| StreamDelta | `streaming.schema.json#/$defs/StreamDelta` |
| MessageEnvelope | `common.schema.json#/$defs/MessageEnvelope` |
| MemoryStoreParams | `memory.schema.json#/$defs/MemoryStoreParams` |
| ContextFrame | `memory.schema.json#/$defs/ContextFrame` |

### Anti-Patterns to Avoid

- **Inventing schema fields in demo code:** Every field in demo JSON MUST exist in the corresponding schema $def. No `interactive: true`, no `protocol_version`, no `option_id`.
- **Using snake_case when schema uses camelCase:** The spec uses camelCase for all field names in schemas. Demo code must match exactly.
- **Creating separate methods for what the spec does with optional fields:** The spec handles sub-threads via optional `parent_thread_id` on `ainm.thread.create`, not a separate method.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Nonce generation | Custom nonce format | UUIDv7 (already in demo via `protocol::new_message_id()`) | Consistent with all other IDs in the protocol |
| Init signature | Complex auth protocol | Simple Ed25519 sign of identity + timestamp | `signing::sign_envelope` pattern already exists, just sign a simpler payload |
| Memory storage | Database | `HashMap<String, serde_json::Value>` behind `Arc<Mutex<>>` | Demo only -- production would use PostgreSQL |
| Bridge scenario | MCP/A2A client libraries | Hardcoded JSON messages using existing types | Demo demonstrates protocol shapes, not actual bridge implementation |

## Common Pitfalls

### Pitfall 1: Forgetting website/static mirrors

**What goes wrong:** Editing `asyncapi.yaml` and `schemas/*.json` in the repo root but forgetting to update the copies in `website/static/`.
**Why it happens:** The Docusaurus site serves from `website/static/` which contains copies (not symlinks) of the canonical files.
**How to avoid:** After every schema/AsyncAPI edit, copy the changed files to `website/static/schemas/` and `website/static/asyncapi.yaml`.
**Warning signs:** Schema links on the published site return stale content.

### Pitfall 2: Demo compilation after field changes

**What goes wrong:** Changing JSON field names in `serde_json::json!()` macros is safe (they're just strings), but changing Rust struct field names or logic that reads specific fields can break compilation.
**Why it happens:** `render.rs` reads fields by name via `block.get("prompt")` -- if the field is renamed to `title`, the code compiles but reads `None`.
**How to avoid:** Search for every usage of the old field name across all demo source files before renaming. The fields affected are: `prompt` -> `title`, `option_id` -> `value`, `protocol_version` -> `protocolVersion`, `interactive` -> `contentTypes`, `tool_visibility` -> `toolVisibility`, `parallelStreaming` -> `parallelBlocks`.
**Warning signs:** Demo runs but renders "unknown" or empty values for renamed fields.

### Pitfall 3: Agent Card schema completeness

**What goes wrong:** Fixing one Agent Card issue but missing other required fields.
**Why it happens:** The `AgentCard` schema has 8 required top-level fields and the `AINMExtensions` has 7 required fields. Easy to miss one.
**How to avoid:** Check against the full required field lists:
  - `AgentCard` required: `name`, `description`, `url`, `version`, `protocolVersion`, `capabilities`, `skills`, `ainm`
  - `AINMExtensions` required: `protocolVersion`, `address`, `contentTypes`, `streaming`, `threading`, `memory`, `tools`
  - `AINMStreamingCapability` required: `supported`, `parallelBlocks`, `preferredChunkSize`
  - `AINMThreadingCapability` required: `subThreads`, `maxConcurrentThreads`
  - `AINMMemoryCapability` required: `archivalSupported`, `contextFramePreferred`
**Warning signs:** Schema validation of the Agent Card would fail on required field checks.

### Pitfall 4: Memory/bridge scenarios breaking existing demo flow

**What goes wrong:** Adding new command handlers changes the agent's message routing and breaks existing scenarios.
**Why it happens:** The agent's message dispatch in `agent.rs` uses `if/else if` chain based on text content. New keywords can shadow existing ones.
**How to avoid:** New scenarios use `/memory` and `/bridge` commands (slash-prefixed) rather than bare keywords. The REPL handles slash commands before sending to agent, or the agent checks for these specific prefixes.
**Warning signs:** Typing "hello" triggers the memory or bridge scenario instead.

### Pitfall 5: AsyncAPI sub_thread removal breaking channel structure

**What goes wrong:** Removing sub-thread messages from the threading channel but leaving orphaned references.
**Why it happens:** AsyncAPI channels, operations, and component messages are cross-referenced.
**How to avoid:** Remove in order: (1) operation `sendSubThreadCreate`, (2) channel message refs in `channels.threading.messages`, (3) component messages `SubThreadCreateRequest` and `SubThreadCreateResponse`.
**Warning signs:** AsyncAPI CLI validation errors about dangling $refs.

## Code Examples

### StreamErrorNotification schema (new $def for FINDING-02)

```json
"StreamErrorNotification": {
    "type": "object",
    "description": "Reports a streaming error that terminates the stream abnormally. Sent as ainm.stream.error JSON-RPC notification. See streaming.md Section 6.",
    "required": ["stream_id", "code", "message", "partial_preserved"],
    "properties": {
        "stream_id": {
            "$ref": "common.schema.json#/$defs/UUIDv7",
            "description": "The stream_id of the affected stream."
        },
        "code": {
            "type": "integer",
            "description": "Error code from the streaming error range (-32040 to -32049)."
        },
        "message": {
            "type": "string",
            "description": "Human-readable error description."
        },
        "partial_preserved": {
            "type": "boolean",
            "description": "Whether partial content accumulated before the error is preserved as a permanent message."
        }
    }
}
```

### Corrected demo InitializeParams (FINDING-03, -04, -05)

```rust
let init_signature = signing::sign_init(&identity.key_pair, &identity.address)?;
let init_req = protocol::new_request(
    "ainm.initialize",
    serde_json::json!({
        "identity": identity.address,
        "protocolVersion": "0.1.0",
        "signature": init_signature,
        "capabilities": {
            "streaming": true,
            "contentTypes": ["text", "code", "button", "form", "table", "approval", "action"],
            "toolVisibility": true
        }
    }),
    serde_json::json!(1),
);
```

### Corrected server InitializeResult (FINDING-05)

```rust
let nonce = protocol::new_message_id(); // UUIDv7 as nonce
let init_response = protocol::new_response(
    req_id,
    serde_json::json!({
        "protocolVersion": "0.1.0",
        "serverInfo": {
            "name": "demo.ainm.local",
            "version": "0.1.0"
        },
        "capabilities": {
            "streaming": true,
            "subThreads": true,
            "storeAndForward": false
        },
        "nonce": nonce
    }),
);
```

### Corrected ApprovalBlock (FINDING-06)

```rust
{
    "type": "approval",
    "version": "1.0",
    "source": "agent",
    "action_id": "demo-approval",
    "title": "Deploy v0.1.0 to production?",
    "options": [
        {"value": "approve", "label": "Approve", "style": "primary"},
        {"value": "reject", "label": "Reject", "style": "danger"}
    ],
    "status": "pending",
    "fallback": "[Approval: Deploy v0.1.0 to production?]"
}
```

### Memory scenario handler sketch (FINDING-10)

```rust
async fn handle_memory(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
    // 1. Store a fact
    let store_req = protocol::new_request(
        "ainm.memory.store",
        serde_json::json!({
            "thread_id": thread_id,
            "tier": "archival",
            "fact": {
                "key": "user_preference.demo",
                "value": "Prefers streaming responses",
                "tags": ["preference", "demo"]
            }
        }),
        serde_json::json!(protocol::new_message_id()),
    );
    // send store_req...

    // 2. Query archival memory
    let query_req = protocol::new_request(
        "ainm.memory.query",
        serde_json::json!({
            "thread_id": thread_id,
            "tier": "archival",
            "key": "user_preference",
            "limit": 5
        }),
        serde_json::json!(protocol::new_message_id()),
    );
    // send query_req...

    // 3. Server delivers context frame (simulated)
    // Server sends ainm.context.frame notification with ContextFrame payload
}
```

### Bridge scenario handler sketch (FINDING-11)

```rust
async fn handle_bridge(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
    // Explain the bridge concept
    let mut intro = serde_json::json!({
        // TextBlock explaining MCP and A2A bridges
    });

    // Simulate MCP bridge: tool call via bridge proxy
    // Uses existing tool event types with bridge metadata
    let mcp_bridge_card = serde_json::json!({
        "name": "MCP Code Tools Bridge",
        "ainm": {
            "bridgeType": "mcp",
            "bridgeTarget": "mcp-code-tools://localhost:3001",
            // ... standard Agent Card fields
        }
    });

    // Simulate A2A bridge: task lifecycle
    // submitted -> thread.create, working -> presence, completed -> result
}
```

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust compiler | Demo compilation | Yes | 1.94.0 | -- |
| Cargo | Demo build | Yes | 1.94.0 | -- |
| Node.js | AsyncAPI CLI validation | Yes | v24.7.0 | -- |
| npm | AsyncAPI CLI install | Yes | 11.5.1 | -- |

**Missing dependencies with no fallback:** None.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `ainm.sub_thread.create` method | `ainm.thread.create` with `parent_thread_id` | Phase 1 spec decision | AsyncAPI must be corrected |
| `StreamingErrorCodes` as error payload | `StreamErrorNotification` event schema | Phase 2 spec (streaming.md Section 6) | New schema $def needed |
| snake_case in initialize params | camelCase per JSON Schema | Phase 3 schema formalization | Demo field names must match |

## Open Questions

1. **FINDING-12 sequence field disposition**
   - What we know: The spec does not include `sequence` in StreamDelta. The demo adds it for ordering.
   - What's clear: WebSocket guarantees message ordering (TCP). Sequence numbers are redundant.
   - Recommendation: Remove from demo. If sequence is desired later, it should be added to the spec first, then to the schema, then to the demo -- not the other way around.

2. **Memory scenario scope**
   - What we know: The spec defines store, query, evict, and context frame operations.
   - What's clear: The demo needs to exercise at least store + query + context push.
   - Recommendation: Implement store and query with server-side in-memory storage. Simulate context frame delivery. Skip evict (can be mentioned in demo output text but doesn't need interactive demonstration).

3. **Bridge scenario scope**
   - What we know: Bridges are complex multi-system integrations.
   - What's clear: The demo cannot connect to real MCP/A2A servers.
   - Recommendation: Simulate both bridge types using existing message types with bridge-specific metadata. Show the JSON shapes that a real bridge would produce. This proves the protocol can represent bridge operations without requiring external dependencies.

## Sources

### Primary (HIGH confidence)
- `spec/threading.md` Section 4.2 -- Sub-thread creation method name (ainm.thread.create with parent_thread_id)
- `spec/streaming.md` Section 6 -- StreamErrorNotification parameters
- `spec/transport.md` Section 4.3 -- InitializeParams / InitializeResult field names
- `spec/compatibility.md` Sections 3-4 -- Bridge architecture and mappings
- `spec/memory.md` Sections 3-5 -- Memory tiers and operations
- `schemas/transport.schema.json` -- InitializeParams, InitializeResult, ClientCapabilities
- `schemas/interactive.schema.json` -- ApprovalBlock (title, value fields)
- `schemas/agent-card.schema.json` -- AINMExtensions required fields
- `schemas/streaming.schema.json` -- StreamDelta properties (no sequence)
- `schemas/common.schema.json` -- MessageEnvelope.type enum
- `schemas/memory.schema.json` -- MemoryStoreParams, ContextFrame
- `schemas/compatibility.schema.json` -- MCPBridgeConfig, A2ATaskMapping
- `.planning/v3.1-MILESTONE-AUDIT.md` -- All 13 findings with severity and fix guidance

### Secondary (MEDIUM confidence)
- None needed -- all findings are internal consistency checks between spec, schema, and demo artifacts.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies. All fixes use existing Rust crate APIs and JSON Schema patterns.
- Architecture: HIGH - File change map is precise. Each finding maps to specific lines in specific files.
- Pitfalls: HIGH - Based on direct code reading and understanding of the artifact relationships.

**Research date:** 2026-03-28
**Valid until:** Indefinite (conformance fixes against stable internal artifacts)
