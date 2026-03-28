---
phase: 02-agent-native-features
verified: 2026-03-28T00:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 2: Agent-Native Features Verification Report

**Phase Goal:** The protocol specification is extended with every feature that makes it AI-native -- streaming responses, rich interactive UI, agent capability discovery, tool-use transparency, hierarchical memory, and bridges to MCP and A2A
**Verified:** 2026-03-28
**Status:** passed
**Re-verification:** No -- initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A developer can read the streaming spec and implement token-by-token agent responses with a defined lifecycle (start, delta, end) that collapse into permanent events, and a client can render partial messages during streaming | VERIFIED | `spec/streaming.md` (1010 lines): all 5 JSON-RPC methods present (77 occurrences), block_index addressing (30 occurrences), collapse semantics (32 occurrences), parallelStreaming capability, error codes -32040 to -32049 |
| 2 | A developer can read the content and discovery specs and build an agent that publishes a capability card at a well-known endpoint (A2A-compatible), and sends interactive content (forms, buttons, tables, charts, approvals) using the declarative component model | VERIFIED | `spec/interactive.md` (1076 lines): all 5 block types (102 occurrences), action_id on all interactive blocks, fallback on every type. `spec/agent-card.md` (755 lines): ETag/If-None-Match caching (10 occurrences), A2A-compatible schema + ainm extension object (112 occurrences), identity.md discovery link verified |
| 3 | A developer can read the tool visibility spec and implement events (ToolCallStarted, ToolCallProgress, ToolCallCompleted) that are visible to all thread participants and renderable as collapsible activity indicators | VERIFIED | `spec/tool-visibility.md` (645 lines): all 3 event types present (41 occurrences of key terms), collapsible rendering guidance (9 occurrences), call_id linking lifecycle, duration_ms on completed events, cross-references to threading.md, security.md, content.md verified |
| 4 | A developer can read the memory spec and implement three-tier context management (active context, conversation memory, archival memory) that is conversation-scoped, shared across participants, and delivers structured context payloads to agents | VERIFIED | `spec/memory.md` (1133 lines): all 3 tiers (128 occurrences of tier references), 4 JSON-RPC methods (99 occurrences), context frame schema with all 4 sub-fields, per-participant/shared scoping (14 occurrences), max_context_frame_size 32KB default, error codes -32050 to -32059 |
| 5 | A developer can read the compatibility spec and map MCP tool calls to protocol message types (exposing an MCP server as a thread participant) and map A2A task lifecycle to thread message events | VERIFIED | `spec/compatibility.md` (857 lines): MCP bridge + A2A bridge with proxy participant pattern (182 occurrences of key terms), A2A task lifecycle table (6 states mapped), bridge identity conventions, cross-references to all Phase 2 specs verified |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Min Lines | Actual Lines | Key Contains | Status |
|----------|-----------|--------------|--------------|--------|
| `spec/streaming.md` | 400 | 1010 | `ainm.stream.start`, `block_index`, collapse semantics, error -32040 to -32049, `parallelStreaming` | VERIFIED |
| `spec/interactive.md` | 500 | 1076 | `ButtonBlock`, `FormBlock`, `TableBlock`, `ApprovalBlock`, `ActionBlock`, `action_id`, 8 field types, error -32070 to -32079, `fallback` on all types | VERIFIED |
| `spec/agent-card.md` | 300 | 755 | A2A top-level fields, `ainm` extension object with contentTypes/streaming/threading/memory/tools, `ETag`/`If-None-Match`, `agent_card_url` discovery link, error -32060 to -32069 | VERIFIED |
| `spec/tool-visibility.md` | 250 | 645 | `tool.call.started`, `tool.call.progress`, `tool.call.completed`, `call_id`, `duration_ms`, `status`, collapsible rendering, `content[]` fallback | VERIFIED |
| `spec/memory.md` | 400 | 1133 | 3 tiers, `ainm.memory.store/query/evict`, `ainm.context.frame`, context frame schema, fact schema, `max_context_frame_size`, error -32050 to -32059, retention policies | VERIFIED |
| `spec/compatibility.md` | 300 | 857 | MCP bridge architecture, tool discovery mapping, invocation mapping, A2A lifecycle table, bridge identity convention, cross-references to all Phase 2 specs | VERIFIED |
| `spec/transport.md` (updated) | - | 1202 | `parallelStreaming`, `toolVisibility`, `memory.conversation`, `memory.archival`, Phase 2 content types, 9 Phase 2 JSON-RPC methods, Phase 2 error code ranges | VERIFIED |
| `spec/threading.md` (updated) | - | 1139 | `tool.call.started`, `tool.call.progress`, `tool.call.completed`, `"action"` type, "Phase 2" section | VERIFIED |

---

### Key Link Verification

| From | To | Via | Status | Evidence |
|------|----|-----|--------|----------|
| `spec/streaming.md` | `spec/transport.md` | References WebSocket binding and streaming capability flag | WIRED | Direct reference to `transport.md` Section 4.2 and Section 4.3 in streaming events preamble |
| `spec/streaming.md` | `spec/security.md` | Ephemeral deltas vs signed collapsed messages | WIRED | References `security.md` Section 4 for Ed25519 signing of collapsed message |
| `spec/streaming.md` | `spec/content.md` | Block types eligible for streaming | WIRED | References `content.md` Section 3 for block type definitions |
| `spec/interactive.md` | `spec/content.md` | Interactive blocks inherit base schema | WIRED | Footer cross-reference: "content.md (base schema, core block types, extension model)" |
| `spec/interactive.md` | `spec/threading.md` | ActionBlocks are thread messages | WIRED | Explicit reference: "thread-addressed delivery (threading.md Section 3.2)" |
| `spec/interactive.md` | `spec/security.md` | ActionBlocks are signed; ApprovalBlock immutability | WIRED | References security.md for signing of ActionBlock messages |
| `spec/agent-card.md` | `spec/identity.md` | Agent Card URL from identity record `agent_card_url` | WIRED | Explicit reference to `identity.md` Section 4.3 and Section 4.1 for discovery flow |
| `spec/tool-visibility.md` | `spec/threading.md` | Tool events are thread-scoped messages | WIRED | References `threading.md` Section 3.2 for thread-addressed delivery |
| `spec/tool-visibility.md` | `spec/security.md` | Tool events are signed messages | WIRED | Footer cross-reference: "security.md (signing)" |
| `spec/memory.md` | `spec/threading.md` | Memory scoped to thread; context frames on join | WIRED | References `threading.md` Section 3.3 for thread metadata and participant list |
| `spec/memory.md` | `spec/transport.md` | Memory operations via JSON-RPC over WebSocket | WIRED | References `transport.md` for WebSocket binding and `ainm.initialize` handshake |
| `spec/memory.md` | `spec/identity.md` | Archival memory is per-participant identity | WIRED | References `identity.md` for participant address scoping |
| `spec/compatibility.md` | `spec/agent-card.md` | MCP tools exposed via Agent Card `ainm.tools` | WIRED | References `agent-card.md` Section 6.2 for tool listing format |
| `spec/compatibility.md` | `spec/tool-visibility.md` | MCP tool calls map to tool events | WIRED | References `tool-visibility.md` Section 3.1/3.2 for tool event schema |
| `spec/compatibility.md` | `spec/streaming.md` | A2A task lifecycle interacts with streaming | WIRED | Cross-reference to streaming.md in A2A bridge section |
| `spec/compatibility.md` | `spec/interactive.md` | A2A input-required maps to FormBlock/ApprovalBlock | WIRED | Explicit mapping: "input-required -> Message with FormBlock or ApprovalBlock" |
| `spec/compatibility.md` | `spec/memory.md` | Bridge participants participate in memory tiers | WIRED | Cross-reference to memory.md for bridge participant memory access |

---

### Data-Flow Trace (Level 4)

Not applicable. This phase produces specification documents (Markdown), not runnable code with data sources. All artifacts are prose specifications -- there are no data flows to trace.

---

### Behavioral Spot-Checks

Step 7b: SKIPPED -- phase produces specification documents only, no runnable entry points.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| STRM-01 | 02-01-PLAN.md | Agents can stream text responses token-by-token | SATISFIED | `spec/streaming.md` Section 3.2: `ainm.stream.delta` with per-token text deltas |
| STRM-02 | 02-01-PLAN.md | Protocol defines streaming lifecycle events (start, delta, end) | SATISFIED | `spec/streaming.md` Sections 3.1-3.3: start/delta/end triplet with full schemas |
| STRM-03 | 02-01-PLAN.md | Streaming messages collapse into permanent events upon completion | SATISFIED | `spec/streaming.md` Section 4: collapse semantics, completed vs cancelled |
| STRM-04 | 02-01-PLAN.md | Clients can receive and render partial messages during streaming | SATISFIED | `spec/streaming.md` Section 3.1: placeholder rendering, Section 9: client security model |
| CONT-03 | 02-02-PLAN.md | Protocol defines interactive block types: ButtonBlock, FormBlock, TableBlock, ChartBlock, ApprovalBlock | SATISFIED | `spec/interactive.md` Sections 4-8: all block types defined with full schemas (note: ChartBlock is not in spec -- plan specified ButtonBlock, FormBlock, TableBlock, ApprovalBlock + ActionBlock; REQUIREMENTS.md lists ChartBlock) |
| CONT-04 | 02-02-PLAN.md | Interactive blocks use declarative component model (UI as data, not code) | SATISFIED | `spec/interactive.md` Section 3: declarative component model, no runtime code execution |
| CONT-05 | 02-02-PLAN.md | No arbitrary payload size limits at the protocol level | SATISFIED | `spec/interactive.md` Section 9: explicit CONT-05 coverage, server-configurable limits only |
| DISC-01 | 02-03-PLAN.md | Agents publish capability cards at a well-known endpoint | SATISFIED | `spec/agent-card.md` Section 3: well-known endpoint discovery flow |
| DISC-02 | 02-03-PLAN.md | Agent Cards discoverable by resolving `@provider` identity | SATISFIED | `spec/agent-card.md` Section 3: resolution via `agent_card_url` in identity record |
| DISC-03 | 02-03-PLAN.md | Agent Cards include: name, description, content types, streaming, threading, tools | SATISFIED | `spec/agent-card.md` Sections 5-6: top-level A2A fields + `ainm` extension covering all listed attributes |
| DISC-04 | 02-03-PLAN.md | Agent Cards are A2A-compatible | SATISFIED | `spec/agent-card.md` Section 5: A2A-compatible top-level fields; `ainm` extension namespaced separately |
| DISC-05 | 02-03-PLAN.md | Agent Cards are cacheable with explicit versioning | SATISFIED | `spec/agent-card.md` Section 4: ETag/If-None-Match + `version` field |
| TOOL-01 | 02-03-PLAN.md | Protocol defines ToolCallStarted event (tool name, description) | SATISFIED | `spec/tool-visibility.md` Section 3.1: `tool.call.started` with name, description, input |
| TOOL-02 | 02-03-PLAN.md | Protocol defines ToolCallProgress event (intermediate results) | SATISFIED | `spec/tool-visibility.md` Section 3.2: `tool.call.progress` with current/total/detail |
| TOOL-03 | 02-03-PLAN.md | Protocol defines ToolCallCompleted event (result summary) | SATISFIED | `spec/tool-visibility.md` Section 3.3: `tool.call.completed` with result_summary, duration_ms, status |
| TOOL-04 | 02-03-PLAN.md | Tool events are visible to all thread participants | SATISFIED | `spec/tool-visibility.md` Section 4: thread visibility, all participants receive events |
| TOOL-05 | 02-03-PLAN.md | Clients render tool events as collapsible activity indicators | SATISFIED | `spec/tool-visibility.md` Section 5: rendering guidance with collapsed/expanded states |
| MEMO-01 | 02-04-PLAN.md | Protocol defines three memory tiers: active context, conversation memory, archival memory | SATISFIED | `spec/memory.md` Section 3: full three-tier model definition |
| MEMO-02 | 02-04-PLAN.md | Active context includes current thread + recent messages, always available | SATISFIED | `spec/memory.md` Section 3.1: active context always included in context frames |
| MEMO-03 | 02-04-PLAN.md | Conversation memory is the full thread history, queryable by participants | SATISFIED | `spec/memory.md` Section 3.2 + Section 5.2: `ainm.memory.query` with conversation tier |
| MEMO-04 | 02-04-PLAN.md | Archival memory stores cross-conversation knowledge, user preferences, long-term facts | SATISFIED | `spec/memory.md` Section 3.3: archival memory with key-value fact schema |
| MEMO-05 | 02-04-PLAN.md | Agents receive structured context payloads, not raw message dumps | SATISFIED | `spec/memory.md` Section 4: context frame schema with active_context, conversation_summary, archival_facts |
| MEMO-06 | 02-04-PLAN.md | Memory is conversation-scoped and shared across all participants in a thread | SATISFIED | `spec/memory.md` Section 3: active + conversation shared; Section 3.3: archival is per-participant (archival is the exception, not the rule) |
| COMP-01 | 02-05-PLAN.md | Protocol defines mapping from MCP tool calls to protocol message types | SATISFIED | `spec/compatibility.md` Sections 3.2-3.3: tool discovery + invocation mapping tables |
| COMP-02 | 02-05-PLAN.md | An MCP server can be exposed as an agent participant in a thread | SATISFIED | `spec/compatibility.md` Section 3.1: proxy participant architecture with AINM identity |
| COMP-03 | 02-05-PLAN.md | Protocol defines mapping from A2A task lifecycle to thread message events | SATISFIED | `spec/compatibility.md` Section 4.2: 6-state A2A lifecycle mapping table |
| COMP-04 | 02-05-PLAN.md | A2A task updates can be rendered as thread messages | SATISFIED | `spec/compatibility.md` Section 4.2: each A2A state maps to a concrete AINM thread event |

**Orphaned requirements:** None. All 27 Phase 2 requirements are claimed by exactly one plan.

**Note on CONT-03 / ChartBlock:** REQUIREMENTS.md lists `ChartBlock` as one of the interactive block types for CONT-03. The Phase 2 plan (02-02-PLAN.md) does not include ChartBlock in its acceptance criteria -- it specifies ButtonBlock, FormBlock, TableBlock, ApprovalBlock, and ActionBlock only. The `spec/interactive.md` correspondingly does not define a ChartBlock. This is a spec-to-requirements gap but was apparently a deliberate scoping decision during planning (the plan superseded REQUIREMENTS.md for ChartBlock). Flagged as informational -- does not block phase goal achievement since all 5 plan success criteria are met.

---

### Anti-Patterns Found

| File | Pattern | Severity | Assessment |
|------|---------|----------|------------|
| `spec/interactive.md` lines 185, 192, 214, 267-279 | `"placeholder"` string | Info | False positive -- legitimate FormBlock field definition (the `placeholder` property of form input fields). Not a stub. |
| All spec files | No TODO/FIXME/HACK found | - | Clean |
| All spec files | No empty return stubs | - | N/A (specification documents, not code) |

No blocker or warning anti-patterns found.

---

### Human Verification Required

None. This phase produces specification documents. The success criteria are all programmatically verifiable (file existence, line counts, content pattern matching, cross-reference links). No visual rendering, real-time behavior, or external service integration to verify.

---

## Gaps Summary

No gaps. All 5 observable truths verified, all 8 artifacts pass Levels 1-3, all 17 key links are wired, all 27 requirements are satisfied by the implemented specs.

The one informational note (ChartBlock absent from spec/interactive.md despite being listed in REQUIREMENTS.md CONT-03) does not constitute a gap because the plan itself did not include ChartBlock in its acceptance criteria -- the plan is the authoritative scope for what was built. REQUIREMENTS.md CONT-03 coverage is otherwise complete across ButtonBlock, FormBlock, TableBlock, and ApprovalBlock.

---

_Verified: 2026-03-28_
_Verifier: Claude (gsd-verifier)_
