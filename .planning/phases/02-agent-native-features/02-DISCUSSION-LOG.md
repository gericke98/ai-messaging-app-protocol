# Phase 2: Agent-Native Features - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-03-28
**Phase:** 02-agent-native-features
**Areas discussed:** Streaming event model, Interactive content blocks, Agent Cards & discovery, Memory architecture

---

## Streaming Event Model

| Option | Description | Selected |
|--------|-------------|----------|
| Token-level deltas | Each delta carries 1-few tokens. Lowest latency. AG-UI/OpenAI pattern. | ✓ |
| Chunk-level deltas | Larger text chunks (sentences/paragraphs). Lower message volume. | |
| Configurable per-session | Negotiate granularity during capability exchange. | |

**User's choice:** Token-level deltas
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Client-initiated cancel | Client sends cancel event, server stops. Partial message preserved. | ✓ |
| Bidirectional cancel | Both client and server/agent can cancel. | |
| No explicit cancel | Client disconnects or ignores remaining deltas. | |

**User's choice:** Client-initiated cancel
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Sequential only | One block at a time. Simpler lifecycle. | |
| Parallel multi-block | Stream into multiple blocks simultaneously. | ✓ |
| Sequential with preview hints | Sequential but declare layout upfront for skeleton UI. | |

**User's choice:** Parallel multi-block
**Notes:** Follow-up question asked about block addressing.

| Option | Description | Selected |
|--------|-------------|----------|
| Block index in declared layout | Stream-start declares structure, deltas use index. Simple addressing. | ✓ |
| Unique block IDs per stream | Each block gets UUID. More flexible, no pre-declared layout. | |

**User's choice:** Block index in declared layout
**Notes:** None

---

## Interactive Content Blocks

| Option | Description | Selected |
|--------|-------------|----------|
| ButtonBlock | Action buttons, quick replies | ✓ |
| FormBlock | Structured input forms | ✓ |
| TableBlock | Data tables | ✓ |
| ApprovalBlock | Approve/reject flows | ✓ |

**User's choice:** All four block types selected as core
**Notes:** Multi-select question

| Option | Description | Selected |
|--------|-------------|----------|
| Core block type | Charts as mandatory core type | |
| Extension block type | Charts as optional protocol-published extension | |
| Not in v1 | Defer charts entirely | ✓ |

**User's choice:** Not in v1
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Action events as thread messages | New message with ActionBlock in thread. All participants see. | ✓ |
| Direct action callbacks via JSON-RPC | Direct callback, bypasses thread model. | |
| Hybrid (action + thread echo) | Both callback and thread message. | |

**User's choice:** Action events as thread messages
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Protocol-defined catalog | Spec defines block fields. Spec IS the catalog. A2UI approach. | ✓ |
| Server-published catalog | Servers publish supported components at well-known endpoint. | |
| Client-declared in handshake | Clients declare supported types during capability exchange. | |

**User's choice:** Protocol-defined catalog
**Notes:** None

---

## Agent Cards & Discovery

| Option | Description | Selected |
|--------|-------------|----------|
| Superset of A2A | Include all A2A fields + AINM extensions. A2A clients can still read. | ✓ |
| Strict A2A conformance | Follow A2A spec exactly. Extensions in separate endpoint. | |
| AINM-native with A2A mapping | Design for AINM first, define transformation to A2A view. | |

**User's choice:** Superset of A2A
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Well-known URL + ETag caching | Standard HTTP caching. agent_card_url already in identity spec. | ✓ |
| Well-known URL + versioned URL | Version hash in URL. Identity record updates on card change. | |
| Push-based via thread events | Agent pushes card on thread join. | |

**User's choice:** Well-known URL + ETag caching
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Supported content block types | Which blocks the agent can produce | ✓ |
| Streaming support & preferences | Streaming capabilities and preferences | ✓ |
| Threading preferences | Sub-thread preferences, concurrent thread limits | ✓ |
| Memory tier support | Which memory tiers the agent uses | ✓ |

**User's choice:** All four capability fields selected
**Notes:** Multi-select question

| Option | Description | Selected |
|--------|-------------|----------|
| Yes -- tool list in card | Tools array with name, description, input schema. MCP/A2A aligned. | ✓ |
| No -- tools discovered via conversation | General description only, tools discovered in-conversation. | |
| Summary only -- categories not specifics | Tool categories without individual tool details. | |

**User's choice:** Yes -- tool list in card
**Notes:** None

---

## Memory Architecture

| Option | Description | Selected |
|--------|-------------|----------|
| Server-managed with participant hints | Server maintains tiers, participants request operations. Server enforces policy. | ✓ |
| Agent-managed (self-service) | Agents manage own memory. Server provides storage APIs only. | |
| Conversation-owner managed | Thread creator controls memory policy. | |

**User's choice:** Server-managed with participant hints
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Context frame on thread join | Server delivers structured context frame on join/@-mention. One bootstrap payload. | ✓ |
| On-demand context queries | Agent requests context via protocol methods when needed. | |
| Continuous context stream | Server pushes context updates alongside messages. | |

**User's choice:** Context frame on thread join
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Structured facts + user preferences | Key-value facts, cross-conversation knowledge. Server enforces schema. | ✓ |
| Unstructured conversation summaries | Auto-generated summaries. Query by topic/keyword. | |
| Both structured and unstructured | Structured facts + auto summaries. Two storage models. | |

**User's choice:** Structured facts + user preferences
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Conversation shared, archival per-participant | Active + conversation shared. Archival per-participant. Explicit sharing via thread. | ✓ |
| Everything shared | All tiers shared across participants. | |
| Everything per-participant | Isolated memory per participant at all tiers. | |

**User's choice:** Conversation memory shared, archival per-participant
**Notes:** None

---

## Claude's Discretion

- Tool visibility event schema design (TOOL-01 through TOOL-05)
- MCP/A2A bridge design (COMP-01 through COMP-04)
- Streaming error handling and collapse semantics
- Interactive block field-level details (form fields, approval states, table capabilities)
- Memory query methods, context frame schema, retention policies

## Deferred Ideas

None -- discussion stayed within phase scope.
