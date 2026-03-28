# Phase 2: Agent-Native Features - Research

**Researched:** 2026-03-28
**Domain:** Protocol specification -- streaming, interactive content, agent discovery, tool visibility, memory, MCP/A2A compatibility
**Confidence:** HIGH

## Summary

Phase 2 extends the AINM protocol specification (spec-only, no implementation code) with six feature domains that make the protocol AI-native: streaming responses, interactive content blocks, Agent Card discovery, tool-use transparency, hierarchical memory management, and MCP/A2A compatibility bridges. All output is prose specification documents extending the five Phase 1 specs (`content.md`, `transport.md`, `identity.md`, `security.md`, `threading.md`).

The research covers the external protocol landscape that informs each feature: AG-UI's streaming event model (the most mature event-based streaming spec), A2A's Agent Card schema (the industry standard for agent capability discovery), A2UI's declarative component catalog model (the reference for safe agent-driven UI), Letta/MemGPT's three-tier memory hierarchy (the only mature agent memory architecture), and MCP's tool listing/calling format (the universal agent-tool standard). The CONTEXT.md decisions lock specific approaches for each domain, so this research focuses on deep investigation of those approaches rather than exploring alternatives.

**Primary recommendation:** Structure Phase 2 as six spec documents (one per feature domain), each following the established Phase 1 patterns: RFC-keyword prose with inline JSON examples, `ainm.*` JSON-RPC method naming, content block base schema inheritance, and mandatory fallback on all new block types.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Token-level deltas -- each delta carries 1-few tokens as generated. Lowest latency, most responsive UX. Aligns with AG-UI and OpenAI streaming patterns.
- **D-02:** Client-initiated cancellation -- client sends explicit cancel event (e.g., `ainm.stream.cancel`). Server/agent acknowledges and stops. Partial message preserved with `cancelled` status. No server-initiated or bidirectional cancel.
- **D-03:** Parallel multi-block streaming -- agents CAN stream into multiple content blocks simultaneously (e.g., text explanation + code). Stream-start event declares the full block layout upfront (types + positions). Clients render placeholders for all declared blocks immediately.
- **D-04:** Block index addressing in deltas -- delta events reference blocks by their index in the declared layout (from stream-start). Simple addressing, no separate block IDs needed for streaming.
- **D-05:** Core interactive block types (all clients MUST support): ButtonBlock, FormBlock, TableBlock, ApprovalBlock. These join the existing core set (TextBlock, CodeBlock, ImageBlock, FileBlock) from Phase 1.
- **D-06:** ChartBlock deferred -- not in v1. The 8 core block types (4 static + 4 interactive) are sufficient.
- **D-07:** Action events flow as thread messages -- when a user clicks a button or submits a form, client sends a new message in the thread with an ActionBlock containing the block reference + user input. Aligns with thread-addressed model (Phase 1 D-11). All participants see actions.
- **D-08:** Protocol-defined component catalog -- the spec defines exactly what fields each interactive block type has. Agents construct blocks from the spec, clients render from the spec. No runtime catalog discovery. The spec IS the catalog. Follows A2UI approach.
- **D-09:** Superset of A2A -- AINM Agent Cards include all A2A required fields (name, description, url, capabilities) PLUS AINM-specific extensions. A2A clients can read the card; AINM clients get richer info.
- **D-10:** Well-known URL + ETag caching -- Agent Card lives at the `agent_card_url` from identity resolution (Phase 1 D-06). Standard HTTP caching with ETag/If-None-Match. No new infrastructure.
- **D-11:** AINM-specific capability fields in Agent Card: supported content block types, streaming support & preferences, threading preferences, memory tier support. All four declared by user.
- **D-12:** Tool list in Agent Card -- card includes a tools array listing available tools with name, description, and input schema. Aligns with MCP tool listing and A2A skills field.
- **D-13:** Server-managed with participant hints -- server maintains all three tiers (active context, conversation memory, archival). Participants can REQUEST memory operations (store, query, evict) but server enforces policy (retention, size, permissions). Server is authoritative.
- **D-14:** Context frame on thread join -- when an agent joins a thread or is @-mentioned, server delivers a structured context frame: active context (recent messages), conversation summary, relevant archival facts, participant list + capabilities. One payload to bootstrap the agent.
- **D-15:** Archival memory stores structured facts + user preferences -- key-value facts, cross-conversation knowledge. Agents store and query structured data. Server enforces schema.
- **D-16:** Conversation memory shared, archival per-participant -- active context and conversation memory are shared across all thread participants. Archival memory is per-participant.

### Claude's Discretion
- Tool visibility event schema (field names, progress granularity, collapsible rendering hints)
- MCP/A2A bridge design (thin translation vs deep integration, proxy participant behavior)
- Streaming error handling mid-stream (error event format, partial message state on errors)
- Streaming collapse semantics (how deltas consolidate into permanent events)
- Form field types within FormBlock
- Approval states and transitions within ApprovalBlock
- Table capabilities within TableBlock (sorting, filtering, pagination)
- Context frame schema structure
- Memory query protocol methods (method names, query syntax, response format)
- Memory retention and eviction policies

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STRM-01 | Agents can stream text responses token-by-token in real-time | AG-UI TEXT_MESSAGE_START/CONTENT/END pattern; D-01 token-level deltas |
| STRM-02 | Protocol defines streaming lifecycle events (stream-start, content-delta, stream-end) | AG-UI RunStarted/RunFinished lifecycle + Start/Content/End triplet pattern |
| STRM-03 | Streaming messages collapse into permanent events upon completion | AG-UI MessagesSnapshot pattern; collapse semantics research below |
| STRM-04 | Clients can receive and render partial messages during streaming | D-03 parallel multi-block with upfront layout declaration; D-04 block index addressing |
| CONT-03 | Protocol defines interactive block types: ButtonBlock, FormBlock, TableBlock, ApprovalBlock | A2UI component catalog model; D-05 four interactive types; D-08 spec-as-catalog |
| CONT-04 | Interactive blocks use declarative component model (UI as data, not code) | A2UI security model -- catalog-based, no code execution; content.md base schema pattern |
| CONT-05 | No arbitrary payload size limits at the protocol level | Transport spec already has `maxContentBlocks` and `maxMessageSize` in capability negotiation |
| DISC-01 | Agents publish capability cards (Agent Cards) as JSON at a well-known endpoint | A2A Agent Card at `/.well-known/agent-card.json`; D-09 superset approach |
| DISC-02 | Agent Cards are discoverable by resolving the `@provider` portion of an agent's identity | Identity spec `agent_card_url` field (Section 4.2) already provides the hook |
| DISC-03 | Agent Cards include: name, description, supported content types, streaming capabilities, threading preferences, tools available | A2A fields + D-11 AINM extensions + D-12 tools array |
| DISC-04 | Agent Cards are compatible with A2A Agent Card format (extend, don't reinvent) | A2A AgentCard schema research: name, description, url, capabilities, skills, securitySchemes |
| DISC-05 | Agent Cards are cacheable with explicit versioning | D-10 ETag/If-None-Match HTTP caching |
| TOOL-01 | Protocol defines ToolCallStarted event (tool name, description) | AG-UI ToolCallStart event pattern (toolCallId, toolCallName, parentMessageId) |
| TOOL-02 | Protocol defines ToolCallProgress event (intermediate results) | AG-UI ToolCallArgs delta pattern; MCP streaming tool results |
| TOOL-03 | Protocol defines ToolCallCompleted event (result summary) | AG-UI ToolCallEnd + ToolCallResult pattern |
| TOOL-04 | Tool events are visible to all thread participants | Thread-addressed delivery model from Phase 1 threading.md |
| TOOL-05 | Clients render tool events as collapsible activity indicators | AG-UI StepStarted/StepFinished pattern; rendering hints in discretion area |
| MEMO-01 | Protocol defines three memory tiers: active context, conversation memory, archival memory | Letta/MemGPT: core memory (in-context), recall memory (searchable history), archival memory (external DB) |
| MEMO-02 | Active context includes current thread + recent messages, always available to agents | Letta core memory blocks always in context window; D-14 context frame |
| MEMO-03 | Conversation memory is the full thread history, queryable by participants | Letta recall memory with conversation_search tool |
| MEMO-04 | Archival memory stores cross-conversation knowledge, user preferences, long-term facts | Letta archival memory with archival_memory_insert/search; D-15 structured facts |
| MEMO-05 | Agents receive structured context payloads, not raw message dumps | D-14 context frame: active context + summary + archival facts + participant list |
| MEMO-06 | Memory is conversation-scoped and shared across all participants in a thread | D-16: active + conversation shared; archival per-participant |
| COMP-01 | Protocol defines mapping from MCP tool calls to protocol message types | MCP tools/list and tools/call JSON-RPC format maps to AINM tool events |
| COMP-02 | An MCP server can be exposed as an agent participant in a thread | Proxy participant design using bridge adapter pattern |
| COMP-03 | Protocol defines mapping from A2A task lifecycle to thread message events | A2A task states (submitted, working, input-required, completed, failed, canceled) map to thread events |
| COMP-04 | A2A task updates can be rendered as thread messages | A2A task state transitions map to system messages with content blocks |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **Protocol-first**: Spec must be complete and implementable before building the iOS app
- **Open standard**: Protocol spec and reference implementations are fully open source
- **Federation**: No single point of control
- **Security**: Cryptographic identity and message signing are non-negotiable from day one
- **GSD Workflow**: All changes through GSD commands
- **Spec format**: RFC-keyword prose + JSON Schema + AsyncAPI
- **Wire format**: JSON-RPC 2.0 over WebSocket/HTTP
- **Content blocks**: Typed with mandatory fallback, closed core + namespaced extensions

## Architecture Patterns

### Recommended Spec File Structure

Phase 2 produces new spec documents that extend the Phase 1 kernel. Based on the six feature domains and their natural boundaries:

```
spec/
  identity.md          # Phase 1 (unchanged -- agent_card_url already defined)
  security.md          # Phase 1 (unchanged -- signing covers all new event types)
  content.md           # Phase 1 (unchanged -- base schema inherited by new blocks)
  transport.md         # Phase 1 (EXTENDED -- new capability flags, new methods)
  threading.md         # Phase 1 (EXTENDED -- new event types for tool/memory)
  streaming.md         # NEW -- streaming lifecycle, deltas, collapse, cancellation
  interactive.md       # NEW -- ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock
  agent-card.md        # NEW -- Agent Card schema, discovery, AINM extensions
  tool-visibility.md   # NEW -- ToolCallStarted/Progress/Completed events
  memory.md            # NEW -- three-tier memory, context frames, query methods
  compatibility.md     # NEW -- MCP bridge, A2A bridge, mapping tables
```

This structure keeps each new feature domain in its own spec file (matching the modular approach from Phase 1 D-01) while noting which Phase 1 files need minor extensions (new capability flags in transport.md, new event types referenced from threading.md).

### Pattern 1: Streaming Event Lifecycle (Start-Delta-End Triplet)

**What:** Every streaming operation follows a three-phase lifecycle: a start event declares intent and layout, delta events carry incremental content, an end event signals completion and triggers collapse.

**When to use:** All streaming agent responses.

**Design based on AG-UI and D-01 through D-04:**

```json
// 1. Stream Start -- declares block layout upfront (D-03)
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.start",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "message_id": "019580b0-bbbb-7000-8000-000000000002",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "blocks": [
      { "type": "text", "version": "1.0", "source": "agent" },
      { "type": "code", "version": "1.0", "source": "agent" }
    ]
  }
}

// 2. Content Delta -- references block by index (D-04)
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 0,
    "delta": "Here is a"
  }
}

// 3. Stream End -- triggers collapse into permanent message (STRM-03)
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.end",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "status": "completed"
  }
}
```

**Collapse semantics (Claude's discretion recommendation):** On `ainm.stream.end` with `status: "completed"`, the server assembles accumulated deltas into a permanent signed message envelope (same format as `ainm.message.deliver`). The `stream_id` is preserved as metadata on the final message for audit trail. Clients replace the streaming placeholder with the permanent message. For `status: "cancelled"`, the partial content is preserved with a `cancelled` flag on the message.

**Streaming errors (Claude's discretion recommendation):** Mid-stream errors use a dedicated event:

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.error",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "code": -32040,
    "message": "Agent encountered an error during generation",
    "partial_preserved": true
  }
}
```

Error codes for streaming should use the `-32040` to `-32049` range.

### Pattern 2: Interactive Content Block (Declarative UI as Data)

**What:** Interactive blocks extend the content.md base schema (type, version, source, fallback) with type-specific fields. The spec defines exactly what fields exist -- no runtime discovery. Follows A2UI's catalog-based security model.

**When to use:** All four interactive block types + the ActionBlock for user responses.

```json
// ButtonBlock example
{
  "type": "button",
  "version": "1.0",
  "source": "agent",
  "label": "Deploy to Production",
  "action_id": "deploy-prod-001",
  "style": "primary",
  "confirmation": "Are you sure you want to deploy to production?",
  "fallback": "[Button: Deploy to Production]"
}

// ActionBlock -- user response as thread message (D-07)
{
  "type": "action",
  "version": "1.0",
  "source": "user",
  "ref_message_id": "019580b1-cccc-7000-8000-000000000003",
  "ref_block_index": 0,
  "action_id": "deploy-prod-001",
  "data": {},
  "fallback": "[Action: clicked 'Deploy to Production']"
}
```

### Pattern 3: Thread-Scoped Events (Tool Visibility and Memory Operations)

**What:** Tool calls and memory operations are thread events -- they appear in the thread's message stream as signed messages with specialized `type` values. All participants see them. This follows the Phase 1 pattern where `thread.join` and `thread.leave` are thread events.

**When to use:** Tool call lifecycle events, memory operation events.

```json
// ToolCallStarted as a thread event
{
  "id": "019580c0-dddd-7000-8000-000000000004",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:00.000Z",
  "type": "tool.call.started",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Searching codebase for authentication patterns",
      "fallback": "Searching codebase for authentication patterns"
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee",
    "name": "code_search",
    "description": "Search codebase for patterns",
    "input": { "query": "authentication middleware", "scope": "src/" }
  },
  "signature": "<agent-signature>"
}
```

### Anti-Patterns to Avoid

- **Separate streaming transport:** Do NOT define a separate WebSocket or SSE endpoint for streaming. Streaming events flow over the existing `ainm.*` JSON-RPC WebSocket connection. The `streaming` capability flag in transport.md already supports this.
- **Opaque tool calls:** Do NOT allow agents to make tool calls that are invisible to other participants. All tool events are thread messages, not private side-channels. This is the core transparency principle.
- **Raw message dumps for memory:** Do NOT send agents unstructured arrays of previous messages. Context frames (D-14) deliver structured payloads: summary, facts, participant list.
- **Mutable message envelopes:** Do NOT modify signed messages after delivery. Streaming collapses into a NEW signed message. The stream deltas are ephemeral; the collapsed message is the permanent record.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Agent Card format | Custom discovery JSON | A2A Agent Card schema as base, extend with AINM fields | A2A has 50+ partners, tools expect this format. D-09 says superset. |
| Component catalog discovery | Runtime component negotiation | Static spec-defined catalog (A2UI approach) | No moving parts. The spec IS the catalog. Prevents code injection. |
| Tool call format | Custom tool invocation schema | MCP tool definition format (name, description, inputSchema) | MCP is the universal standard. D-12 aligns Agent Card tools with MCP. |
| Streaming event model | Custom streaming protocol | AG-UI Start/Delta/End triplet pattern | Proven in 2M+ interactions/week at CopilotKit. Adapted for AINM's multi-block model. |
| Memory architecture | Custom memory system | Letta/MemGPT three-tier model (core/recall/archival) | Only mature agent memory architecture. Adapted for multi-participant protocol context. |
| Incremental state updates | Custom diff format | JSON Patch (RFC 6902) for tool progress and state deltas | Standard, well-tooled, used by AG-UI for STATE_DELTA. |
| JSON deterministic serialization | Custom canonicalization | RFC 8785 (JCS) already used in Phase 1 signing | Already in the spec. All new events inherit the same signing procedure. |

## Common Pitfalls

### Pitfall 1: Streaming Block Layout Changes Mid-Stream

**What goes wrong:** Agent declares 2 blocks in stream-start, then tries to add a 3rd block mid-stream. Clients have already rendered placeholders for only 2 blocks.
**Why it happens:** LLM responses are unpredictable. The agent might decide mid-generation to add a code block it did not originally plan.
**How to avoid:** The spec MUST state that block layout declared in `ainm.stream.start` is immutable for the duration of that stream. If an agent needs additional blocks, it must end the current stream and start a new one, or send a separate follow-up message.
**Warning signs:** Stream delta events referencing block indices that exceed the declared layout.

### Pitfall 2: Signing Ephemeral Streaming Events

**What goes wrong:** Requiring Ed25519 signatures on every streaming delta event (potentially hundreds per second) creates prohibitive CPU overhead and latency.
**Why it happens:** Phase 1 says "every message is signed." Streaming deltas are technically messages.
**How to avoid:** Define a clear distinction between ephemeral events (stream deltas, presence updates) and persistent events (collapsed messages, tool call events). Only persistent events require full signatures. Streaming deltas are authenticated by the session (the WebSocket connection was authenticated during `ainm.initialize`). The collapsed message at stream-end IS signed.
**Warning signs:** Performance benchmarks showing signing overhead dominates streaming latency.

### Pitfall 3: Memory Context Explosion

**What goes wrong:** Context frames delivered to agents on thread-join become enormous for long-lived threads with many archival facts.
**Why it happens:** No size limits on context assembly. Server tries to include "everything relevant."
**How to avoid:** Define explicit size budgets in the memory spec. The server enforces a `max_context_frame_tokens` limit (configurable per server). Context assembly prioritizes: (1) most recent messages, (2) conversation summary, (3) highest-relevance archival facts. The agent can query for more via `ainm.memory.query`.
**Warning signs:** Context frame payloads exceeding 100KB.

### Pitfall 4: ActionBlock Replay Attacks

**What goes wrong:** A malicious participant replays an ActionBlock (e.g., re-clicking an approval button) by resending a previously valid signed message.
**Why it happens:** ActionBlocks reference a `ref_message_id` and `action_id`, which could be reused.
**How to avoid:** ActionBlock messages have unique `id` fields (UUIDv7) and `timestamp`. Servers MUST deduplicate by message `id`. ApprovalBlocks should track state transitions (pending -> approved/rejected) and reject actions on already-resolved approvals.
**Warning signs:** Duplicate `action_id` events for the same `ref_message_id` from the same participant.

### Pitfall 5: A2A Compatibility Breaking on Extension Fields

**What goes wrong:** AINM-specific fields in the Agent Card break A2A clients that strictly validate the schema.
**Why it happens:** JSON Schema `additionalProperties: false` in strict A2A validators rejects unknown fields.
**How to avoid:** AINM extensions MUST use a namespaced top-level field (e.g., `"ainm"` object) that A2A validators skip as an additional property. The A2A spec allows additional properties at the top level. Verify by testing AINM Agent Cards against A2A's reference Python client.
**Warning signs:** A2A client libraries throwing validation errors on AINM Agent Cards.

### Pitfall 6: Memory Tier Confusion (Shared vs Per-Participant)

**What goes wrong:** Agent stores a private fact in archival memory expecting it to be shared, but other agents cannot see it because archival memory is per-participant (D-16).
**Why it happens:** The distinction between shared memory tiers (active context, conversation memory) and per-participant memory (archival) is subtle.
**How to avoid:** The memory spec MUST clearly document which tiers are shared and which are private. The context frame (D-14) only includes the joining agent's own archival facts. To share archival knowledge, an agent posts a message to the thread (making it part of shared conversation memory).
**Warning signs:** Agents querying archival memory for facts stored by other agents and getting empty results.

## Code Examples

### Interactive Block Types

#### ButtonBlock

```json
{
  "type": "button",
  "version": "1.0",
  "source": "agent",
  "label": "Approve Deployment",
  "action_id": "approve-deploy-v2.1",
  "style": "primary",
  "disabled": false,
  "confirmation": null,
  "fallback": "[Button: Approve Deployment]"
}
```

Fields: `label` (string, REQUIRED), `action_id` (string, REQUIRED -- unique within the message), `style` (string, OPTIONAL -- `"primary"`, `"secondary"`, `"danger"`), `disabled` (boolean, OPTIONAL -- default false), `confirmation` (string, OPTIONAL -- prompt shown before action).

#### FormBlock

Recommended form field types (Claude's discretion):

```json
{
  "type": "form",
  "version": "1.0",
  "source": "agent",
  "action_id": "submit-config-form",
  "title": "Deployment Configuration",
  "fields": [
    {
      "name": "environment",
      "label": "Target Environment",
      "field_type": "select",
      "options": ["staging", "production"],
      "required": true
    },
    {
      "name": "version",
      "label": "Version Tag",
      "field_type": "text",
      "placeholder": "e.g., v2.1.0",
      "required": true,
      "validation": "^v\\d+\\.\\d+\\.\\d+$"
    },
    {
      "name": "notify_team",
      "label": "Notify team on deploy",
      "field_type": "checkbox",
      "default": true,
      "required": false
    }
  ],
  "submit_label": "Deploy",
  "fallback": "[Form: Deployment Configuration - fields: environment, version, notify_team]"
}
```

Recommended field types: `text`, `textarea`, `number`, `select`, `multiselect`, `checkbox`, `date`, `datetime`. Each field has: `name`, `label`, `field_type`, `required`, `default`, `placeholder`, `validation` (regex), `options` (for select types).

#### TableBlock

Recommended capabilities (Claude's discretion):

```json
{
  "type": "table",
  "version": "1.0",
  "source": "agent",
  "title": "Test Results",
  "columns": [
    { "key": "test", "label": "Test Name", "sortable": true },
    { "key": "status", "label": "Status", "sortable": true },
    { "key": "duration", "label": "Duration (ms)", "sortable": true, "align": "right" }
  ],
  "rows": [
    { "test": "auth_login", "status": "passed", "duration": 142 },
    { "test": "auth_logout", "status": "passed", "duration": 89 },
    { "test": "auth_refresh", "status": "failed", "duration": 3012 }
  ],
  "row_actions": [
    { "action_id": "view-details", "label": "View", "style": "secondary" }
  ],
  "fallback": "Test Results:\n| Test | Status | Duration |\n|------|--------|----------|\n| auth_login | passed | 142ms |\n| auth_logout | passed | 89ms |\n| auth_refresh | failed | 3012ms |"
}
```

Table capabilities: `sortable` (per-column client-side sort), `row_actions` (action buttons per row), `align` (per-column alignment). Pagination and server-side filtering deferred to future versions -- tables in v1 carry all rows inline.

#### ApprovalBlock

Recommended states and transitions (Claude's discretion):

```json
{
  "type": "approval",
  "version": "1.0",
  "source": "agent",
  "action_id": "approve-pr-merge",
  "title": "Merge Pull Request #42",
  "description": "Changes: refactor auth middleware, add rate limiting",
  "status": "pending",
  "options": [
    { "value": "approve", "label": "Approve", "style": "primary" },
    { "value": "reject", "label": "Reject", "style": "danger" },
    { "value": "request_changes", "label": "Request Changes", "style": "secondary" }
  ],
  "fallback": "[Approval: Merge Pull Request #42 - Status: pending]"
}
```

Approval states: `pending` (initial), `approved`, `rejected`, `changes_requested`. State transitions happen via ActionBlock messages. Once resolved (non-pending), the ApprovalBlock is immutable -- subsequent actions on a resolved approval are rejected by the server.

### Agent Card Schema (AINM Superset of A2A)

```json
{
  "name": "Code Assistant",
  "description": "AI-powered code review and generation agent",
  "url": "https://agents.example.com",
  "version": "1.0.0",
  "protocolVersion": "0.1.0",

  "capabilities": {
    "streaming": true,
    "pushNotifications": false
  },

  "skills": [
    {
      "id": "code-review",
      "name": "Code Review",
      "description": "Analyzes code for bugs, security issues, and style",
      "tags": ["code", "review", "security"]
    }
  ],

  "defaultInputModes": ["text/plain", "application/json"],
  "defaultOutputModes": ["text/plain", "application/json"],

  "ainm": {
    "contentTypes": ["text", "code", "button", "table", "approval"],
    "streaming": {
      "supported": true,
      "parallelBlocks": true,
      "preferredChunkSize": "token"
    },
    "threading": {
      "subThreads": true,
      "maxConcurrentThreads": 10
    },
    "memory": {
      "archivalSupported": true,
      "contextFramePreferred": true
    },
    "tools": [
      {
        "name": "code_search",
        "description": "Search codebase for patterns and symbols",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": { "type": "string" },
            "scope": { "type": "string" }
          },
          "required": ["query"]
        }
      }
    ]
  }
}
```

The top-level fields (`name`, `description`, `url`, `version`, `capabilities`, `skills`, `defaultInputModes`, `defaultOutputModes`) match the A2A Agent Card spec. The `ainm` object contains all AINM-specific extensions, namespaced to avoid breaking A2A validators.

### Tool Visibility Event Lifecycle

Recommended event schema (Claude's discretion):

```json
// Message type: "tool.call.started" (TOOL-01)
{
  "type": "tool.call.started",
  "content": [{
    "type": "text", "version": "1.0", "source": "agent",
    "text": "Searching codebase...",
    "fallback": "Searching codebase..."
  }],
  "tool": {
    "call_id": "tc-019580c0-eeee",
    "name": "code_search",
    "description": "Search codebase for patterns",
    "input": { "query": "auth middleware" }
  }
}

// Message type: "tool.call.progress" (TOOL-02)
{
  "type": "tool.call.progress",
  "content": [{
    "type": "text", "version": "1.0", "source": "agent",
    "text": "Found 12 matches, analyzing...",
    "fallback": "Found 12 matches, analyzing..."
  }],
  "tool": {
    "call_id": "tc-019580c0-eeee",
    "progress": { "current": 12, "total": null, "detail": "Analyzing matches" }
  }
}

// Message type: "tool.call.completed" (TOOL-03)
{
  "type": "tool.call.completed",
  "content": [{
    "type": "text", "version": "1.0", "source": "agent",
    "text": "Code search complete: 12 results in 3 files",
    "fallback": "Code search complete: 12 results in 3 files"
  }],
  "tool": {
    "call_id": "tc-019580c0-eeee",
    "name": "code_search",
    "result_summary": "12 results in 3 files",
    "duration_ms": 1420,
    "status": "success"
  }
}
```

Rendering hint: clients SHOULD render tool events as collapsible UI elements. Default state: collapsed showing tool name + status. Expanded state: shows input, progress, and result. The `content` array provides fallback text for clients that do not support tool event rendering.

### Memory Context Frame

```json
// Delivered as part of ainm.thread.join response or ainm.context.frame notification
{
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "frame": {
    "active_context": {
      "recent_messages": [
        {
          "id": "019580a5-1111-7000-8000-aaaaaaaaaaaa",
          "from": "alice@messaging.example.com",
          "timestamp": "2026-03-28T12:39:00.000Z",
          "summary": "Asked about authentication patterns in the codebase"
        }
      ],
      "window_start": "2026-03-28T12:30:00.000Z",
      "message_count": 15
    },
    "conversation_summary": "Discussion about refactoring the authentication middleware. Alice asked for code review. Bob suggested using JWT refresh tokens. The team is evaluating two approaches.",
    "archival_facts": [
      {
        "key": "user_preference.code_style",
        "value": "Prefer functional patterns over OOP",
        "stored_by": "code-bot@agents.example.com",
        "stored_at": "2026-03-25T10:00:00.000Z"
      }
    ],
    "participants": [
      {
        "identity": "alice@messaging.example.com",
        "type": "human",
        "presence": "online"
      },
      {
        "identity": "bob@messaging.example.com",
        "type": "human",
        "presence": "away"
      }
    ]
  }
}
```

### Memory JSON-RPC Methods

Recommended method design (Claude's discretion):

| Method | Direction | Purpose |
|--------|-----------|---------|
| `ainm.memory.store` | client -> server (request) | Store a fact in archival memory |
| `ainm.memory.query` | client -> server (request) | Query conversation or archival memory |
| `ainm.memory.evict` | client -> server (request) | Request removal of an archival fact |
| `ainm.context.frame` | server -> client (notification) | Deliver context frame on join/@-mention |

```json
// ainm.memory.store -- agent stores a fact
{
  "jsonrpc": "2.0",
  "id": 100,
  "method": "ainm.memory.store",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "fact": {
      "key": "user_preference.timezone",
      "value": "Europe/Madrid",
      "tags": ["preference", "locale"]
    }
  }
}

// ainm.memory.query -- agent queries conversation memory
{
  "jsonrpc": "2.0",
  "id": 101,
  "method": "ainm.memory.query",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "conversation",
    "query": "authentication discussion",
    "limit": 10
  }
}
```

### MCP Bridge Mapping

```
MCP tools/list          -> Agent Card ainm.tools array (static mapping)
MCP tools/call          -> ainm.message.send with tool.call.started event
MCP tool result         -> ainm.message.send with tool.call.completed event
MCP notifications/tools/list_changed -> Agent Card ETag update
```

The MCP bridge is a **proxy participant** pattern: an AINM server component that (1) connects to an MCP server as a client, (2) exposes the MCP server's tools in an Agent Card, and (3) translates between AINM thread messages and MCP tool calls. The proxy has its own AINM identity (e.g., `mcp-bridge@server.example.com`).

### A2A Bridge Mapping

```
A2A task submitted      -> ainm.thread.create (new sub-thread for the task)
A2A task working        -> ainm.presence.update status: "processing"
A2A task input-required -> ainm.message.send with FormBlock or ApprovalBlock
A2A task completed      -> ainm.message.send with result content blocks
A2A task failed         -> ainm.message.send with error content block
A2A task canceled       -> ainm.stream.cancel / thread leave
```

The A2A bridge maps A2A task lifecycle to AINM thread lifecycle. Each A2A task becomes a sub-thread (or is managed within an existing thread). A2A task state transitions become presence updates or messages in the thread.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| MCP Apps (sandboxed HTML iframes) | A2UI (declarative JSON components) | Dec 2025 / Jan 2026 | UI as data replaces UI as code for agent-driven interfaces |
| ACP (IBM) separate from A2A | ACP merged into A2A under Linux Foundation | Aug 2025 | A2A is the convergence point for agent-to-agent communication |
| Per-agent memory (Letta v1) | Multi-agent shared memory (Letta v2 + AINM novelty) | 2025-2026 | AINM's shared conversation memory + per-participant archival is novel |
| OpenAI function calling format | MCP tools/list + tools/call | 2024-2026 | MCP is the universal standard, 97M+ monthly SDK downloads |
| A2A v0.1 (HTTP-only) | A2A v0.3 (+ gRPC, signed security cards) | 2025-2026 | A2A Agent Cards now include security and authentication fields |
| REST API discovery | Well-known URL Agent Cards | 2025-2026 | `/.well-known/agent-card.json` is the industry convergence |

**Deprecated/outdated:**
- **ACP (Agent Communication Protocol):** Merged into A2A. Do not reference ACP independently.
- **A2UI v0.8 component nesting:** v0.9 moved to flat component lists with ID references. AINM should follow the flat model.
- **MCP HTTP+SSE transport:** Replaced by Streamable HTTP in 2025. Not relevant for AINM (we use WebSocket), but worth noting for compatibility discussion.

## Open Questions

1. **Streaming delta granularity for non-text blocks**
   - What we know: Text deltas carry token fragments (D-01). Code block deltas work the same way (token-level text).
   - What is unclear: How do streaming deltas work for structured blocks like TableBlock? Row-by-row? Cell-by-cell?
   - Recommendation: v1 restricts streaming to text-compatible blocks (TextBlock, CodeBlock). Interactive blocks (ButtonBlock, FormBlock, TableBlock, ApprovalBlock) are delivered complete, not streamed. This avoids complex partial-render semantics.

2. **Context frame size budgeting**
   - What we know: D-14 says server delivers a structured context frame. Letta uses configurable memory block sizes.
   - What is unclear: What is a reasonable default size budget? How does the server prioritize what to include?
   - Recommendation: Define `max_context_frame_size` as a server-configurable parameter (recommended default: 32KB). Priority order: recent messages (50%), conversation summary (20%), archival facts (20%), participant list (10%). Exact algorithm is implementation-defined.

3. **Memory query result format**
   - What we know: D-13 says server is authoritative. Agents request; server enforces policy.
   - What is unclear: Should memory query results be a JSON-RPC response or delivered as a thread notification?
   - Recommendation: JSON-RPC request/response (not a thread notification). Memory queries are private operations -- the agent asking "what do I know about this user?" should not be visible to all participants. Only explicit memory stores that affect shared tiers generate thread-visible events.

4. **A2A Agent Card protocolVersion alignment**
   - What we know: A2A uses `protocolVersion` field. AINM has its own protocol version.
   - What is unclear: Should AINM Agent Cards include both A2A `protocolVersion` and AINM protocol version?
   - Recommendation: Include A2A `protocolVersion` at the top level (for A2A compatibility). Include AINM protocol version inside the `ainm` extension object. Both versions are independent.

## New Message Types for Phase 2

The Phase 1 `security.md` Section 3.3 defines five message types. Phase 2 adds:

| Type Value | Domain | Description |
|-----------|--------|-------------|
| `tool.call.started` | Tool Visibility | Agent began a tool invocation |
| `tool.call.progress` | Tool Visibility | Intermediate tool execution update |
| `tool.call.completed` | Tool Visibility | Tool invocation finished |
| `action` | Interactive Content | User interacted with an interactive block |

Streaming events (`ainm.stream.start`, `ainm.stream.delta`, `ainm.stream.end`, `ainm.stream.cancel`, `ainm.stream.error`) are JSON-RPC notifications on the WebSocket, NOT message types. They are ephemeral transport events, not persistent signed messages. The collapsed result of streaming becomes a `message` type.

Memory operations (`ainm.memory.store`, `ainm.memory.query`, `ainm.memory.evict`, `ainm.context.frame`) are JSON-RPC methods, NOT message types. They are protocol operations between a participant and the server, not thread messages.

## New Error Code Ranges for Phase 2

Extending the Phase 1 error code scheme:

| Range | Domain | Codes |
|-------|--------|-------|
| `-32040` to `-32049` | Streaming | Stream not found, invalid block index, stream already ended |
| `-32050` to `-32059` | Memory | Memory tier unavailable, quota exceeded, invalid query, fact not found |
| `-32060` to `-32069` | Agent Card | Card not found, invalid card format, version mismatch |
| `-32070` to `-32079` | Interactive | Invalid action, approval already resolved, form validation failed |

## New Capability Flags for Transport Initialization

The `ainm.initialize` handshake capabilities object (transport.md Section 4.3) needs extension:

```json
{
  "capabilities": {
    "streaming": true,
    "subThreads": true,
    "contentTypes": ["text", "code", "image", "file", "button", "form", "table", "approval", "action"],
    "toolVisibility": true,
    "memory": {
      "conversation": true,
      "archival": true
    },
    "parallelStreaming": true
  }
}
```

New flags: `toolVisibility` (client supports rendering tool events), `memory` (server supports memory tiers), `parallelStreaming` (client supports multi-block streaming), extended `contentTypes` to include interactive block types.

## Sources

### Primary (HIGH confidence)
- Phase 1 spec files (`spec/content.md`, `spec/transport.md`, `spec/identity.md`, `spec/security.md`, `spec/threading.md`) -- existing patterns, schemas, and conventions that Phase 2 extends
- `research/landscape-protocols-projects.md` -- project's own landscape research covering AG-UI, A2A, A2UI, Letta, MCP
- AG-UI events specification at `docs.ag-ui.com/concepts/events` -- all streaming event types (TextMessageStart/Content/End, ToolCallStart/Args/End, RunStarted/Finished, StateSnapshot/Delta)
- A2A Agent Card specification at `a2a-protocol.org/latest/specification/` -- AgentCard object structure, capabilities, skills, task lifecycle states
- MCP tools specification at `modelcontextprotocol.io/specification/draft/server/tools` -- Tool definition (name, description, inputSchema, outputSchema), tools/list, tools/call

### Secondary (MEDIUM confidence)
- A2UI introduction at `a2ui.org` -- declarative component model, catalog-based security, flat component lists with ID references
- Letta/MemGPT blog at `letta.com/blog/agent-memory` -- three-tier memory architecture (core/recall/archival), memory management tools, context assembly

### Tertiary (LOW confidence)
- JSON-RPC 3.0 streaming proposal (GitHub) -- parallel stream support concepts, transport-agnostic streaming. Early/unofficial, but validates the multi-stream approach.

## Metadata

**Confidence breakdown:**
- Streaming architecture: HIGH -- AG-UI provides battle-tested patterns (2M+ interactions/week); AINM adaptations (parallel multi-block, block index addressing) are novel but well-grounded in locked decisions
- Interactive content blocks: HIGH -- A2UI provides the security model (catalog-based, UI as data); specific block schemas are Claude's discretion but follow established patterns
- Agent Card discovery: HIGH -- A2A spec is the industry standard; AINM extends rather than reinvents
- Tool visibility: HIGH -- AG-UI ToolCallStart/Args/End + MCP tool format are well-established; adaptation to thread events follows Phase 1 patterns
- Memory architecture: MEDIUM -- Letta/MemGPT provides the conceptual model, but adapting per-agent memory to multi-participant protocol context is novel. D-16 (shared vs per-participant split) has no direct precedent.
- MCP/A2A compatibility: MEDIUM -- mapping concepts are clear, but proxy participant implementation patterns are untested in protocol spec form

**Research date:** 2026-03-28
**Valid until:** 2026-04-28 (30 days -- stable protocol landscape, A2A/MCP specs not changing rapidly)
