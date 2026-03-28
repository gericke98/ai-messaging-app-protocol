# Phase 2: Agent-Native Features - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend the AINM protocol specification with the features that make it AI-native: streaming responses, rich interactive content blocks, agent capability discovery (Agent Cards), tool-use transparency, hierarchical memory management, and MCP/A2A compatibility bridges. Output is spec documents extending the Phase 1 kernel. No reference implementation code -- spec only.

</domain>

<decisions>
## Implementation Decisions

### Streaming Event Model
- **D-01:** Token-level deltas -- each delta carries 1-few tokens as generated. Lowest latency, most responsive UX. Aligns with AG-UI and OpenAI streaming patterns.
- **D-02:** Client-initiated cancellation -- client sends explicit cancel event (e.g., `ainm.stream.cancel`). Server/agent acknowledges and stops. Partial message preserved with `cancelled` status. No server-initiated or bidirectional cancel.
- **D-03:** Parallel multi-block streaming -- agents CAN stream into multiple content blocks simultaneously (e.g., text explanation + code). Stream-start event declares the full block layout upfront (types + positions). Clients render placeholders for all declared blocks immediately.
- **D-04:** Block index addressing in deltas -- delta events reference blocks by their index in the declared layout (from stream-start). Simple addressing, no separate block IDs needed for streaming.

### Interactive Content Blocks
- **D-05:** Core interactive block types (all clients MUST support): ButtonBlock, FormBlock, TableBlock, ApprovalBlock. These join the existing core set (TextBlock, CodeBlock, ImageBlock, FileBlock) from Phase 1.
- **D-06:** ChartBlock deferred -- not in v1. The 8 core block types (4 static + 4 interactive) are sufficient.
- **D-07:** Action events flow as thread messages -- when a user clicks a button or submits a form, client sends a new message in the thread with an ActionBlock containing the block reference + user input. Aligns with thread-addressed model (Phase 1 D-11). All participants see actions.
- **D-08:** Protocol-defined component catalog -- the spec defines exactly what fields each interactive block type has. Agents construct blocks from the spec, clients render from the spec. No runtime catalog discovery. The spec IS the catalog. Follows A2UI approach.

### Agent Cards & Discovery
- **D-09:** Superset of A2A -- AINM Agent Cards include all A2A required fields (name, description, url, capabilities) PLUS AINM-specific extensions. A2A clients can read the card; AINM clients get richer info.
- **D-10:** Well-known URL + ETag caching -- Agent Card lives at the `agent_card_url` from identity resolution (Phase 1 D-06). Standard HTTP caching with ETag/If-None-Match. No new infrastructure.
- **D-11:** AINM-specific capability fields in Agent Card: supported content block types, streaming support & preferences, threading preferences, memory tier support. All four declared by user.
- **D-12:** Tool list in Agent Card -- card includes a tools array listing available tools with name, description, and input schema. Aligns with MCP tool listing and A2A skills field.

### Memory Architecture
- **D-13:** Server-managed with participant hints -- server maintains all three tiers (active context, conversation memory, archival). Participants can REQUEST memory operations (store, query, evict) but server enforces policy (retention, size, permissions). Server is authoritative.
- **D-14:** Context frame on thread join -- when an agent joins a thread or is @-mentioned, server delivers a structured context frame: active context (recent messages), conversation summary, relevant archival facts, participant list + capabilities. One payload to bootstrap the agent. Inspired by Letta/MemGPT context assembly.
- **D-15:** Archival memory stores structured facts + user preferences -- key-value facts, cross-conversation knowledge. Agents store and query structured data. Server enforces schema. Unstructured summaries deferred.
- **D-16:** Conversation memory shared, archival per-participant -- active context and conversation memory are shared across all thread participants. Archival memory is per-participant (each agent/user has their own long-term store). Agents share archival facts explicitly by posting to thread.

### Tool Visibility (no user discussion needed -- technical)
- Spec will define ToolCallStarted, ToolCallProgress, ToolCallCompleted events per requirements TOOL-01 through TOOL-05. These are thread events visible to all participants, rendered as collapsible activity indicators. Researcher and planner have full discretion on event schema design.

### MCP/A2A Compatibility (no user discussion needed -- technical)
- Spec will define mappings per requirements COMP-01 through COMP-04. MCP tool calls map to protocol message types; MCP servers can be exposed as thread participants. A2A task lifecycle maps to thread events. Researcher and planner have full discretion on bridge design.

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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 1 Specs (foundation for Phase 2 extensions)
- `spec/identity.md` -- Identity model with `agent_card_url` field (Section 4.2). Agent Cards connect to this.
- `spec/security.md` -- Message signing, trust annotations (`source` field). Interactive blocks inherit this.
- `spec/content.md` -- Content block base schema (type, version, source, fallback). Interactive blocks extend this.
- `spec/transport.md` -- WebSocket binding with streaming capability flag. Streaming spec builds on this.
- `spec/threading.md` -- Thread model, sub-threading, presence. Memory and tool events are thread-scoped.

### Research
- `research/landscape-protocols-projects.md` -- Survey of 15+ protocols including A2A Agent Cards, A2UI declarative UI, Letta/MemGPT memory, AG-UI streaming, MCP tools
- `research/bottlenecks-current-messaging-apps.md` -- Current messaging limitations that Phase 2 features address

### Phase 1 Context (prior decisions)
- `.planning/phases/01-protocol-kernel/01-CONTEXT.md` -- Foundational decisions: modular spec files (D-01), developer-friendly prose (D-02), closed core + namespaced extensions (D-13), mandatory fallback (D-14), block-level versioning (D-15)

### External References
- A2A Agent Card specification (Google) -- AINM cards are a superset of this format
- A2UI v0.8 (Google) -- Declarative UI-as-data model informing interactive block design
- AG-UI (CopilotKit) -- Streaming event model reference
- Letta/MemGPT -- Hierarchical memory architecture reference (active/conversation/archival)
- MCP tool listing -- Tool array format informing Agent Card tools field

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- 5 Phase 1 spec documents in `spec/` -- all interactive blocks, streaming, and memory specs extend these
- Content block base schema (content.md Section 3) -- interactive blocks inherit type/version/source/fallback fields
- JSON-RPC method naming convention (transport.md) -- streaming and memory methods follow `ainm.*` pattern
- Identity record schema (identity.md Section 4) -- `agent_card_url` field is the hook for Agent Card discovery

### Established Patterns
- RFC-keyword prose style with inline JSON examples (Phase 1 D-02, D-03)
- Closed core + namespaced extensions model (Phase 1 D-13)
- Mandatory fallback on all content blocks (Phase 1 D-14)
- Thread-addressed messages (Phase 1 D-11) -- all new event types (streaming, tool, action) go to threads

### Integration Points
- Interactive blocks extend `spec/content.md` -- either as new sections or a separate `spec/interactive-content.md`
- Streaming spec references `spec/transport.md` WebSocket binding and capability negotiation
- Agent Cards connect to `spec/identity.md` `agent_card_url` field
- Memory methods use JSON-RPC pattern from `spec/transport.md`
- Tool events are thread events per `spec/threading.md` event model

</code_context>

<specifics>
## Specific Ideas

- Parallel multi-block streaming is the most ambitious design choice -- no existing protocol does this. The stream-start event declaring full block layout upfront (with block index addressing) is the key enabler. This should be thoroughly specified with edge cases.
- Action events as thread messages (D-07) is a deliberate choice to keep interactions visible to all participants. This means an ApprovalBlock approval is a thread message that everyone sees -- transparent by design.
- The memory architecture directly mirrors Letta/MemGPT's three-tier model but adapted to a multi-participant protocol context. The "conversation memory shared, archival per-participant" split is novel.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>

---

*Phase: 02-agent-native-features*
*Context gathered: 2026-03-28*
